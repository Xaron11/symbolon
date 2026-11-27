//! Offset pages and row orders of `Comparison` results.

#![cfg(feature = "local")]
#![expect(clippy::unwrap_used, reason = "test helpers fail loudly by design")]

use std::fs;

use symbolon_core::catalog::{Catalog, CatalogError, HashPolicy, NewLocation};
use symbolon_core::compare::{
    Category, CompareError, CompareOptions, Comparison, Direction, DuplicateSet, ExtensionTotals,
    Row, RowFilter, RowOrder, SIZE_BUCKETS, Side,
};
use symbolon_core::query::Page;
use symbolon_core::scan::{ScanOptions, Scanner};

struct Fixture {
    _dir: tempfile::TempDir,
    catalog: Catalog,
}

/// Two locations of about 1,050 rows: identical files in ten folders, every 50th file
/// different, 30 files only on the left, one large file and one empty folder on the left.
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let catalog = Catalog::open(dir.path().join("catalog.db")).unwrap();
    for label in ["left", "right"] {
        let root = dir.path().join(label);
        for i in 0..1000 {
            let path = root.join(format!("d{}/f{i:04}.txt", i / 100));
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            let changed = label == "right" && i % 50 == 0;
            let contents = if changed {
                format!("y{i}")
            } else {
                format!("x{i}")
            };
            fs::write(path, contents.repeat(i % 7 + 1)).unwrap();
        }
        fs::write(root.join("big.bin"), vec![7u8; 10_000]).unwrap();
        // A directory is a row only on some sides with nothing compared beneath it
        // (ADR-0019).
        if label == "left" {
            fs::create_dir_all(root.join("empty")).unwrap();
        }
        if label == "left" {
            fs::create_dir_all(root.join("extra")).unwrap();
            for j in 0..30 {
                fs::write(root.join(format!("extra/e{j:02}.txt")), "e").unwrap();
            }
        }
        catalog
            .add_location(NewLocation::local(label, &root).hash_policy(HashPolicy::Eager))
            .unwrap();
        Scanner::start(&catalog, label, ScanOptions::default())
            .unwrap()
            .wait()
            .unwrap();
    }
    Fixture { _dir: dir, catalog }
}

fn compare(f: &Fixture) -> Comparison {
    Comparison::new(
        &f.catalog,
        &[Side::location("left"), Side::location("right")],
        CompareOptions::default(),
    )
    .unwrap()
}

fn keys(rows: &[Row]) -> Vec<String> {
    rows.iter().map(|r| r.key.clone()).collect()
}

fn by_cursor(cmp: &Comparison, filter: &RowFilter) -> Vec<String> {
    let mut all = Vec::new();
    let mut page = Page::first(100);
    loop {
        let got = cmp.rows(filter, RowOrder::default(), &page).unwrap();
        all.extend(keys(&got.items));
        match got.next {
            Some(cursor) => page = Page::after(cursor, 100),
            None => return all,
        }
    }
}

fn by_offset(cmp: &Comparison, filter: &RowFilter, order: RowOrder) -> Vec<String> {
    let mut all = Vec::new();
    let mut offset = 0;
    loop {
        let got = cmp.rows(filter, order, &Page::at(offset, 100)).unwrap();
        assert!(got.next.is_none(), "offset pages carry no cursor");
        if got.items.is_empty() {
            return all;
        }
        offset += got.items.len() as u64;
        all.extend(keys(&got.items));
    }
}

#[test]
fn offset_pages_match_cursor_order() {
    let f = fixture();
    let cmp = compare(&f);
    for filter in [
        RowFilter::default(),
        RowFilter::categories([Category::Different]),
    ] {
        let cursor = by_cursor(&cmp, &filter);
        assert!(cursor.len() > 10);
        assert_eq!(by_offset(&cmp, &filter, RowOrder::default()), cursor);
    }
}

