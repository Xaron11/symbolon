//! The folder tree of `Comparison` results: children, counts and per-folder totals.

#![cfg(feature = "local")]
#![expect(clippy::unwrap_used, reason = "test helpers fail loudly by design")]

use std::fs;

use symbolon_core::catalog::{Catalog, HashPolicy, NewLocation};
use symbolon_core::compare::{Category, CompareOptions, Comparison, RowFilter, Side, TreeChild};
use symbolon_core::pathkey::KeyOptions;
use symbolon_core::query::Page;
use symbolon_core::scan::{ScanOptions, Scanner};

struct Fixture {
    _dir: tempfile::TempDir,
    catalog: Catalog,
}

/// Left and right: `a/x.txt` different, `a/b/y.txt` only left, `a/b/z.txt` and `c.txt`
/// identical, `m/old.jpg` moved to `n/old.jpg`, `k` a file on the left but a folder holding
/// `k/f.txt` on the right, and 30 identical files in `big/`.
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let catalog = Catalog::open(dir.path().join("catalog.db")).unwrap();
    let left: &[(&str, &str)] = &[
        ("a/x.txt", "one"),
        ("a/b/y.txt", "only"),
        ("a/b/z.txt", "same"),
        ("c.txt", "same too"),
        ("m/old.jpg", "moving content"),
        ("k", "a file"),
    ];
    let right: &[(&str, &str)] = &[
        ("a/x.txt", "two!"),
        ("a/b/z.txt", "same"),
        ("c.txt", "same too"),
        ("n/old.jpg", "moving content"),
        ("k/f.txt", "inside"),
    ];
    for (label, files) in [("left", left), ("right", right)] {
        let root = dir.path().join(label);
        for (path, contents) in files {
            let path = root.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, contents).unwrap();
        }
        for i in 0..30 {
            fs::create_dir_all(root.join("big")).unwrap();
            fs::write(root.join(format!("big/f{i:02}.txt")), format!("{i}")).unwrap();
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

/// Each child as `name/` for a folder (with `+row` when it is also a row) or `name` for a row.
fn names(children: &[TreeChild]) -> Vec<String> {
    children
        .iter()
        .map(|child| match child {
            TreeChild::Row(row) => row.key.rsplit('/').next().unwrap().to_owned(),
            TreeChild::Folder { name, row, .. } => {
                format!("{name}/{}", if row.is_some() { "+row" } else { "" })
            }
            _ => unreachable!("unknown child"),
        })
        .collect()
}

fn children(cmp: &Comparison, filter: &RowFilter, parent: &str) -> Vec<TreeChild> {
    cmp.children(filter, parent, &Page::at(0, 100))
        .unwrap()
        .items
}

#[test]
fn root_children_list_folders_and_files() {
    let f = fixture();
    let cmp = compare(&f);
    let all = RowFilter::default();
    assert_eq!(
        names(&children(&cmp, &all, "")),
        ["a/", "big/", "c.txt", "k/+row", "m/"]
    );
    assert_eq!(cmp.child_count(&all, "").unwrap(), 5);
    assert_eq!(names(&children(&cmp, &all, "a")), ["b/", "x.txt"]);
    assert_eq!(names(&children(&cmp, &all, "a/b")), ["y.txt", "z.txt"]);
}

#[test]
fn filter_hides_folders_without_matches() {
    let f = fixture();
    let cmp = compare(&f);
    let different = RowFilter::categories([Category::Different]);
    // `k` is a different row (a kind clash) with nothing different beneath it.
    assert_eq!(names(&children(&cmp, &different, "")), ["a/", "k"]);
    assert_eq!(names(&children(&cmp, &different, "a")), ["x.txt"]);
    assert_eq!(cmp.child_count(&different, "a/b").unwrap(), 0);
}

#[test]
fn folder_totals_count_rows_and_bytes_below() {
    let f = fixture();
    let cmp = compare(&f);
    let all = RowFilter::default();
    let below_a = cmp.filtered_summary(&all.clone().prefix("a")).unwrap();
    let TreeChild::Folder { matching, .. } = &children(&cmp, &all, "")[0] else {
        panic!("a is a folder");
    };
    assert_eq!(matching.rows, 3);
    assert_eq!(
        matching.bytes,
        below_a.categories.values().map(|t| t.bytes).sum::<u64>()
    );
}

#[test]
fn folder_that_is_a_row_carries_it() {
    let f = fixture();
    let cmp = compare(&f);
    let root = children(&cmp, &RowFilter::default(), "");
    let Some(TreeChild::Folder {
        row: Some(row),
        matching,
        ..
    }) = root
        .iter()
        .find(|c| matches!(c, TreeChild::Folder { name, .. } if name == "k"))
    else {
        panic!("k is a folder and a row");
    };
    assert_eq!(row.category, Category::Different);
    assert_eq!(matching.rows, 1);
}

#[test]
fn moved_rows_sit_under_first_placement() {
    let f = fixture();
    let cmp = compare(&f);
    let moved = RowFilter::categories([Category::Moved]);
    assert_eq!(names(&children(&cmp, &moved, "")), ["m/"]);
    let in_m = children(&cmp, &moved, "m");
    let TreeChild::Row(row) = &in_m[0] else {
        panic!("a row");
    };
    assert_eq!(row.key, "m/old.jpg");
    assert_eq!(row.cells.len(), 2);
}

#[test]
fn count_matches_summary() {
    let f = fixture();
    let cmp = compare(&f);
    for filter in [
        RowFilter::default(),
        RowFilter::categories([Category::Identical]),
        RowFilter::default().prefix("a"),
    ] {
        let rows: u64 = cmp
            .filtered_summary(&filter)
            .unwrap()
            .categories
            .values()
            .map(|t| t.rows)
            .sum();
        assert_eq!(cmp.count(&filter).unwrap(), rows, "{filter:?}");
    }
}

#[test]
fn children_offsets_page() {
    let f = fixture();
    let cmp = compare(&f);
    let page = cmp
        .children(&RowFilter::default(), "big", &Page::at(10, 10))
        .unwrap();
    assert_eq!(
        names(&page.items),
        (10..20).map(|i| format!("f{i:02}.txt")).collect::<Vec<_>>()
    );
    assert!(page.next.is_none());
}

#[test]
fn folder_names_display_unescaped() {
    let dir = tempfile::tempdir().unwrap();
    let catalog = Catalog::open(dir.path().join("catalog.db")).unwrap();
    for label in ["left", "right"] {
        let root = dir.path().join(label);
        fs::create_dir_all(root.join("100%")).unwrap();
        fs::write(root.join("100%/a.txt"), label).unwrap();
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::ffi::OsStrExt as _;
            let odd = root.join(std::ffi::OsStr::from_bytes(b"dir\xe9"));
            fs::create_dir_all(&odd).unwrap();
            fs::write(odd.join("b.txt"), label).unwrap();
        }
        catalog
            .add_location(NewLocation::local(label, &root).hash_policy(HashPolicy::Eager))
            .unwrap();
        Scanner::start(&catalog, label, ScanOptions::default())
            .unwrap()
            .wait()
            .unwrap();
    }
    let cmp = Comparison::new(
        &catalog,
        &[Side::location("left"), Side::location("right")],
        CompareOptions::default(),
    )
    .unwrap();
    let shown: Vec<(String, String)> = children(&cmp, &RowFilter::default(), "")
        .into_iter()
        .map(|child| match child {
            TreeChild::Folder { key, display, .. } => (key, display),
            _ => panic!("only folders at the root"),
        })
        .collect();
    assert_eq!(shown[0], ("100%25".into(), "100%".into()));
    #[cfg(target_os = "linux")]
    {
        assert_eq!(shown[1], ("dir%E9".into(), "dir\u{FFFD}".into()));
        // The key, not the display text, opens the folder.
        assert_eq!(
            cmp.child_count(&RowFilter::default(), &shown[1].0).unwrap(),
            1
        );
    }
}

