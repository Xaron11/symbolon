//! Following a location's device in `scan` and `diff` (ADR-0016), with a simulated mount
//! table: `disk` is the temporary directory, pendrives are subdirectories mounted on top.

use serde_json::Value;

use crate::common::Env;

/// A scanned location `photos` on pendrive `usb-1`, mounted at `usb-a`.
fn pendrive() -> Env {
    let env = Env::new();
    let usb = env.tree("usb-a", &[("a.jpg", "aaaa"), ("b.jpg", "bb")]);
    env.mount(&[("disk", "."), ("usb-1", "usb-a")]);
    env.ok(&["location", "add", "photos", usb.to_str().unwrap()]);
    env.ok(&["scan", "photos", "--json"]);
    env
}

fn location(env: &Env, label: &str) -> Value {
    let list: Value = serde_json::from_str(&env.ok(&["location", "list", "--json"])).unwrap();
    list["locations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["label"] == label)
        .unwrap()
        .clone()
}

/// The pendrive is unplugged: its mount point disappears.
fn unplug(env: &Env) {
    std::fs::remove_dir_all(env.path("usb-a")).unwrap();
}

#[test]
fn a_replugged_device_is_followed_to_its_new_mount_point() {
    let env = pendrive();
    assert_eq!(location(&env, "photos")["fingerprint"], "fs:usb-1:/");
    unplug(&env);
    env.tree(
        "usb-b",
        &[("a.jpg", "aaaa"), ("b.jpg", "bb"), ("c.jpg", "c")],
    );
    env.mount(&[("disk", "."), ("usb-1", "usb-b")]);

    let run = env.run(&["scan", "photos", "--json"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(
        run.stderr
            .contains("photos: its device is now at [TMP]/usb-b (was [TMP]/usb-a); following it"),
        "{}",
        run.stderr
    );
    let photos = location(&env, "photos");
    assert_eq!(photos["root"], "[TMP]/usb-b");
    assert_eq!(photos["files"], 3);
}

#[test]
fn an_ambiguous_device_needs_a_choice() {
    let env = pendrive();
    unplug(&env);
    env.tree("usb-b", &[]);
    env.tree("usb-c", &[]);
    env.mount(&[("disk", "."), ("usb-1", "usb-b"), ("usb-1", "usb-c")]);

    let run = env.run(&["scan", "photos"]);
    assert_eq!(run.code, 4, "{}", run.stderr);
    assert!(
        run.stderr.contains("[TMP]/usb-b, [TMP]/usb-c")
            && run
                .stderr
                .contains("symbolon location set photos --root <PATH>"),
        "{}",
        run.stderr
    );
    assert_eq!(location(&env, "photos")["files"], 2, "nothing scanned");

    let usb = env.path("usb-c");
    env.ok(&["location", "set", "photos", "--root", usb.to_str().unwrap()]);
    assert_eq!(env.run(&["scan", "photos"]).code, 0);
}

#[test]
fn an_empty_mount_point_is_not_scanned_without_consent() {
    let env = pendrive();
    unplug(&env);
    env.tree("usb-a", &[]);
    env.mount(&[("disk", ".")]);

    let run = env.run(&["scan", "photos"]);
    assert_eq!(run.code, 4, "{}", run.stderr);
    assert!(
        run.stderr.contains("expects device fs:usb-1:/")
            && run.stderr.contains("device fs:disk:/usb-a is there")
            && run.stderr.contains("pass --yes"),
        "{}",
        run.stderr
    );
    assert_eq!(
        location(&env, "photos")["files"],
        2,
        "nothing recorded as deleted"
    );

    let run = env.run(&["scan", "photos", "--yes", "--json"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let photos = location(&env, "photos");
    assert_eq!(photos["fingerprint"], "fs:disk:/usb-a");
    assert_eq!(photos["files"], 0);
}

#[test]
fn scanning_everything_skips_unplugged_devices() {
    let env = pendrive();
    let laptop = env.tree("laptop", &[("notes.txt", "n")]);
    env.ok(&["location", "add", "laptop", laptop.to_str().unwrap()]);
    unplug(&env);
    env.mount(&[("disk", ".")]);

    let run = env.run(&["scan", "--all", "--json"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(
        run.stderr
            .contains("photos: skipped, [TMP]/usb-a is not available"),
        "{}",
        run.stderr
    );
    let scans: Value = serde_json::from_str(&run.stdout).unwrap();
    assert_eq!(scans["scans"].as_array().unwrap().len(), 1);
    assert_eq!(scans["scans"][0]["label"], "laptop");
}

#[test]
fn diff_follows_a_moved_device() {
    let env = pendrive();
    let copy = env.tree("copy", &[("a.jpg", "aaaa"), ("b.jpg", "bb")]);
    env.ok(&["location", "add", "copy", copy.to_str().unwrap()]);
    env.ok(&["scan", "copy", "--json"]);
    // Replugged: the same files, with their times, at another mount point.
    std::fs::rename(env.path("usb-a"), env.path("usb-b")).unwrap();
    env.mount(&[("disk", "."), ("usb-1", "usb-b")]);

    let run = env.run(&["diff", "photos", "copy"]);
    assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
    assert!(run.stderr.contains("following it"), "{}", run.stderr);
    assert_eq!(location(&env, "photos")["root"], "[TMP]/usb-b");
}
