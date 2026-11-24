//! The M3 acceptance test: the whole workflow from the shell, as `docs/cli.md` walks through
//! it. Add two locations, scan, diff, sync and rescan, find duplicates, export one location
//! and compare against it from another catalog.

use serde_json::Value;

use crate::common::Env;

fn json(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

/// Categories with at least one file in a `diff --json` summary, by name.
fn categories(diff: &Value) -> Vec<String> {
    diff["summary"]
        .as_object()
        .unwrap()
        .iter()
        .filter(|(_, totals)| totals["files"].as_u64() > Some(0))
        .map(|(name, _)| name.clone())
        .collect()
}

#[test]
fn the_full_workflow() {
    let env = Env::new();
    let laptop = env.tree(
        "laptop",
        &[
            ("notes.txt", "meeting at 10"),
            ("todo.txt", "buy milk, call mum"),
            ("notes-copy.txt", "meeting at 10"),
            ("photos/beach.jpg", "sand and sea"),
            ("draft.txt", "version 2"),
        ],
    );
    let backup = env.tree(
        "backup",
        &[
            ("notes.txt", "meeting at 10"),
            ("old/beach.jpg", "sand and sea"),
            ("todo-list.txt", "buy milk, call mum"),
            ("draft.txt", "version 1"),
            ("2019.txt", "archived"),
        ],
    );

    // 1. Save both directories as locations and scan them.
    env.ok(&["location", "add", "laptop", laptop.to_str().unwrap()]);
    env.ok(&[
        "location",
        "add",
        "backup",
        backup.to_str().unwrap(),
        "--policy",
        "eager",
    ]);
    let scans = json(&env.ok(&["scan", "laptop", "backup", "--json"]));
    assert_eq!(scans["scans"][0]["hashed_files"], 0, "laptop is lazy");
    assert_eq!(scans["scans"][1]["hashed_files"], 5, "backup is eager");

    // 2. Compare: every kind of difference shows up, and the exit code says so.
    let diff = env.run(&["diff", "laptop", "backup", "--json"]);
    assert_eq!(diff.code, 1);
    let diff = json(&diff.stdout);
    assert_eq!(
        categories(&diff),
        ["different", "identical", "moved", "only_in", "renamed"]
    );

    // 3. Bring the backup up to date, rescan, and nothing differs anymore.
    for (path, contents) in [
        ("todo.txt", "buy milk, call mum"),
        ("notes-copy.txt", "meeting at 10"),
        ("photos/beach.jpg", "sand and sea"),
        ("draft.txt", "version 2"),
    ] {
        std::fs::create_dir_all(backup.join("photos")).unwrap();
        std::fs::write(backup.join(path), contents).unwrap();
    }
    for stale in ["old/beach.jpg", "todo-list.txt", "2019.txt"] {
        std::fs::remove_file(backup.join(stale)).unwrap();
    }
    let rescan = json(&env.ok(&["scan", "backup", "--json"]));
    assert_eq!(
        rescan["scans"][0]["reused_hashes"], 1,
        "unchanged notes.txt is not read again"
    );
    assert_eq!(env.run(&["diff", "laptop", "backup"]).code, 0);

    // 4. Duplicates within the laptop.
    let dupes = json(&env.ok(&["dupes", "laptop", "--json"]));
    assert_eq!(
        dupes["groups"][0]["paths"],
        serde_json::json!(["notes-copy.txt", "notes.txt"])
    );

    // 5. Export the backup and compare against it from another computer's catalog.
    let file = env.path("backup.symbolon");
    env.ok(&["export", "backup", "-o", file.to_str().unwrap()]);
    let elsewhere = env.path("elsewhere.db");
    let other =
        |args: &[&str]| env.run(&[&["--catalog", elsewhere.to_str().unwrap()], args].concat());
    assert_eq!(other(&["import", file.to_str().unwrap()]).code, 0);
    assert_eq!(
        other(&["location", "add", "laptop", laptop.to_str().unwrap()]).code,
        0
    );
    assert_eq!(other(&["scan", "laptop"]).code, 0);
    let remote = other(&["diff", "laptop", "backup"]);
    assert_eq!(remote.code, 0, "{}{}", remote.stdout, remote.stderr);
}