#[test]
fn offset_past_end_is_empty() {
    let f = fixture();
    let cmp = compare(&f);
    let got = cmp
        .rows(
            &RowFilter::default(),
            RowOrder::default(),
            &Page::at(10_000, 10),
        )
        .unwrap();
    assert_eq!(got.items, []);
    assert!(got.next.is_none());
}

#[test]
fn size_order_desc_puts_largest_first_and_unsized_last() {
    let f = fixture();
    let cmp = compare(&f);
    for direction in [Direction::Asc, Direction::Desc] {
        let page = cmp
            .rows(
                &RowFilter::default(),
                RowOrder::Size(direction),
                &Page::at(0, 2000),
            )
            .unwrap();
        let sizes: Vec<_> = page.items.iter().map(|r| r.size).collect();
        let first_unsized = sizes.iter().position(Option::is_none).unwrap();
        assert!(
            sizes[first_unsized..].iter().all(Option::is_none),
            "{direction:?}"
        );
        let known: Vec<u64> = sizes[..first_unsized].iter().map(|s| s.unwrap()).collect();
        let mut expected = known.clone();
        expected.sort_unstable();
        if direction == Direction::Desc {
            expected.reverse();
            assert_eq!(page.items[0].key, "big.bin");
        }
        assert_eq!(known, expected, "{direction:?}");
    }
}

#[test]
fn size_order_ties_break_by_path() {
    let f = fixture();
    let cmp = compare(&f);
    let page = cmp
        .rows(
            &RowFilter::default(),
            RowOrder::Size(Direction::Desc),
            &Page::at(0, 2000),
        )
        .unwrap();
    for pair in page.items.windows(2) {
        if pair[0].size == pair[1].size {
            assert!(
                pair[0].key < pair[1].key,
                "{} then {}",
                pair[0].key,
                pair[1].key
            );
        }
    }
}

#[test]
fn page_with_cursor_and_offset_is_rejected() {
    let f = fixture();
    let cmp = compare(&f);
    let cursor = cmp
        .rows(&RowFilter::default(), RowOrder::default(), &Page::first(10))
        .unwrap()
        .next
        .unwrap();
    let mut page = Page::after(cursor, 10);
    page.offset = Some(5);
    let err = cmp
        .rows(&RowFilter::default(), RowOrder::default(), &page)
        .unwrap_err();
    assert!(
        matches!(err, CompareError::Catalog(CatalogError::Corrupt(_))),
        "{err:?}"
    );
}

/// Two identical locations holding `files` (path, size in bytes), plus `left_only` on the left.
fn sized(files: &[(&str, usize)], left_only: &[(&str, usize)]) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let catalog = Catalog::open(dir.path().join("catalog.db")).unwrap();
    for label in ["left", "right"] {
        let root = dir.path().join(label);
        let extra = if label == "left" { left_only } else { &[] };
        for (i, (path, size)) in files.iter().chain(extra).enumerate() {
            let path = root.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            // Distinct contents, so no two files are duplicates by accident.
            let mut contents = format!("{i:08}").into_bytes();
            contents.resize(*size, b'.');
            fs::write(path, contents).unwrap();
        }
        catalog
            .add_location(NewLocation::local(label, &root).hash_policy(HashPolicy::Eager))
            .unwrap();
        Scanner::start(&catalog, label, ScanOptions::default())
            .unwrap()
            .wait()
            .unwrap();
    }
    Fixture { _dir: dir, catalog }
}

fn extensions(totals: &[ExtensionTotals]) -> Vec<(Option<&str>, u64, u64)> {
    totals
        .iter()
        .map(|t| (t.extension.as_deref(), t.rows, t.bytes))
        .collect()
}

