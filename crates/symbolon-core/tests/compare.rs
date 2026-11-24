//! `Comparison` on scanned locations: sides, algorithms, categories, key options, paging.

#![cfg(feature = "local")]
#![expect(clippy::unwrap_used, reason = "test helpers fail loudly by design")]

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, UNIX_EPOCH};

use symbolon_core::catalog::{Catalog, CatalogError, HashPolicy, NewLocation};
use symbolon_core::compare::{
    Category, CompareError, CompareOptions, Comparison, Row, RowFilter, Side, SideSet,
};
use symbolon_core::hash::HashAlgo;
use symbolon_core::pathkey::KeyOptions;
use symbolon_core::query::Page;
use symbolon_core::scan::{ScanOptions, Scanner};

struct Fixture {
    dir: tempfile::TempDir,
    catalog: Catalog,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let catalog = Catalog::open(dir.path().join("catalog.db")).unwrap();
        Self { dir, catalog }
    }

    fn root(&self, label: &str) -> PathBuf {
        self.dir.path().join(label)
    }

    /// Creates a location with `files` (path, contents, mtime seconds) and scans it.
    fn location(
        &self,
        label: &str,
        policy: HashPolicy,
        algo: HashAlgo,
        files: &[(&str, &str, u64)],
    ) {
        let root = self.root(label);
        fs::create_dir_all(&root).unwrap();
        for (path, contents, mtime) in files {
            let path = root.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, contents).unwrap();
            fs::File::options()
                .write(true)
                .open(&path)
                .unwrap()
                .set_modified(UNIX_EPOCH + Duration::from_secs(*mtime))
                .unwrap();
        }
        self.catalog
            .add_location(
                NewLocation::local(label, &root)
                    .hash_policy(policy)
                    .hash_algo(algo),
            )
            .unwrap();
        Scanner::start(&self.catalog, label, ScanOptions::default())
            .unwrap()
            .wait()
            .unwrap();
    }

    fn eager(&self, label: &str, files: &[(&str, &str)]) {
        let files: Vec<_> = files.iter().map(|(p, c)| (*p, *c, 1_000)).collect();
        self.location(label, HashPolicy::Eager, HashAlgo::Blake3, &files);
    }

    fn compare(&self, sides: &[&str], options: CompareOptions) -> Comparison {
        let sides: Vec<Side> = sides.iter().map(|s| s.parse().unwrap()).collect();
        Comparison::new(&self.catalog, &sides, options).unwrap()
    }
}

fn all_rows(comparison: &Comparison) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut page = Page::first(3);
    loop {
        let result = comparison.rows(&RowFilter::default(), &page).unwrap();
        rows.extend(result.items);
        match result.next {
            Some(cursor) => page = Page::after(cursor, 3),
            None => return rows,
        }
    }
}

/// `key -> (category, presence, assumed)`.
fn classes(comparison: &Comparison) -> BTreeMap<String, (Category, Vec<usize>, bool)> {
    all_rows(comparison)
        .into_iter()
        .map(|row| {
            (
                row.key.clone(),
                (row.category, row.presence.iter().collect(), row.assumed),
            )
        })
        .collect()
}

#[test]
fn two_sides_identical_different_and_only_in() {
    let fx = Fixture::new();
    fx.eager(
        "a",
        &[
            ("same.txt", "x"),
            ("content.txt", "aa"),
            ("size.txt", "a"),
            ("only_a", "1"),
            ("d/deep", "z"),
        ],
    );
    fx.eager(
        "b",
        &[
            ("same.txt", "x"),
            ("content.txt", "bb"),
            ("size.txt", "abc"),
            ("only_b", "2"),
            ("d/deep", "z"),
        ],
    );
    let cmp = fx.compare(&["a", "b"], CompareOptions::default());

    let expected = BTreeMap::from([
        (
            "content.txt".to_owned(),
            (Category::Different, vec![0, 1], false),
        ),
        (
            "d/deep".to_owned(),
            (Category::Identical, vec![0, 1], false),
        ),
        ("only_a".to_owned(), (Category::OnlyIn, vec![0], false)),
        ("only_b".to_owned(), (Category::OnlyIn, vec![1], false)),
        (
            "same.txt".to_owned(),
            (Category::Identical, vec![0, 1], false),
        ),
        (
            "size.txt".to_owned(),
            (Category::Different, vec![0, 1], false),
        ),
    ]);
    assert_eq!(classes(&cmp), expected);

    let summary = cmp.summary().unwrap();
    assert_eq!(summary.files(), 10);
    assert_eq!(
        (
            summary.get(Category::Identical).rows,
            summary.get(Category::Identical).files
        ),
        (2, 4)
    );
    assert_eq!(summary.get(Category::Different).bytes, 2 + 2 + 1 + 3);

    let rows = all_rows(&cmp);
    let deep = rows.iter().find(|r| r.key == "d/deep").unwrap();
    assert_eq!(deep.cells.len(), 2);
    assert_eq!(deep.cells[0].path.key().as_str(), "d/deep");
    assert!(deep.cells.iter().all(|c| c.digest.is_some()));
}

