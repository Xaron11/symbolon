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
    let labels: Vec<&str> = cmp.sides().iter().map(|s| s.side.label.as_str()).collect();
    assert_eq!(labels, ["a", "b", "c"]);
    assert_eq!(cmp.options(), &CompareOptions::default());
    let fast = CompareOptions::default().assume_equal_within(Duration::from_secs(1));
    assert_eq!(fx.compare(&["a", "b"], fast.clone()).options(), &fast);
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

    // Without options the names differ, so equal content shows up as renamed files.
    let plain = classes(&fx.compare(&["a", "b"], CompareOptions::default()));
    assert_eq!(plain.len(), 2, "{plain:?}");
    assert!(
        plain
            .values()
            .all(|(category, _, _)| *category == Category::Renamed)
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
    // NFC and NFD spellings are distinct names on Linux and Windows, so this works everywhere.
    let fx = Fixture::new();
    fx.eager("a", &[("Cafe\u{301}", "1"), ("Café", "2")]);
    fx.eager("b", &[("Café", "1")]);
    let nfc = CompareOptions::default().keys(KeyOptions {
        nfc: true,
        ..KeyOptions::default()
    });
    let rows = all_rows(&fx.compare(&["a", "b"], nfc));
    assert_eq!(rows.len(), 1);
    assert_eq!(
        (rows[0].category, rows[0].cells.len()),
        (Category::Collision, 3)
    );
}

