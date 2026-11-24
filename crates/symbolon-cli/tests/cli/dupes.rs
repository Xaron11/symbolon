//! `symbolon dupes`.

use serde_json::Value;

use crate::common::{Env, settings};

const FILES: &[(&str, &str)] = &[
    ("a.jpg", "photo"),
    ("copy/a.jpg", "photo"),
    ("copy/again.jpg", "photo"),
    ("doc.txt", "a longer text file"),
    ("old/doc.txt", "a longer text file"),
    ("unique.txt", "only one"),
    ("empty1", ""),
    ("empty2", ""),
];

fn env(policy: &str) -> Env {
    let env = Env::new();
    let root = env.tree("photos", FILES);
    env.ok(&[
        "location",
        "add",
        "photos",
        root.to_str().unwrap(),
        "--policy",
        policy,
    ]);
    env.ok(&["scan", "photos", "--json"]);
    env
}

fn json(env: &Env, args: &[&str]) -> Value {
    serde_json::from_str(&env.ok(&[&["dupes", "--json"], args].concat())).unwrap()
}

#[test]
fn every_format_lists_the_largest_waste_first() {
    let env = env("eager");
    settings(true).bind(|| insta::assert_snapshot!("table", env.ok(&["dupes", "photos"])));
    settings(false).bind(|| {
        insta::assert_snapshot!("json", env.ok(&["dupes", "photos", "--json"]));
        insta::assert_snapshot!("csv", env.ok(&["dupes", "photos", "--format", "csv"]));
    });
}

#[test]
fn lazy_locations_are_hashed_unless_told_not_to() {
    let env = env("lazy");
    assert_eq!(
        json(&env, &["photos", "--no-hash"])["groups"],
        Value::Array(Vec::new())
    );
    assert_eq!(
        json(&env, &["photos"]),
        json(&self::env("eager"), &["photos"])
    );
}

#[test]
fn folders_sizes_and_several_locations() {
    let env = env("eager");
    let copy = json(&env, &["photos:copy"]);
    assert_eq!(copy["groups"].as_array().unwrap().len(), 1);
    assert_eq!(
        copy["groups"][0]["paths"],
        serde_json::json!(["copy/a.jpg", "copy/again.jpg"])
    );

    let big = json(&env, &["photos", "--min-size", "10"]);
    assert_eq!(
        big["groups"].as_array().unwrap().len(),
        1,
        "only doc.txt is that large"
    );

    let other = env.tree("other", &[("x", "same"), ("y", "same")]);
    env.ok(&[
        "location",
        "add",
        "other",
        other.to_str().unwrap(),
        "--policy",
        "eager",
    ]);
    env.ok(&["scan", "other", "--json"]);
    let both = json(&env, &["photos", "other"]);
    let sides: Vec<&str> = both["totals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["side"].as_str().unwrap())
        .collect();
    assert_eq!(sides, ["photos", "other"]);
    assert_eq!(both["totals"][1]["groups"], 1);

    let none = env.ok(&["dupes", "photos:old"]);
    assert_eq!(none, "photos:old: 0 groups, 0 files, 0 B could be freed\n");
    assert_eq!(env.run(&["dupes"]).code, 2);
    assert_eq!(env.run(&["dupes", "nope"]).code, 3);
}