#[test]
fn three_sides_keep_the_presence_matrix() {
    let fx = Fixture::new();
    fx.eager("a", &[("all", "same"), ("ab", "pair"), ("diff", "one")]);
    fx.eager("b", &[("all", "same"), ("ab", "pair"), ("diff", "one")]);
    fx.eager("c", &[("all", "same"), ("diff", "two"), ("c_only", "!")]);
    let cmp = fx.compare(&["a", "b", "c"], CompareOptions::default());
    assert_eq!(
        classes(&cmp),
        BTreeMap::from([
            ("ab".to_owned(), (Category::OnlyIn, vec![0, 1], false)),
            (
                "all".to_owned(),
                (Category::Identical, vec![0, 1, 2], false)
            ),
            ("c_only".to_owned(), (Category::OnlyIn, vec![2], false)),
            (
                "diff".to_owned(),
                (Category::Different, vec![0, 1, 2], false)
            ),
        ])
    );
}

#[test]
fn unhashed_files_are_unverified_unless_sizes_settle_it_or_fast_mode_assumes() {
    let fx = Fixture::new();
    let lazy = |label, files: &[(&str, &str, u64)]| {
        fx.location(label, HashPolicy::Lazy, HashAlgo::Blake3, files);
    };
    lazy(
        "a",
        &[
            ("same_time", "aaaa", 1_000),
            ("other_time", "aaaa", 1_000),
            ("sizes", "a", 1_000),
        ],
    );
    lazy(
        "b",
        &[
            ("same_time", "bbbb", 1_000),
            ("other_time", "aaaa", 9_000),
            ("sizes", "abc", 1_000),
        ],
    );

    let strict = classes(&fx.compare(&["a", "b"], CompareOptions::default()));
    assert_eq!(
        strict["same_time"],
        (Category::Unverified, vec![0, 1], false)
    );
    assert_eq!(
        strict["other_time"],
        (Category::Unverified, vec![0, 1], false)
    );
    assert_eq!(
        strict["sizes"],
        (Category::Different, vec![0, 1], false),
        "sizes decide without hashing"
    );

    let fast = fx.compare(
        &["a", "b"],
        CompareOptions::default().assume_equal_within(Duration::from_secs(2)),
    );
    let fast_classes = classes(&fast);
    assert_eq!(
        fast_classes["same_time"],
        (Category::Identical, vec![0, 1], true),
        "assumed, even though contents differ"
    );
    assert_eq!(
        fast_classes["other_time"],
        (Category::Unverified, vec![0, 1], false)
    );
    assert_eq!(
        fast_classes["sizes"],
        (Category::Different, vec![0, 1], false)
    );
    assert_eq!(
        fast.summary()
            .unwrap()
            .get(Category::Identical)
            .assumed_rows,
        1
    );
}

#[test]
fn case_and_unicode_options_match_keys() {
    let fx = Fixture::new();
    fx.eager("a", &[("Photo.JPG", "img"), ("Cafe\u{301}.txt", "menu")]);
    fx.eager("b", &[("photo.jpg", "img"), ("Café.txt", "menu")]);

    let plain = classes(&fx.compare(&["a", "b"], CompareOptions::default()));
    assert_eq!(plain.len(), 4, "without options nothing matches: {plain:?}");
    assert!(
        plain
            .values()
            .all(|(category, _, _)| *category == Category::OnlyIn)
    );

    let both = KeyOptions {
        nfc: true,
        case_insensitive: true,
    };
    let matched = classes(&fx.compare(&["a", "b"], CompareOptions::default().keys(both)));
    assert_eq!(
        matched,
        BTreeMap::from([
            (
                "café.txt".to_owned(),
                (Category::Identical, vec![0, 1], false)
            ),
            (
                "photo.jpg".to_owned(),
                (Category::Identical, vec![0, 1], false)
            ),
        ])
    );
}

#[test]
fn names_that_collapse_on_one_side_are_collisions() {
    let fx = Fixture::new();
    fx.eager("a", &[("Readme", "1"), ("README", "2")]);
    fx.eager("b", &[("readme", "1")]);
    let ci = CompareOptions::default().keys(KeyOptions {
        case_insensitive: true,
        ..KeyOptions::default()
    });
    let cmp = fx.compare(&["a", "b"], ci);
    let rows = all_rows(&cmp);
    assert_eq!(rows.len(), 1);
    assert_eq!(
        (rows[0].category, rows[0].cells.len()),
        (Category::Collision, 3)
    );
}

