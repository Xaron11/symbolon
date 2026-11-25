//! `symbolon export` and `symbolon import`.

use serde_json::Value;

use crate::common::{Env, settings};

/// A laptop catalog with a scanned `photos` location.
fn laptop() -> Env {
    let env = Env::new();
    let root = env.tree("photos", &[("a.jpg", "aaaa"), ("b.jpg", "bb")]);
    env.ok(&[
        "location",
        "add",
        "photos",
        root.to_str().unwrap(),
        "--policy",
        "eager",
    ]);
    env.ok(&["scan", "photos", "--json"]);
    env
}

/// Runs against a second catalog in the same temporary directory.
fn desktop(env: &Env, args: &[&str]) -> crate::common::Run {
    let catalog = env.path("desktop.db");
    env.run(&[&["--catalog", catalog.to_str().unwrap()], args].concat())
}

#[test]
fn export_then_import_elsewhere() {
    let env = laptop();
    let file = env.path("photos.symbolon");
    let file = file.to_str().unwrap();
    settings(false).bind(|| {
        insta::assert_snapshot!(
            "export",
            env.ok(&["export", "photos", "-o", file, "--json"])
        );
    });

    let imported = desktop(&env, &["import", file, "--json"]);
    assert_eq!(imported.code, 0, "{}", imported.stderr);
    settings(false).bind(|| insta::assert_snapshot!("import", imported.stdout));

    let list: Value =
        serde_json::from_str(&desktop(&env, &["location", "list", "--json"]).stdout).unwrap();
    let location = &list["locations"][0];
    assert_eq!(
        (location["kind"].as_str(), location["scannable"].as_bool()),
        (Some("imported"), Some(false))
    );
    assert_eq!(location["files"], 2);

    let scan = desktop(&env, &["scan", "photos"]);
    assert_eq!(scan.code, 3);
    assert!(
        scan.stderr.contains("cannot be scanned here"),
        "{}",
        scan.stderr
    );

    // Compared with a local copy of the same files, nothing differs.
    let copy = env.tree("copy", &[("a.jpg", "aaaa"), ("b.jpg", "bb")]);
    assert_eq!(
        desktop(&env, &["location", "add", "copy", copy.to_str().unwrap()]).code,
        0
    );
    assert_eq!(desktop(&env, &["scan", "copy", "--json"]).code, 0);
    let diff = desktop(&env, &["diff", "photos", "copy"]);
    assert_eq!(diff.code, 0, "{}{}", diff.stdout, diff.stderr);
}

#[test]
fn existing_files_and_older_scans_need_force() {
    let env = laptop();
    let old = env.path("old.symbolon");
    let old = old.to_str().unwrap();
    env.ok(&["export", "photos", "-o", old]);
    let again = env.run(&["export", "photos", "-o", old]);
    assert_eq!(again.code, 4);
    assert!(again.stderr.contains("pass --force"), "{}", again.stderr);
    env.ok(&["export", "photos", "-o", old, "--force"]);

    std::fs::write(env.path("photos/c.jpg"), "c").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(5));
    env.ok(&["scan", "photos", "--json"]);
    let new = env.path("new.symbolon");
    let new = new.to_str().unwrap();
    env.ok(&["export", "photos", "-o", new]);

    assert_eq!(desktop(&env, &["import", new]).code, 0);
    let older = desktop(&env, &["import", old]);
    assert_eq!(older.code, 4);
    assert!(older.stderr.contains("--force"), "{}", older.stderr);
    let forced = desktop(&env, &["import", old, "--force", "--json"]);
    assert_eq!(forced.code, 0, "{}", forced.stderr);
    let json: Value = serde_json::from_str(&forced.stdout).unwrap();
    assert_eq!(json["imports"][0]["action"], "replaced");
    assert_eq!(json["imports"][0]["entries"], 2);

    let second = desktop(&env, &["import", new, "--as", "photos-new", "--json"]);
    let json: Value = serde_json::from_str(&second.stdout).unwrap();
    assert_eq!(
        (
            json["imports"][0]["label"].as_str(),
            json["imports"][0]["action"].as_str()
        ),
        (Some("photos-new"), Some("added"))
    );
}

#[test]
fn a_scanned_location_with_the_same_label_needs_a_choice() {
    let env = laptop();
    let file = env.path("photos.symbolon");
    let file = file.to_str().unwrap();
    env.ok(&["export", "photos", "-o", file]);

    let refused = env.run(&["import", file]);
    assert_eq!(refused.code, 4);
    assert!(
        refused.stderr.contains("is scanned on this computer") && refused.stderr.contains("--as"),
        "{}",
        refused.stderr
    );

    let replaced = env.run(&["import", file, "--replace", "--json"]);
    assert_eq!(replaced.code, 0, "{}", replaced.stderr);
    let list: Value = serde_json::from_str(&env.ok(&["location", "list", "--json"])).unwrap();
    assert_eq!(list["locations"][0]["kind"], "imported");
}

#[test]
fn bad_input_fails_clearly() {
    let env = laptop();
    let text = env.path("notes.txt");
    std::fs::write(&text, "hello").unwrap();
    let run = desktop(&env, &["import", text.to_str().unwrap()]);
    assert_eq!(run.code, 3);
    assert!(
        run.stderr.contains("is not a Symbolon export"),
        "{}",
        run.stderr
    );

    let missing = env.run(&[
        "export",
        "nope",
        "-o",
        env.path("x.symbolon").to_str().unwrap(),
    ]);
    assert_eq!(
        (missing.code, missing.stderr.as_str()),
        (3, "error: no location labelled \"nope\"\n")
    );
    assert!(!env.path("x.symbolon").exists());
    assert_eq!(
        env.run(&["import", "f", "--as", "x", "--into", "y"]).code,
        2
    );
}