#[test]
fn by_extension_ranks_by_bytes_and_folds_rest() {
    let f = sized(
        &[
            ("a.jpg", 500),
            ("b.png", 400),
            ("c.txt", 150),
            ("d.txt", 150),
            ("e.md", 200),
            ("f.rs", 100),
            ("noext", 50),
        ],
        &[],
    );
    let cmp = compare(&f);
    let totals = cmp.by_extension(&RowFilter::default(), 3).unwrap();
    // Bytes count both sides.
    assert_eq!(
        extensions(&totals),
        [
            (Some("jpg"), 1, 1000),
            (Some("png"), 1, 800),
            (Some("txt"), 2, 600),
            (None, 3, 700),
        ]
    );
    let all = cmp.by_extension(&RowFilter::default(), 10).unwrap();
    assert!(
        all.iter().all(|t| t.extension.is_some()),
        "nothing left to fold"
    );
    assert!(extensions(&all).contains(&(Some(""), 1, 100)));
}

#[test]
fn by_extension_respects_filter() {
    let f = sized(&[("a.jpg", 500)], &[("b.png", 400), ("c.png", 10)]);
    let cmp = compare(&f);
    let totals = cmp
        .by_extension(&RowFilter::categories([Category::OnlyIn]), 10)
        .unwrap();
    assert_eq!(extensions(&totals), [(Some("png"), 2, 410)]);
}

#[test]
fn size_histogram_has_six_buckets_with_edges() {
    let f = sized(
        &[
            ("a", 1023),
            ("b", 1024),
            ("c", (1 << 20) - 1),
            ("d", 1 << 20),
        ],
        &[],
    );
    let cmp = compare(&f);
    let buckets = cmp.size_histogram(&RowFilter::default()).unwrap();
    assert_eq!(
        buckets.iter().map(|b| b.max_exclusive).collect::<Vec<_>>(),
        SIZE_BUCKETS
    );
    assert_eq!(
        buckets.iter().map(|b| b.rows).collect::<Vec<_>>(),
        [1, 2, 1, 0, 0, 0]
    );
    assert_eq!(buckets[0].bytes, 2 * 1023);
}

#[test]
fn size_histogram_skips_directories() {
    let f = fixture();
    let cmp = compare(&f);
    let rows: u64 = cmp
        .size_histogram(&RowFilter::default())
        .unwrap()
        .iter()
        .map(|b| b.rows)
        .sum();
    // Every row but the empty folder.
    assert_eq!(rows, cmp.count(&RowFilter::default()).unwrap() - 1);
}

#[test]
fn duplicates_offset_pages_match_cursor_pages() {
    let files: Vec<(String, usize)> = (0..5)
        .flat_map(|g| (0..3).map(move |i| (format!("g{g}/copy{i}.bin"), 100 + g)))
        .collect();
    let dir = tempfile::tempdir().unwrap();
    let catalog = Catalog::open(dir.path().join("catalog.db")).unwrap();
    let root = dir.path().join("only");
    for (path, size) in &files {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![b'x'; *size]).unwrap();
    }
    catalog
        .add_location(NewLocation::local("only", &root).hash_policy(HashPolicy::Eager))
        .unwrap();
    Scanner::start(&catalog, "only", ScanOptions::default())
        .unwrap()
        .wait()
        .unwrap();
    let cmp = Comparison::new(
        &catalog,
        &[Side::location("only")],
        CompareOptions::default().duplicates(true),
    )
    .unwrap();
    let digests =
        |items: Vec<DuplicateSet>| items.into_iter().map(|d| d.digest).collect::<Vec<_>>();
    let mut by_cursor = Vec::new();
    let mut page = Page::first(2);
    loop {
        let got = cmp.duplicates(0, &page).unwrap();
        by_cursor.extend(digests(got.items));
        match got.next {
            Some(cursor) => page = Page::after(cursor, 2),
            None => break,
        }
    }
    let mut by_offset = Vec::new();
    for offset in (0..6).step_by(2) {
        let got = cmp.duplicates(0, &Page::at(offset, 2)).unwrap();
        assert!(got.next.is_none());
        by_offset.extend(digests(got.items));
    }
    assert_eq!(by_cursor.len(), 5);
    assert_eq!(by_offset, by_cursor);
}