#[test]
fn subpaths_compare_relative_keys_without_leaking_siblings() {
    let fx = Fixture::new();
    fx.eager(
        "disk",
        &[
            ("photos/a.jpg", "A"),
            ("photos/sub/b.jpg", "B"),
            ("photos2/x", "X"),
            ("photos-old/y", "Y"),
        ],
    );
    fx.eager(
        "phone",
        &[
            ("DCIM/a.jpg", "A"),
            ("DCIM/sub/b.jpg", "changed"),
            ("DCIM/new.jpg", "N"),
        ],
    );
    let cmp = fx.compare(&["disk:photos", "phone:DCIM"], CompareOptions::default());
    assert_eq!(
        classes(&cmp),
        BTreeMap::from([
            ("a.jpg".to_owned(), (Category::Identical, vec![0, 1], false)),
            ("new.jpg".to_owned(), (Category::OnlyIn, vec![1], false)),
            (
                "sub/b.jpg".to_owned(),
                (Category::Different, vec![0, 1], false)
            ),
        ])
    );
    let a = all_rows(&cmp)
        .into_iter()
        .find(|r| r.key == "a.jpg")
        .unwrap();
    assert_eq!(a.cells[0].path.key().as_str(), "photos/a.jpg");
    assert_eq!(a.cells[1].path.key().as_str(), "DCIM/a.jpg");
    assert_eq!(a.cells[1].key.as_str(), "a.jpg");

    // One location against itself, through two subpaths.
    let same = fx.compare(&["disk:photos", "disk:photos2"], CompareOptions::default());
    assert_eq!(same.summary().unwrap().files(), 3);
}

#[test]
fn literal_percent_and_underscore_names_match() {
    let fx = Fixture::new();
    fx.eager("a", &[("100%.txt", "p"), ("a_b", "u"), ("dir_1/f", "d")]);
    fx.eager(
        "b",
        &[
            ("100%.txt", "p"),
            ("a_b", "u"),
            ("dir_1/f", "d"),
            ("dirX1/f", "nope"),
        ],
    );
    let cmp = fx.compare(&["a:dir_1", "b:dir_1"], CompareOptions::default());
    assert_eq!(classes(&cmp).len(), 1);
    let whole = classes(&fx.compare(&["a", "b"], CompareOptions::default()));
    assert_eq!(whole["100%25.txt"].0, Category::Identical);
    assert_eq!(whole["a_b"].0, Category::Identical);
}

#[test]
fn mixed_hash_algorithms_are_refused() {
    let fx = Fixture::new();
    fx.eager("a", &[("f", "x")]);
    fx.location(
        "b",
        HashPolicy::Eager,
        HashAlgo::Sha256,
        &[("f", "x", 1_000)],
    );
    let sides = [Side::location("a"), Side::location("b")];
    let err = Comparison::new(&fx.catalog, &sides, CompareOptions::default()).unwrap_err();
    assert!(matches!(&err, CompareError::AlgorithmMismatch(algos) if algos.len() == 2));
    assert_eq!(
        err.to_string(),
        "locations use different hash algorithms (a: blake3, b: sha256); rehash them with one algorithm first"
    );
}

#[test]
fn bad_inputs_are_refused() {
    let fx = Fixture::new();
    fx.eager("a", &[("f", "x")]);
    let options = CompareOptions::default;
    assert!(matches!(
        Comparison::new(&fx.catalog, &[Side::location("a")], options()),
        Err(CompareError::SideCount(1))
    ));
    assert!(matches!(
        Comparison::new(&fx.catalog, &[Side::location("a"), Side::location("nope")], options()),
        Err(CompareError::Catalog(CatalogError::LocationNotFound(label))) if label == "nope"
    ));
}

#[test]
fn rows_filter_by_category_and_page_stably() {
    let fx = Fixture::new();
    let many: Vec<(String, String)> = (0..7).map(|i| (format!("f{i}"), format!("c{i}"))).collect();
    let files: Vec<(&str, &str)> = many.iter().map(|(p, c)| (p.as_str(), c.as_str())).collect();
    fx.eager("a", &files);
    fx.eager("b", &[("f0", "c0"), ("f1", "changed")]);
    let cmp = fx.compare(&["a", "b"], CompareOptions::default());

    let only: Vec<String> = {
        let mut out = Vec::new();
        let mut page = Page::first(2);
        loop {
            let result = cmp
                .rows(&RowFilter::categories([Category::OnlyIn]), &page)
                .unwrap();
            out.extend(result.items.iter().map(|r| r.key.clone()));
            match result.next {
                Some(cursor) => page = Page::after(cursor, 2),
                None => break out,
            }
        }
    };
    assert_eq!(only, ["f2", "f3", "f4", "f5", "f6"]);
    let both = cmp
        .rows(
            &RowFilter::categories([Category::Identical, Category::Different]),
            &Page::first(10),
        )
        .unwrap();
    assert_eq!(
        both.items
            .iter()
            .map(|r| r.key.as_str())
            .collect::<Vec<_>>(),
        ["f0", "f1"]
    );
    assert_eq!(both.items[0].presence, SideSet::from_bits(0b11));
}
