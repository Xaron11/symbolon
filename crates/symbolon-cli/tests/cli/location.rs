//! `symbolon location add|list|remove`.

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
    let catalog = env.open_catalog();
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
        "label,root,kind,scannable,hash_policy,hash_algo,files,bytes,hashed_files,last_scan_status,last_scan_started_at,fingerprint\n"
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

/// A catalog lists private paths and hashes; directories created for it are owner-only (#86).
#[cfg(unix)]
#[test]
fn a_new_catalog_directory_is_private() {
    use std::os::unix::fs::PermissionsExt as _;
    let env = Env::new();
    let catalog = env.path("new/dir/catalog.db");
    env.ok(&["--catalog", catalog.to_str().unwrap(), "location", "list"]);
    for dir in [env.path("new"), env.path("new/dir")] {
        let mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700, "{}", dir.display());
    }
    let mode = std::fs::metadata(&catalog).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn set_renames_moves_and_changes_the_policy() {
    let env = two_locations();
    let json = |args: &[&str]| -> serde_json::Value {
        let run = env.run(args);
        assert_eq!(run.code, 0, "{args:?}: {}", run.stderr);
        serde_json::from_str(&run.stdout).unwrap()
    };

    let renamed = json(&["location", "set", "photos", "--label", "pictures", "--json"]);
    assert_eq!(renamed["locations"][0]["label"], "pictures");
    assert_eq!(renamed["locations"][0]["files"], 2, "the scan stays");

    let taken = env.run(&["location", "set", "pictures", "--label", "backup"]);
    assert_eq!(taken.code, 3);
    assert!(taken.stderr.contains("already exists"), "{}", taken.stderr);
    let invalid = env.run(&["location", "set", "pictures", "--label", "a:b"]);
    assert_eq!(invalid.code, 3);

    let eager = json(&["location", "set", "backup", "--policy", "eager", "--json"]);
    assert_eq!(eager["locations"][0]["hash_policy"], "eager");

    // A new root records the device found there.
    env.tree("usb/photos", &[("a.jpg", "aaaa")]);
    env.mount(&[("usb-1", "usb")]);
    let usb = env.path("usb/photos");
    let moved = json(&[
        "location",
        "set",
        "pictures",
        "--root",
        usb.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(moved["locations"][0]["root"], "[TMP]/usb/photos");
    assert_eq!(moved["locations"][0]["fingerprint"], "fs:usb-1:/photos");

    let missing = env.path("nowhere");
    let run = env.run(&[
        "location",
        "set",
        "pictures",
        "--root",
        missing.to_str().unwrap(),
    ]);
    assert_eq!(run.code, 3);
    assert!(run.stderr.contains("is not a directory"), "{}", run.stderr);

    assert_eq!(
        env.run(&["location", "set", "pictures"]).code,
        2,
        "nothing to change"
    );
}
