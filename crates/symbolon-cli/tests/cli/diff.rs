//! `symbolon diff`.

use serde_json::Value;

use crate::common::{Env, settings};

const A: &[(&str, &str)] = &[
    ("same.txt", "same"),
    ("changed.txt", "one"),
    ("only_a.txt", "just here"),
    ("x/photo.jpg", "pixels"),
    ("old.txt", "renamed content"),
];
const B: &[(&str, &str)] = &[
    ("same.txt", "same"),
    ("changed.txt", "two!"),
    ("y/photo.jpg", "pixels"),
    ("new.txt", "renamed content"),
];

/// Locations `a` and `b` (scanned) with one row of most categories.
fn env(policy: &str) -> Env {
    let env = Env::new();
    for (label, files) in [("a", A), ("b", B)] {
        let root = env.tree(label, files);
        env.ok(&[
            "location",
            "add",
            label,
            root.to_str().unwrap(),
            "--policy",
            policy,
        ]);
        env.ok(&["scan", label, "--json"]);
    }
    env
}

fn json(env: &Env, args: &[&str]) -> Value {
    let run = env.run(&[&["diff", "--json"], args].concat());
    assert!(run.code == 0 || run.code == 1, "{args:?}: {}", run.stderr);
    serde_json::from_str(&run.stdout).unwrap()
}

