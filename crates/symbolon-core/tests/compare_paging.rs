//! Offset pages and row orders of `Comparison` results.

#![cfg(feature = "local")]
#![expect(clippy::unwrap_used, reason = "test helpers fail loudly by design")]

use std::fs;

use symbolon_core::catalog::{Catalog, CatalogError, HashPolicy, NewLocation};
use symbolon_core::compare::{
    Category, CompareError, CompareOptions, Comparison, Direction, Row, RowFilter, RowOrder, Side,
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
