//! Phones from the command line (ADR-0017), through a fake `adb` whose phones are directories
//! of this computer. No agent is found next to the test binary, so phones are read with the
//! toybox fallback (GNU tools speak the same options).

#![cfg(unix)]

use serde_json::Value;

use crate::common::Env;

const FILES: &[(&str, &str)] = &[("Camera/a.jpg", "aaaa"), ("Camera/b.jpg", "bbbb")];

fn json(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

/// A phone `PHONE` (hardware serial `HW1`) whose DCIM folder matches a local `copy`.
fn phone() -> (Env, String) {
    let env = Env::new();
    let dcim = env.tree("phone/DCIM", FILES);
    env.tree("copy", FILES);
    env.phones(
        "PHONE device usb:1-3 product:frankel model:Pixel_10\n",
        &[("PHONE", "HW1")],
    );
    (env, dcim.to_str().unwrap().to_owned())
}

#[test]
fn devices_lists_states_and_what_to_do() {
    let env = Env::new();
    env.phones(
        "PHONE device model:Pixel_10\nR58M unauthorized usb:1-1\n",
        &[],
    );
    let devices = json(&env.ok(&["devices", "--json"]));
    let devices = devices["devices"].as_array().unwrap();
    assert_eq!(devices.len(), 2);
    assert_eq!(
        (
            &devices[0]["serial"],
            &devices[0]["state"],
            &devices[0]["hint"]
        ),
        (&Value::from("PHONE"), &Value::from("device"), &Value::Null)
    );
    assert!(
        devices[1]["hint"]
            .as_str()
            .unwrap()
            .contains("Allow USB debugging")
    );
}

#[test]
fn a_phone_is_added_scanned_and_compared() {
    let (env, dcim) = phone();
    let spec = format!("adb::{dcim}");
    let added = json(&env.ok(&[
        "location", "add", "phone", &spec, "--algo", "sha256", "--policy", "eager", "--json",
    ]));
    let location = &added["locations"][0];
    assert_eq!(location["kind"], "android");
    assert_eq!(location["root"], "adb:PHONE:[TMP]/phone/DCIM");
    assert_eq!(location["fingerprint"], "adb:HW1:[TMP]/phone/DCIM");
    assert_eq!(location["scannable"], true);

    let scan = env.run(&["scan", "phone", "--json"]);
    assert_eq!(scan.code, 0, "{}", scan.stderr);
    assert!(
        scan.stderr.contains("phone: using the toybox fallback")
            && scan.stderr.contains("no copy of the agent"),
        "{}",
        scan.stderr
    );
    let scans = json(&scan.stdout);
    assert_eq!(scans["scans"][0]["files"], 2);
    assert_eq!(scans["scans"][0]["hashed_files"], 2);

    let copy = env.path("copy");
    env.ok(&[
        "location",
        "add",
        "copy",
        copy.to_str().unwrap(),
        "--algo",
        "sha256",
    ]);
    env.ok(&["scan", "copy", "--json"]);
    let diff = env.run(&["diff", "phone", "copy"]);
    assert_eq!(diff.code, 0, "{}{}", diff.stdout, diff.stderr);
}

#[test]
fn the_fallback_cannot_hash_blake3() {
    let (env, dcim) = phone();
    env.ok(&[
        "location",
        "add",
        "phone",
        &format!("adb:PHONE:{dcim}"),
        "--policy",
        "eager",
    ]);
    let scan = env.run(&["scan", "phone", "--json"]);
    assert_eq!(scan.code, 0, "{}", scan.stderr);
    assert!(
        scan.stderr.contains("can only compute SHA-256"),
        "{}",
        scan.stderr
    );
    assert_eq!(json(&scan.stdout)["scans"][0]["hashed_files"], 0);
}

#[test]
fn a_phone_that_is_not_ready_is_not_scanned() {
    let (env, dcim) = phone();
    env.ok(&["location", "add", "phone", &format!("adb:PHONE:{dcim}")]);
    env.phones("PHONE unauthorized usb:1-3\n", &[]);
    let scan = env.run(&["scan", "phone"]);
    assert_eq!(scan.code, 3);
    assert!(
        scan.stderr.contains("Allow USB debugging"),
        "{}",
        scan.stderr
    );

    // Scanning everything skips the unplugged phone instead of failing.
    env.phones("", &[]);
    let all = env.run(&["scan", "--all", "--json"]);
    assert_eq!(all.code, 0, "{}", all.stderr);
    assert!(all.stderr.contains("phone: skipped"), "{}", all.stderr);
}

#[test]
fn adding_needs_one_ready_phone_or_a_serial() {
    let env = Env::new();
    let dcim = env.tree("phone/DCIM", FILES);
    let spec = format!("adb::{}", dcim.display());
    let none = env.run(&["location", "add", "phone", &spec]);
    assert_eq!(none.code, 3);
    assert!(
        none.stderr.contains("no phone is connected"),
        "{}",
        none.stderr
    );
    env.phones("A device\nB device\n", &[("A", "HW1"), ("B", "HW2")]);
    let two = env.run(&["location", "add", "phone", &spec]);
    assert!(two.stderr.contains("several phones"), "{}", two.stderr);
}