/// `(category, key)` of every listed row.
fn rows(value: &Value) -> Vec<(String, String)> {
    value["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            (
                row["category"].as_str().unwrap().to_owned(),
                row["key"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

#[test]
fn every_format_shows_the_differences() {
    let env = env("eager");
    let run = env.run(&["diff", "a", "b"]);
    assert_eq!(run.code, 1, "differences found");
    settings(true).bind(|| insta::assert_snapshot!("table", run.stdout));
    settings(false).bind(|| {
        insta::assert_snapshot!("json", env.run(&["diff", "a", "b", "--json"]).stdout);
        insta::assert_snapshot!(
            "csv",
            env.run(&["diff", "a", "b", "--format", "csv"]).stdout
        );
    });
}

#[test]
fn identical_folders_exit_zero() {
    let env = env("eager");
    let run = env.run(&["diff", "a:x", "b:y"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let all = json(&env, &["a:x", "b:y", "--all"]);
    assert_eq!(
        rows(&all),
        [("identical".to_owned(), "photo.jpg".to_owned())]
    );
    assert_eq!(all["sides"][0]["side"], "a:x");
}

#[test]
fn filters_narrow_the_listing_but_not_the_summary() {
    let env = env("eager");
    let only = json(&env, &["a", "b", "--only", "moved,renamed"]);
    assert_eq!(
        rows(&only),
        [
            ("renamed".to_owned(), "new.txt".to_owned()),
            ("moved".to_owned(), "x/photo.jpg".to_owned())
        ]
    );
    assert_eq!(
        only["summary"]["different"]["rows"], 1,
        "the summary covers everything"
    );

    assert_eq!(rows(&json(&env, &["a", "b", "--ext", "jpg"])).len(), 1);
    assert_eq!(rows(&json(&env, &["a", "b", "--under", "x"])).len(), 1);
    assert_eq!(rows(&json(&env, &["a", "b", "--min-size", "9"])).len(), 2);
    assert_eq!(rows(&json(&env, &["a", "b", "--all"])).len(), 5);
    assert!(json(&env, &["a", "b", "--summary"]).get("rows").is_none());

    settings(true).bind(|| {
        insta::assert_snapshot!(
            "summary_table",
            env.run(&["diff", "a", "b", "--summary"]).stdout
        );
    });
    settings(false).bind(|| {
        insta::assert_snapshot!(
            "summary_csv",
            env.run(&["diff", "a", "b", "--summary", "--format", "csv"])
                .stdout
        );
    });
}

#[test]
fn lazy_locations_are_hashed_on_demand_unless_told_not_to() {
    let eager = json(&env("eager"), &["a", "b"]);
    let lazy = env("lazy");
    let unhashed = json(&lazy, &["a", "b", "--no-hash"]);
    assert_eq!(
        unhashed["summary"]["unverified"]["rows"], 1,
        "same.txt has equal sizes"
    );
    assert_eq!(json(&lazy, &["a", "b"])["summary"], eager["summary"]);

    let fast = json(&env("lazy"), &["a", "b", "--no-hash", "--fast", "1h"]);
    assert_eq!(fast["summary"]["identical"]["assumed_rows"], 1);
}

#[test]
fn three_sides_show_where_each_file_is() {
    let env = env("eager");
    let c = env.tree("c", &[("same.txt", "same")]);
    env.ok(&[
        "location",
        "add",
        "c",
        c.to_str().unwrap(),
        "--policy",
        "eager",
    ]);
    env.ok(&["scan", "c", "--json"]);
    settings(true).bind(|| {
        insta::assert_snapshot!(
            env.run(&["diff", "a", "b", "c", "--only", "only-in,identical"])
                .stdout
        );
    });
}

#[test]
fn unicode_spellings_collide_with_nfc() {
    let env = Env::new();
    let a = env.tree("a", &[("café", "1"), ("cafe\u{301}", "2")]);
    let b = env.tree("b", &[("café", "1")]);
    for (label, root) in [("a", &a), ("b", &b)] {
        env.ok(&[
            "location",
            "add",
            label,
            root.to_str().unwrap(),
            "--policy",
            "eager",
        ]);
        env.ok(&["scan", label, "--json"]);
    }
    let categories: Vec<String> = rows(&json(&env, &["a", "b", "--nfc"]))
        .into_iter()
        .map(|(c, _)| c)
        .collect();
    assert_eq!(categories, ["collision"]);
}

#[test]
fn bad_sides_fail_clearly() {
    let env = env("eager");
    let xxh = env.tree("x3", &[]);
    env.ok(&[
        "location",
        "add",
        "x3",
        xxh.to_str().unwrap(),
        "--algo",
        "xxh3",
    ]);

    let mixed = env.run(&["diff", "a", "x3"]);
    assert_eq!(mixed.code, 3);
    assert!(
        mixed
            .stderr
            .contains("different hash algorithms (a: blake3, x3: xxh3)"),
        "{}",
        mixed.stderr
    );

    let unknown = env.run(&["diff", "a", "nope"]);
    assert_eq!(
        (unknown.code, unknown.stderr.as_str()),
        (3, "error: no location labelled \"nope\"\n")
    );

    assert_eq!(env.run(&["diff", "a", "../b"]).code, 3);
    assert_eq!(env.run(&["diff", "a"]).code, 2, "two sides at least");
    assert_eq!(env.run(&["diff", "a", "b", "--only", "dupes"]).code, 2);
    assert_eq!(env.run(&["diff", "a", "b", "--fast", "soon"]).code, 2);
}

/// File names come from whoever wrote the scanned tree; tables must not pass their control
/// characters to the terminal (#83). Windows does not allow them in names.
#[cfg(unix)]
#[test]
fn control_characters_in_names_are_escaped_in_tables() {
    let evil = "evil\u{1b}]0;title\u{7}\u{1b}[2K";
    let env = Env::new();
    let a = env.tree("a", &[(evil, "x")]);
    let b = env.tree("b", &[]);
    env.ok(&["location", "add", "a", a.to_str().unwrap()]);
    env.ok(&["location", "add", "b", b.to_str().unwrap()]);
    env.ok(&["scan", "a", "b", "--json"]);

    let table = env.run(&["diff", "a", "b"]);
    assert!(!table.stdout.contains('\u{1b}'), "{:?}", table.stdout);
    assert!(
        table.stdout.contains("evil%1B]0;title%07%1B[2K"),
        "{:?}",
        table.stdout
    );
    let json = env.run(&["diff", "a", "b", "--json"]);
    assert!(!json.stdout.contains('\u{1b}'), "{:?}", json.stdout);
}

/// A location: label, files `(path, contents)`, symlinks `(path, target)` and empty
/// directories.
#[cfg(unix)]
type Tree<'a> = (
    &'a str,
    &'a [(&'a str, &'a str)],
    &'a [(&'a str, &'a [u8])],
    &'a [&'a str],
);

/// Locations `a` and `b`, scanned.
#[cfg(unix)]
fn env_with_links(sides: [Tree<'_>; 2]) -> Env {
    use std::os::unix::ffi::OsStrExt as _;

    let env = Env::new();
    for (label, files, links, dirs) in sides {
        let root = env.tree(label, files);
        for (path, target) in links {
            std::os::unix::fs::symlink(std::ffi::OsStr::from_bytes(target), root.join(path))
                .unwrap();
        }
        for dir in dirs {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        env.ok(&["location", "add", label, root.to_str().unwrap()]);
    }
    env.ok(&["scan", "a", "b", "--json"]);
    env
}

#[cfg(unix)]
fn dirs_and_links() -> Env {
    env_with_links([
        ("a", &[("notes", "a file")], &[("l", b"f.txt")], &["empty"]),
        ("b", &[("notes/n.txt", "n")], &[("l", b"g.txt")], &[]),
    ])
}

#[cfg(unix)]
#[test]
fn dirs_and_symlinks_table() {
    let env = dirs_and_links();
    let run = env.run(&["diff", "a", "b"]);
    assert_eq!(run.code, 1);
    settings(true).bind(|| insta::assert_snapshot!("dirs_and_symlinks_table", run.stdout));
}

#[cfg(unix)]
#[test]
fn diff_filters_by_kind() {
    let env = dirs_and_links();
    assert_eq!(
        rows(&json(&env, &["a", "b", "--kind", "symlink"])),
        [("different".to_owned(), "l".to_owned())]
    );
    assert_eq!(
        rows(&json(&env, &["a", "b", "--kind", "dir,symlink"])),
        [
            ("only_in".to_owned(), "empty".to_owned()),
            ("different".to_owned(), "l".to_owned()),
            ("different".to_owned(), "notes".to_owned()),
        ]
    );
}

/// Link targets come from whoever made the tree, like names (#83).
#[cfg(unix)]
#[test]
fn link_targets_are_escaped() {
    let env = env_with_links([
        (
            "a",
            &[],
            &[("evil", b"\x1b[31mred"), ("odd", b"a\xffb")],
            &[],
        ),
        ("b", &[], &[], &[]),
    ]);
    let table = env.run(&["diff", "a", "b"]);
    assert!(!table.stdout.contains('\u{1b}'), "{:?}", table.stdout);
    assert!(table.stdout.contains("-> %1B[31mred"), "{:?}", table.stdout);
    assert!(table.stdout.contains("-> a%FFb"), "{:?}", table.stdout);
    let json = env.run(&["diff", "a", "b", "--json"]);
    assert!(!json.stdout.contains('\u{1b}'), "{:?}", json.stdout);
    assert!(
        json.stdout.contains(r#""target": "a%FFb""#),
        "{}",
        json.stdout
    );
    let csv = env.run(&["diff", "a", "b", "--format", "csv"]);
    assert!(csv.stdout.contains(",symlink,"), "{}", csv.stdout);
    assert!(csv.stdout.contains("a%FFb"), "{}", csv.stdout);
}