#[test]
fn folder_names_keep_their_case_when_matching_ignores_it() {
    let dir = tempfile::tempdir().unwrap();
    let catalog = Catalog::open(dir.path().join("catalog.db")).unwrap();
    for (label, path) in [
        ("left", "Docs/Inner/Read.ME"),
        ("right", "docs/inner/read.me"),
    ] {
        let root = dir.path().join(label);
        let file = root.join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, "same").unwrap();
        catalog
            .add_location(NewLocation::local(label, &root).hash_policy(HashPolicy::Eager))
            .unwrap();
        Scanner::start(&catalog, label, ScanOptions::default())
            .unwrap()
            .wait()
            .unwrap();
    }
    let ci = CompareOptions::default().keys(KeyOptions {
        case_insensitive: true,
        ..KeyOptions::default()
    });
    let cmp = Comparison::new(
        &catalog,
        &[Side::location("left"), Side::location("right")],
        ci,
    )
    .unwrap();
    let display = |parent: &str| match children(&cmp, &RowFilter::default(), parent).remove(0) {
        TreeChild::Folder { key, display, .. } => (key, display),
        _ => panic!("expected a folder"),
    };
    let (docs, shown) = display("");
    assert_eq!(shown, "Docs");
    let (_, inner) = display(&docs);
    assert_eq!(inner, "Inner");
}
