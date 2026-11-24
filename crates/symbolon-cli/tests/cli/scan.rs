//! `symbolon scan`.

use std::io;
use std::ops::ControlFlow;
use std::sync::Arc;

use serde_json::Value;
use symbolon_core::catalog::Catalog;
use symbolon_core::pathkey::RelPath;
use symbolon_core::scan::{ScanOptions, Scanner};
use symbolon_core::source::{ReadSeek, Source, SourceEntry, SourceError, WalkItem, WalkOutcome};

use crate::common::{Env, settings};

fn env_with(policy: &str) -> Env {
    let env = Env::new();
    let root = env.tree(
        "photos",
        &[("a.jpg", "aaaa"), ("trip/b.jpg", "bb"), ("c.jpg", "c")],
    );
    env.ok(&[
        "location",
        "add",
        "photos",
        root.to_str().unwrap(),
        "--policy",
        policy,
    ]);
    env
}

fn scans(env: &Env, args: &[&str]) -> Vec<Value> {
    let out = env.ok(&[&["scan", "--json"], args].concat());
    let json: Value = serde_json::from_str(&out).unwrap();
    json["scans"].as_array().unwrap().clone()
}

#[test]
fn a_scan_reports_its_summary() {
    let env = env_with("eager");
    let mut settings = settings(false);
    settings.add_filter(r#""elapsed_ms": \d+"#, r#""elapsed_ms": "[MS]""#);
    settings
        .bind(|| insta::assert_snapshot!("summary_json", env.ok(&["scan", "photos", "--json"])));

    let mut table = crate::common::settings(true);
    table.add_filter(r"\d+\.\d s", "[TIME]");
    table.bind(|| insta::assert_snapshot!("summary_table", env.ok(&["scan", "photos"])));
}

#[test]
fn rescans_reuse_hashes_and_full_hashes_lazy_locations() {
    let env = env_with("lazy");
    let first = &scans(&env, &["photos"])[0];
    assert_eq!(
        (first["files"].as_u64(), first["hashed_files"].as_u64()),
        (Some(3), Some(0))
    );

    let full = &scans(&env, &["photos", "--full"])[0];
    assert_eq!(full["hashed_files"], 3);

    let again = &scans(&env, &["photos", "--full"])[0];
    assert_eq!(
        (
            again["hashed_files"].as_u64(),
            again["reused_hashes"].as_u64()
        ),
        (Some(0), Some(3))
    );
}

#[test]
fn all_scans_every_location_and_unknown_labels_scan_nothing() {
    let env = env_with("eager");
    let docs = env.tree("docs", &[("d.txt", "d")]);
    env.ok(&["location", "add", "docs", docs.to_str().unwrap()]);

    let unknown = env.run(&["scan", "photos", "nope"]);
    assert_eq!(unknown.code, 3);
    assert_eq!(unknown.stderr, "error: no location labelled \"nope\"\n");
    assert!(
        env.ok(&["location", "list", "--format", "csv"])
            .lines()
            .skip(1)
            .all(|l| l.ends_with(",,")),
        "nothing was scanned"
    );

    let labels: Vec<Value> = scans(&env, &["--all"])
        .iter()
        .map(|s| s["label"].clone())
        .collect();
    assert_eq!(labels, ["docs", "photos"]);
    assert_eq!(env.run(&["scan"]).code, 2, "labels or --all are required");
    assert_eq!(env.run(&["scan", "photos", "--all"]).code, 2);
}

#[cfg(unix)]
#[test]
fn unreadable_files_are_counted_or_listed() {
    use std::os::unix::fs::PermissionsExt as _;
    let env = env_with("eager");
    let secret = env.path("photos/c.jpg");
    std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read(&secret).is_ok() {
        return; // Running as root: permissions do not apply.
    }

    let run = env.run(&["scan", "photos", "--json"]);
    assert_eq!(run.code, 0, "warnings do not fail a scan");
    assert_eq!(
        run.stderr,
        "photos: 1 warning; run with --verbose to list them\n"
    );

    let verbose = env.run(&["scan", "photos", "--verbose", "--json"]);
    assert!(
        verbose.stderr.starts_with("warning: photos:c.jpg: "),
        "{}",
        verbose.stderr
    );
    assert_eq!(env.run(&["scan", "photos", "--quiet", "--json"]).stderr, "");
}

/// A walk that never ends, so its scan holds the location's lock.
struct Stuck;

impl Source for Stuck {
    fn walk(
        &self,
        _visit: &mut dyn FnMut(WalkItem) -> ControlFlow<()>,
    ) -> Result<WalkOutcome, SourceError> {
        loop {
            std::thread::park();
        }
    }
    fn open(&self, _path: &RelPath) -> io::Result<Box<dyn ReadSeek + Send>> {
        Err(io::ErrorKind::Unsupported.into())
    }
    fn stat(&self, _path: &RelPath) -> io::Result<SourceEntry> {
        Err(io::ErrorKind::Unsupported.into())
    }
}

#[test]
fn a_location_being_scanned_is_refused() {
    let env = env_with("eager");
    let catalog = Catalog::open(&env.catalog).unwrap();
    let _running =
        Scanner::start_with_source(&catalog, "photos", Arc::new(Stuck), ScanOptions::default())
            .unwrap();
    let run = env.run(&["scan", "photos"]);
    assert_eq!(run.code, 4);
    assert_eq!(
        run.stderr,
        "error: location \"photos\" is already being scanned\n"
    );
}

#[cfg(unix)]
#[test]
fn ctrl_c_cancels_cleanly() {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    let env = Env::new();
    // Sparse files: instant to create, slow enough to hash for the signal to arrive first.
    let root = env.tree("big", &[]);
    for i in 0..64 {
        std::fs::File::create(root.join(format!("{i}.bin")))
            .unwrap()
            .set_len(64 << 20)
            .unwrap();
    }
    env.ok(&[
        "location",
        "add",
        "big",
        root.to_str().unwrap(),
        "--policy",
        "eager",
    ]);

    let child = Command::new(env!("CARGO_BIN_EXE_symbolon"))
        .args(["scan", "big", "--json"])
        .env("SYMBOLON_CATALOG", &env.catalog)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let status = last_scan_status(&env);
    let deadline = Instant::now() + Duration::from_secs(10);
    while status().as_deref() != Some("running") {
        assert!(Instant::now() < deadline, "the scan never started");
        std::thread::sleep(Duration::from_millis(5));
    }
    let signal = Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .unwrap();
    assert!(signal.success());

    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(130));
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["scans"][0]["status"], "cancelled");
    assert_eq!(status().as_deref(), Some("cancelled"));
}

/// The status of the newest scan, read through the CLI's JSON output of `location list`.
#[cfg(unix)]
fn last_scan_status(env: &Env) -> impl Fn() -> Option<String> + '_ {
    move || {
        let out = env.ok(&["location", "list", "--json"]);
        let json: Value = serde_json::from_str(&out).unwrap();
        json["locations"][0]["last_scan_status"]
            .as_str()
            .map(str::to_owned)
    }
}
