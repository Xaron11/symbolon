//! Runs the real `symbolon` binary against a temporary catalog.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;

use symbolon_core::catalog::Catalog;
use symbolon_core::identity::{Mount, Volumes};

/// An empty mount table, so scans started from tests record no fingerprints either.
struct NoMounts;

impl Volumes for NoMounts {
    fn mounts(&self) -> std::io::Result<Vec<Mount>> {
        Ok(Vec::new())
    }
}

pub(crate) struct Env {
    _dir: tempfile::TempDir,
    /// The temporary directory as the binary prints it: canonical, without `\\?\`.
    root: PathBuf,
    pub(crate) catalog: PathBuf,
    /// The simulated mount table every run uses (`SYMBOLON_TEST_MOUNTS`): empty unless a test
    /// plugs devices in, so no test depends on the real filesystems.
    mounts: PathBuf,
    /// The `adb` every run uses (`SYMBOLON_ADB`): a fake one, so no test reaches a real phone.
    adb: PathBuf,
}

/// One finished run.
#[derive(Debug)]
pub(crate) struct Run {
    pub(crate) code: i32,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

impl Env {
    pub(crate) fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let canonical = fs::canonicalize(dir.path()).unwrap();
        let text = canonical.to_str().unwrap();
        let root = PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(text));
        let catalog = root.join("catalog.db");
        let mounts = root.join("mounts.txt");
        fs::write(&mounts, "").unwrap();
        let adb = root.join("fake-adb/adb");
        write_fake_adb(&adb);
        Self {
            _dir: dir,
            root,
            catalog,
            mounts,
            adb,
        }
    }

    /// Connects simulated phones: `devices` as `adb devices -l` lists them, and the hardware
    /// serial (`ro.serialno`) of each.
    pub(crate) fn phones(&self, devices: &str, hardware: &[(&str, &str)]) {
        let dir = self.adb.parent().unwrap();
        fs::write(dir.join("devices"), devices).unwrap();
        let lines: Vec<String> = hardware.iter().map(|(s, h)| format!("{s} {h}")).collect();
        fs::write(dir.join("hardware"), lines.join("\n")).unwrap();
    }

    /// Replaces the simulated mount table: `(filesystem UUID, mount point relative to the
    /// temporary directory)`, mounted at its root.
    pub(crate) fn mount(&self, mounts: &[(&str, &str)]) {
        let lines: Vec<String> = mounts
            .iter()
            .map(|(uuid, at)| format!("{uuid}\t{}", self.root.join(at).display()))
            .collect();
        fs::write(&self.mounts, lines.join("\n")).unwrap();
    }

    /// The catalog opened in this process, seeing no mounted filesystems like the binary.
    pub(crate) fn open_catalog(&self) -> Catalog {
        Catalog::open(&self.catalog)
            .unwrap()
            .with_volumes(Arc::new(NoMounts))
    }

    /// Creates `dir` under the temporary directory with the given files.
    pub(crate) fn tree(&self, dir: &str, files: &[(&str, &str)]) -> PathBuf {
        let base = self.root.join(dir);
        fs::create_dir_all(&base).unwrap();
        for (path, contents) in files {
            let path = base.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, contents).unwrap();
        }
        base
    }

    pub(crate) fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    /// Runs `symbolon` with `args`, the catalog passed through `SYMBOLON_CATALOG` and stdin
    /// closed (not a terminal).
    pub(crate) fn run(&self, args: &[&str]) -> Run {
        let output = Command::new(env!("CARGO_BIN_EXE_symbolon"))
            .args(args)
            .env("SYMBOLON_CATALOG", &self.catalog)
            .env("SYMBOLON_TEST_MOUNTS", &self.mounts)
            .env("SYMBOLON_ADB", &self.adb)
            .env_remove("SYMBOLON_AGENT")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        Run {
            code: output.status.code().unwrap(),
            stdout: self.redact(&String::from_utf8(output.stdout).unwrap()),
            stderr: self.redact(&String::from_utf8(output.stderr).unwrap()),
        }
    }

    /// Runs and expects exit code 0, returning stdout.
    pub(crate) fn ok(&self, args: &[&str]) -> String {
        let run = self.run(args);
        assert_eq!(run.code, 0, "{args:?} failed: {}", run.stderr);
        run.stdout
    }

    /// Replaces the temporary directory with `[TMP]` and uses `/` as the separator, so
    /// snapshots match on every platform.
    fn redact(&self, text: &str) -> String {
        let root = self.root.to_str().unwrap();
        let json_root = serde_json_escape(root);
        text.replace(&json_root, "[TMP]")
            .replace(root, "[TMP]")
            .replace(r"\\", "/")
            .replace('\\', "/")
    }
}

fn serde_json_escape(text: &str) -> String {
    text.replace('\\', r"\\")
}

/// Settings shared by snapshots: timestamps redacted; with `table`, runs of spaces collapsed,
/// since column widths depend on the temporary directory's length.
pub(crate) fn settings(table: bool) -> insta::Settings {
    let mut settings = insta::Settings::clone_current();
    settings.add_filter(r"\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ", "[TIME]");
    if table {
        settings.add_filter(r" {2,}", "  ");
    }
    settings
}

/// A fake `adb` whose phones are this computer: it lists `devices`, answers `getprop` from
/// `hardware`, copies for `push` and runs `shell` and `exec-out` command lines with bash. On
/// Windows it is never executed (phone tests are Unix-only); its path still keeps the real
/// `adb` out of every test.
fn write_fake_adb(path: &std::path::Path) {
    let dir = path.parent().unwrap();
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join("devices"), "").unwrap();
    fs::write(dir.join("hardware"), "").unwrap();
    fs::write(
        path,
        r#"#!/usr/bin/env bash
dir="$(dirname "$0")"
if [ "$1" = devices ]; then echo "List of devices attached"; cat "$dir/devices"; exit 0; fi
[ "$1" = -s ] || { echo "unexpected: $*" >&2; exit 1; }
serial=$2; cmd=$3; shift 3
case "$cmd" in
  push) cp -- "$1" "$2" ;;
  shell)
    case "$1" in
      "getprop ro.serialno") awk -v s="$serial" '$1 == s { print $2 }' "$dir/hardware" ;;
      "getprop ro.product.cpu.abi") echo arm64-v8a ;;
      *) exec bash -c "$1" ;;
    esac ;;
  exec-out) exec bash -c "$1" ;;
  *) echo "unexpected: $cmd" >&2; exit 1 ;;
esac
"#,
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
}