/// Case-insensitive filesystems (Windows, macOS) cannot hold `Readme` next to `README`.
#[cfg(target_os = "linux")]
#[test]
fn case_variants_on_one_side_are_collisions() {
    let fx = Fixture::new();
    fx.eager("a", &[("Readme", "1"), ("README", "2")]);
    fx.eager("b", &[("readme", "1")]);
    let ci = CompareOptions::default().keys(KeyOptions {
        case_insensitive: true,
        ..KeyOptions::default()
    });
    let rows = all_rows(&fx.compare(&["a", "b"], ci));
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

#[test]
fn same_content_at_another_path_is_moved_or_renamed() {
    let fx = Fixture::new();
    fx.eager(
        "a",
        &[
            ("photos/2024/a.jpg", "IMG"),
            ("music/song.flac", "SONG"),
            ("x/one.txt", "ONE"),
            ("keep", "K"),
        ],
    );
    fx.eager(
        "b",
        &[
            ("archive/a.jpg", "IMG"),
            ("music/song (1).flac", "SONG"),
            ("y/two.txt", "ONE"),
            ("keep", "K"),
        ],
    );
    let rows = all_rows(&fx.compare(&["a", "b"], CompareOptions::default()));
    let summary: BTreeMap<String, (Category, Vec<String>)> = rows
        .iter()
        .map(|r| {
            (
                r.key.clone(),
                (
                    r.category,
                    r.cells
                        .iter()
                        .map(|c| c.path.key().as_str().to_owned())
                        .collect(),
                ),
            )
        })
        .collect();
    assert_eq!(
        summary,
        BTreeMap::from([
            (
                "archive/a.jpg".to_owned(),
                (
                    Category::Moved,
                    vec!["photos/2024/a.jpg".to_owned(), "archive/a.jpg".to_owned()]
                )
            ),
            (
                "keep".to_owned(),
                (
                    Category::Identical,
                    vec!["keep".to_owned(), "keep".to_owned()]
                )
            ),
            (
                "music/song (1).flac".to_owned(),
                (
                    Category::Renamed,
                    vec![
                        "music/song.flac".to_owned(),
                        "music/song (1).flac".to_owned()
                    ]
                )
            ),
            (
                "x/one.txt".to_owned(),
                (
                    Category::Renamed,
                    vec!["x/one.txt".to_owned(), "y/two.txt".to_owned()]
                )
            ),
        ])
    );
    assert!(rows.iter().all(|r| r.presence == SideSet::from_bits(0b11)));
    let cmp = fx.compare(&["a", "b"], CompareOptions::default());
    assert_eq!(
        cmp.summary().unwrap().files(),
        8,
        "every file in exactly one row"
    );
}

#[test]
fn moves_across_three_sides_follow_disjoint_presence() {
    let fx = Fixture::new();
    fx.eager("a", &[("x/f", "F"), ("dup1", "D"), ("dup2", "D")]);
    fx.eager("b", &[("x/f", "F"), ("dup2", "D")]);
    fx.eager("c", &[("y/f", "F")]);
    let got = classes(&fx.compare(&["a", "b", "c"], CompareOptions::default()));
    assert_eq!(
        got["x/f"],
        (Category::Moved, vec![0, 1, 2], false),
        "A,B at x/f and C at y/f"
    );
    assert!(!got.contains_key("y/f"));
    // A holds an extra copy of content B also has at dup2: not a move.
    assert_eq!(got["dup1"], (Category::OnlyIn, vec![0], false));
    assert_eq!(got["dup2"], (Category::OnlyIn, vec![0, 1], false));
}

#[test]
fn duplicates_on_one_side_join_a_move_group() {
    let fx = Fixture::new();
    fx.eager("a", &[("p/a", "SAME"), ("q/a", "SAME")]);
    fx.eager("b", &[("r/a", "SAME")]);
    let rows = all_rows(&fx.compare(&["a", "b"], CompareOptions::default()));
    assert_eq!(rows.len(), 1);
    assert_eq!(
        (rows[0].category, rows[0].cells.len()),
        (Category::Moved, 3)
    );
}

#[test]
fn unhashed_or_different_files_are_never_matched_as_moves() {
    let fx = Fixture::new();
    fx.location(
        "a",
        HashPolicy::Lazy,
        HashAlgo::Blake3,
        &[("old/f", "SAME", 1_000)],
    );
    fx.location(
        "b",
        HashPolicy::Lazy,
        HashAlgo::Blake3,
        &[("new/f", "SAME", 1_000)],
    );
    let lazy = classes(&fx.compare(&["a", "b"], CompareOptions::default()));
    assert_eq!(
        lazy["old/f"].0,
        Category::OnlyIn,
        "no digests yet, so no move"
    );
    assert_eq!(lazy["new/f"].0, Category::OnlyIn);

    fx.eager("c", &[("old/f", "one")]);
    fx.eager("d", &[("new/f", "two")]);
    let different = classes(&fx.compare(&["c", "d"], CompareOptions::default()));
    assert_eq!(different.len(), 2);
    assert!(
        different
            .values()
            .all(|(category, _, _)| *category == Category::OnlyIn)
    );
}

mod resolve {
    use std::ops::ControlFlow;

    use symbolon_core::compare::ResolveEvent;
    use symbolon_core::pathkey::RelPath;
    use symbolon_core::query::{HashState, Query};

    use super::*;

    const BIG: usize = 200 * 1024;

    fn lazy(fx: &Fixture, label: &str, files: &[(&str, String)]) {
        let files: Vec<(&str, &str, u64)> =
            files.iter().map(|(p, c)| (*p, c.as_str(), 1_000)).collect();
        fx.location(label, HashPolicy::Lazy, HashAlgo::Blake3, &files);
    }

    fn state(fx: &Fixture, label: &str, path: &str) -> HashState {
        let query = Query::open(&fx.catalog).unwrap();
        query
            .entry(label, &RelPath::from_path(path.as_ref()).unwrap())
            .unwrap()
            .unwrap()
            .hash_state
    }

    fn resolve(cmp: &mut Comparison) -> symbolon_core::compare::ResolveSummary {
        cmp.resolve(&ScanOptions::default(), |_| ControlFlow::Continue(()))
            .unwrap()
    }

    fn big(fill: u8, tail: u8) -> String {
        let mut text = vec![fill; BIG];
        text[BIG - 1] = tail;
        String::from_utf8(text).unwrap()
    }

    #[test]
    fn only_deciding_files_are_hashed_and_unique_sizes_never_read() {
        let fx = Fixture::new();
        lazy(
            &fx,
            "a",
            &[
                ("same", "aaaa".into()),
                ("diff", "aaaa".into()),
                ("unique_a", "1234567".into()),
            ],
        );
        lazy(
            &fx,
            "b",
            &[
                ("same", "aaaa".into()),
                ("diff", "bbbb".into()),
                ("unique_b", "123456789".into()),
            ],
        );
        let mut cmp = fx.compare(&["a", "b"], CompareOptions::default());
        assert_eq!(classes(&cmp)["same"].0, Category::Unverified);

        let needs = cmp.hash_needs().unwrap();
        let listed: u64 = needs.iter().map(|n| n.full_files + n.partial_files).sum();
        assert_eq!(listed, 4, "same and diff on both sides: {needs:?}");

        let summary = resolve(&mut cmp);
        assert_eq!(
            (summary.hashed_files, summary.hashed_bytes, summary.warnings),
            (4, 16, 0)
        );
        let got = classes(&cmp);
        assert_eq!(got["same"].0, Category::Identical);
        assert_eq!(got["diff"].0, Category::Different);
        assert_eq!(state(&fx, "a", "unique_a"), HashState::Unhashed);
        assert_eq!(state(&fx, "b", "unique_b"), HashState::Unhashed);
        assert_eq!(cmp.hash_needs().unwrap(), Vec::new());
    }

    #[test]
    fn partial_digests_settle_large_differences_without_full_reads() {
        let fx = Fixture::new();
        lazy(
            &fx,
            "a",
            &[("differs", big(b'x', b'1')), ("equal", big(b'y', b'2'))],
        );
        lazy(
            &fx,
            "b",
            &[("differs", big(b'x', b'9')), ("equal", big(b'y', b'2'))],
        );
        let mut cmp = fx.compare(&["a", "b"], CompareOptions::default());
        let first = cmp.hash_needs().unwrap();
        assert!(
            first
                .iter()
                .all(|n| n.full_files == 0 && n.partial_files == 2),
            "{first:?}"
        );

        let summary = resolve(&mut cmp);
        assert_eq!(
            summary.rounds, 2,
            "partial round, then full round for the still-equal pair"
        );
        // Round 1: four partial digests of 128 KiB; round 2: two full reads of the equal pair.
        assert_eq!(summary.hashed_files, 6);
        assert_eq!(summary.hashed_bytes, 4 * 128 * 1024 + 2 * BIG as u64);
        let got = classes(&cmp);
        assert_eq!(got["differs"].0, Category::Different);
        assert_eq!(got["equal"].0, Category::Identical);
        assert_eq!(
            state(&fx, "a", "differs"),
            HashState::Partial,
            "the tail differs, so no full read"
        );
        assert_eq!(state(&fx, "a", "equal"), HashState::Verified);
    }

    #[test]
    fn moves_appear_once_candidates_are_hashed() {
        let fx = Fixture::new();
        lazy(
            &fx,
            "a",
            &[("old/photo.jpg", "PHOTO".into()), ("only_a", "x".into())],
        );
        lazy(
            &fx,
            "b",
            &[
                ("new/photo.jpg", "PHOTO".into()),
                ("only_b", "yyyyyy".into()),
            ],
        );
        let mut cmp = fx.compare(&["a", "b"], CompareOptions::default());
        assert_eq!(classes(&cmp)["old/photo.jpg"].0, Category::OnlyIn);
        resolve(&mut cmp);
        let got = classes(&cmp);
        assert_eq!(got["new/photo.jpg"].0, Category::Moved);
        assert_eq!(
            state(&fx, "a", "only_a"),
            HashState::Unhashed,
            "no size partner on b"
        );
    }

    #[test]
    fn resolving_lazily_matches_comparing_eagerly_hashed_copies() {
        let files_a = [
            ("x/1", "one"),
            ("x/2", "two"),
            ("moved/a", "AAAA"),
            ("same", "SAME"),
            ("d", "left"),
        ];
        let files_b = [
            ("x/1", "one"),
            ("x/2", "TWO"),
            ("elsewhere/a", "AAAA"),
            ("same", "SAME"),
            ("d", "rite"),
        ];
        let fx = Fixture::new();
        lazy(&fx, "a", &files_a.map(|(p, c)| (p, c.to_owned())));
        lazy(&fx, "b", &files_b.map(|(p, c)| (p, c.to_owned())));
        let mut lazily = fx.compare(&["a", "b"], CompareOptions::default());
        resolve(&mut lazily);

        let eager = Fixture::new();
        eager.eager("a", &files_a);
        eager.eager("b", &files_b);
        assert_eq!(
            classes(&lazily),
            classes(&eager.compare(&["a", "b"], CompareOptions::default()))
        );
    }

    #[test]
    fn unreachable_locations_leave_rows_unverified() {
        let fx = Fixture::new();
        lazy(&fx, "a", &[("f", "same".into())]);
        lazy(&fx, "b", &[("f", "same".into())]);
        fs::remove_dir_all(fx.root("b")).unwrap();
        let mut cmp = fx.compare(&["a", "b"], CompareOptions::default());
        let mut unreachable = Vec::new();
        let summary = cmp
            .resolve(&ScanOptions::default(), |event| {
                if let ResolveEvent::Unreachable { label, .. } = event {
                    unreachable.push(label);
                }
                ControlFlow::Continue(())
            })
            .unwrap();
        assert_eq!(unreachable, ["b"]);
        assert_eq!(summary.unreachable, ["b"]);
        assert_eq!(classes(&cmp)["f"].0, Category::Unverified);
    }

    #[test]
    fn files_that_cannot_be_hashed_are_counted_and_reported() {
        let fx = Fixture::new();
        lazy(&fx, "a", &[("f", "same".into()), ("g", "also".into())]);
        lazy(&fx, "b", &[("f", "same".into()), ("g", "also".into())]);
        fs::remove_file(fx.root("b").join("g")).unwrap();
        let mut cmp = fx.compare(&["a", "b"], CompareOptions::default());
        let mut warned = Vec::new();
        let summary = cmp
            .resolve(&ScanOptions::default(), |event| {
                if let ResolveEvent::Warning { warning, .. } = event {
                    warned.push(warning.path.unwrap().key().as_str().to_owned());
                }
                ControlFlow::Continue(())
            })
            .unwrap();
        assert_eq!(warned, ["g"]);
        assert_eq!((summary.warnings, summary.hashed_files), (1, 3));
        let got = classes(&cmp);
        assert_eq!(got["f"].0, Category::Identical);
        assert_eq!(
            got["g"].0,
            Category::Unverified,
            "b's copy vanished before it could be hashed"
        );
    }

    #[test]
    fn progress_events_report_running_totals() {
        let fx = Fixture::new();
        lazy(&fx, "a", &[("f", "aaaa".into()), ("g", "bbbbbb".into())]);
        lazy(&fx, "b", &[("f", "aaaa".into()), ("g", "bbbbbb".into())]);
        let mut cmp = fx.compare(&["a", "b"], CompareOptions::default());
        let mut options = ScanOptions::default();
        options.progress_interval = Duration::ZERO;
        let mut last = BTreeMap::new();
        cmp.resolve(&options, |event| {
            if let ResolveEvent::Progress {
                label,
                files,
                bytes,
            } = event
            {
                let previous: (u64, u64) = last.get(&label).copied().unwrap_or_default();
                assert!(
                    files >= previous.0 && bytes >= previous.1,
                    "totals never go down"
                );
                last.insert(label, (files, bytes));
            }
            ControlFlow::Continue(())
        })
        .unwrap();
        assert_eq!(last.get("a"), Some(&(2, 10)));
        assert_eq!(last.get("b"), Some(&(2, 10)));
    }

    #[test]
    fn the_callback_can_stop_resolving() {
        let fx = Fixture::new();
        lazy(&fx, "a", &[("f", "same".into())]);
        lazy(&fx, "b", &[("f", "same".into())]);
        let mut cmp = fx.compare(&["a", "b"], CompareOptions::default());
        let summary = cmp
            .resolve(&ScanOptions::default(), |_| ControlFlow::Break(()))
            .unwrap();
        assert!(summary.cancelled);
        assert_eq!(classes(&cmp)["f"].0, Category::Unverified);
    }

    #[test]
    fn assumed_rows_need_no_hashing() {
        let fx = Fixture::new();
        lazy(&fx, "a", &[("f", "same".into())]);
        lazy(&fx, "b", &[("f", "same".into())]);
        let fast = fx.compare(
            &["a", "b"],
            CompareOptions::default().assume_equal_within(Duration::ZERO),
        );
        assert_eq!(fast.hash_needs().unwrap(), Vec::new());
    }
}

mod filters_and_duplicates {
    use std::ops::ControlFlow;

    use super::*;

    fn keys(cmp: &Comparison, filter: &RowFilter) -> Vec<String> {
        cmp.rows(filter, &Page::first(100))
            .unwrap()
            .items
            .into_iter()
            .map(|r| r.key)
            .collect()
    }

    #[test]
    fn rows_filter_by_extension_size_prefix_and_presence() {
        let fx = Fixture::new();
        fx.eager(
            "a",
            &[
                ("Photos/a.JPG", "1"),
                ("Photos/b.jpg", "2222"),
                ("docs/c.pdf", "333"),
                ("docs/x.jpg", "x"),
            ],
        );
        fx.eager("b", &[("Photos/a.JPG", "1"), ("docs/x.jpg", "x")]);
        fx.eager(
            "c",
            &[
                ("Photos/a.JPG", "1"),
                ("docs/x.jpg", "y"),
                ("Photos/b.jpg", "2222"),
            ],
        );
        let ci = KeyOptions {
            case_insensitive: true,
            ..KeyOptions::default()
        };
        let cmp = fx.compare(&["a", "b", "c"], CompareOptions::default().keys(ci));

        assert_eq!(
            keys(&cmp, &RowFilter::default().extension(".JPG")),
            ["docs/x.jpg", "photos/a.jpg", "photos/b.jpg"]
        );
        assert_eq!(
            keys(&cmp, &RowFilter::default().size_range(Some(2), Some(4))),
            ["docs/c.pdf", "photos/b.jpg"]
        );
        assert_eq!(
            keys(&cmp, &RowFilter::default().prefix("PHOTOS")),
            ["photos/a.jpg", "photos/b.jpg"],
            "normalized like keys"
        );
        assert_eq!(
            keys(
                &cmp,
                &RowFilter::default().presence(SideSet::from_bits(0b101))
            ),
            ["photos/b.jpg"]
        );
        let combined = RowFilter::categories([Category::Different])
            .extension("jpg")
            .prefix("docs");
        assert_eq!(keys(&cmp, &combined), ["docs/x.jpg"]);

        let summary = cmp
            .filtered_summary(&RowFilter::default().extension("jpg"))
            .unwrap();
        assert_eq!(summary.files(), 2 + 3 + 3);
        assert_eq!(summary.get(Category::Identical).rows, 1);
        assert_eq!(cmp.summary().unwrap().files(), 9);
    }

    #[test]
    fn duplicates_are_listed_per_side_without_empty_files() {
        let fx = Fixture::new();
        fx.eager(
            "a",
            &[
                ("p/x", "copy"),
                ("q/x", "copy"),
                ("r/x", "copy"),
                ("unique", "u"),
                ("e1", ""),
                ("e2", ""),
                ("out/x", "copy"),
            ],
        );
        fx.eager("b", &[("one", "pair"), ("two", "pair")]);
        let cmp = fx.compare(&["a:", "b"], CompareOptions::default());

        let a = cmp.duplicates(0, &Page::first(10)).unwrap();
        assert_eq!(a.items.len(), 1);
        let keys: Vec<_> = a.items[0]
            .cells
            .iter()
            .map(|c| c.path.key().as_str().to_owned())
            .collect();
        assert_eq!(keys, ["out/x", "p/x", "q/x", "r/x"]);
        assert_eq!(a.items[0].redundant_bytes(), 3 * 4);

        let totals = cmp.duplicate_totals().unwrap();
        assert_eq!(
            (totals[0].groups, totals[0].files, totals[0].redundant_bytes),
            (1, 4, 12)
        );
        assert_eq!(
            (totals[1].groups, totals[1].files, totals[1].redundant_bytes),
            (1, 2, 4)
        );

        // Scope: a subpath only sees its own copies.
        let scoped = fx.compare(&["a:p", "b"], CompareOptions::default());
        assert_eq!(scoped.duplicate_totals().unwrap()[0].groups, 0);
    }

    #[test]
    fn duplicates_of_lazy_locations_are_found_after_resolving() {
        let fx = Fixture::new();
        let files = [
            ("d1/a", "SAME", 1_000),
            ("d2/a", "SAME", 1_000),
            ("other", "DIFF", 1_000),
            ("big", "bigger file", 1_000),
        ];
        fx.location("a", HashPolicy::Lazy, HashAlgo::Blake3, &files);
        fx.location(
            "b",
            HashPolicy::Lazy,
            HashAlgo::Blake3,
            &[("z", "z", 1_000)],
        );

        let without = fx.compare(&["a", "b"], CompareOptions::default());
        assert_eq!(
            without.hash_needs().unwrap(),
            Vec::new(),
            "duplicates were not asked for"
        );

        let mut with = fx.compare(&["a", "b"], CompareOptions::default().duplicates(true));
        let needs = with.hash_needs().unwrap();
        assert_eq!(
            needs.iter().map(|n| n.full_files).sum::<u64>(),
            3,
            "the three 4-byte files: {needs:?}"
        );
        with.resolve(&ScanOptions::default(), |_| ControlFlow::Continue(()))
            .unwrap();
        let sets = with.duplicates(0, &Page::first(10)).unwrap().items;
        assert_eq!(sets.len(), 1);
        assert_eq!(sets[0].cells.len(), 2);
    }

    #[test]
    fn duplicate_pages_follow_cursors() {
        let fx = Fixture::new();
        let files: Vec<(String, String)> = (0..5)
            .flat_map(|g| {
                [
                    (format!("g{g}/1"), format!("group{g}")),
                    (format!("g{g}/2"), format!("group{g}")),
                ]
            })
            .collect();
        let refs: Vec<(&str, &str)> = files
            .iter()
            .map(|(p, c)| (p.as_str(), c.as_str()))
            .collect();
        fx.eager("a", &refs);
        fx.eager("b", &[("z", "z")]);
        let cmp = fx.compare(&["a", "b"], CompareOptions::default());
        let mut seen = 0;
        let mut page = Page::first(2);
        loop {
            let result = cmp.duplicates(0, &page).unwrap();
            seen += result.items.len();
            match result.next {
                Some(cursor) => page = Page::after(cursor, 2),
                None => break,
            }
        }
        assert_eq!(seen, 5);
    }
}
