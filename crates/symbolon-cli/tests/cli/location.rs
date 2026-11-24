//! `symbolon location add|list|remove`.

use symbolon_core::catalog::Catalog;
use symbolon_core::scan::{ScanOptions, Scanner};

use crate::common::{Env, settings};

fn two_locations() -> Env {
    let env = Env::new();
    let photos = env.tree("photos", &[("a.jpg", "aaaa"), ("trip/b.jpg", "bb")]);
    let backup = env.tree("backup", &[]);
    env.ok(&[
        "location",
        "add",
        "photos",
        photos.to_str().unwrap(),
        "--policy",
        "eager",
    ]);
    env.ok(&[
        "location",
        "add",
        "backup",
        backup.to_str().unwrap(),
        "--algo",
        "xxh3",
        "--ignore",
        "*.tmp",
    ]);
    let catalog = Catalog::open(&env.catalog).unwrap();
    Scanner::start(&catalog, "photos", ScanOptions::default())
        .unwrap()
        .wait()
        .unwrap();
    env
}

#[test]
fn list_in_every_format() {
    let env = two_locations();
    settings(true).bind(|| {
        insta::assert_snapshot!("list_table", env.ok(&["location", "list"]));
    });
    settings(false).bind(|| {
        insta::assert_snapshot!("list_json", env.ok(&["location", "list", "--json"]));
        insta::assert_snapshot!("list_csv", env.ok(&["location", "list", "--format", "csv"]));
    });
}

#[test]
fn add_prints_the_new_location() {
    let env = Env::new();
    let docs = env.tree("docs", &[]);
    let out = env.ok(&[
        "location",
        "add",
        "docs",
        docs.to_str().unwrap(),
        "--format",
        "json",
    ]);
    settings(false).bind(|| insta::assert_snapshot!(out));
}

#[test]
fn an_empty_catalog_lists_nothing_and_hints_on_stderr() {
    let env = Env::new();
    let run = env.run(&["location", "list"]);
    assert_eq!((run.code, run.stdout.as_str()), (0, ""));
    assert!(
        run.stderr.contains("symbolon location add"),
        "{}",
        run.stderr
    );
    assert_eq!(env.run(&["location", "list", "--quiet"]).stderr, "");
    assert_eq!(
        env.ok(&["location", "list", "--json"]),
        "{\n  \"locations\": []\n}\n"
    );
    assert_eq!(
        env.ok(&["location", "list", "--format", "csv"]),
        "label,root,scannable,hash_policy,hash_algo,files,bytes,hashed_files,last_scan_status,last_scan_started_at\n"
    );
}

#[test]
fn bad_input_fails_with_clear_errors() {
    let env = two_locations();
    let photos = env.path("photos");
    let photos = photos.to_str().unwrap();

    let taken = env.run(&["location", "add", "photos", photos]);
    assert_eq!(taken.code, 3);
    assert_eq!(
        taken.stderr,
        "error: a location labelled \"photos\" already exists\n"
    );

    let missing = env.run(&[
        "location",
        "add",
        "other",
        env.path("nope").to_str().unwrap(),
    ]);
    assert_eq!(missing.code, 3);
    assert!(
        missing.stderr.contains("is not a directory"),
        "{}",
        missing.stderr
    );

    let unknown = env.run(&["location", "remove", "nope", "--yes"]);
    assert_eq!(unknown.code, 3);
    assert_eq!(unknown.stderr, "error: no location labelled \"nope\"\n");

    assert_eq!(
        env.run(&["location", "add", "x"]).code,
        2,
        "missing argument"
    );
    assert_eq!(
        env.run(&["location", "list", "--json", "--format", "csv"])
            .code,
        2
    );
    assert_eq!(
        env.run(&["location", "add", "x", photos, "--policy", "sometimes"])
            .code,
        2
    );
}

#[test]
fn remove_needs_confirmation_without_a_terminal() {
    let env = two_locations();
    let refused = env.run(&["location", "remove", "photos"]);
    assert_eq!(refused.code, 4);
    assert!(refused.stderr.contains("pass --yes"), "{}", refused.stderr);

    settings(true).bind(|| {
        insta::assert_snapshot!(env.ok(&["location", "remove", "photos", "--yes"]));
    });
    let labels = env.ok(&["location", "list", "--format", "csv"]);
    assert_eq!(labels.lines().count(), 2, "only backup is left: {labels}");
    assert!(env.path("photos/a.jpg").exists(), "files are never touched");
}

#[test]
fn catalog_option_overrides_the_environment() {
    let env = two_locations();
    let other = env.path("other.db");
    let out = env.ok(&[
        "--catalog",
        other.to_str().unwrap(),
        "location",
        "list",
        "--json",
    ]);
    assert_eq!(out, "{\n  \"locations\": []\n}\n");
    assert!(other.exists(), "the catalog is created on first use");
}
