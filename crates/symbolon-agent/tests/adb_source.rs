//! `AdbSource` end to end without a phone (ADR-0017): a mock transport whose "phone" is this
//! computer. `push` copies, `exec` runs the host-built agent, `shell` runs `bash -c`, so the
//! agent path and the toybox fallback (GNU tools speak the same options) are both exercised.

#![cfg(unix)]
#![expect(clippy::unwrap_used, reason = "test helpers fail loudly by design")]

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};

use symbolon_core::catalog::{Catalog, HashPolicy, NewLocation};
use symbolon_core::hash::HashAlgo;
use symbolon_core::scan::{ScanOptions, Scanner};
use symbolon_core::source::{AdbMode, AdbOptions, AdbSource, Source};
use symbolon_core::transport::android::{
    AdbError, AndroidTransport, Device, ExecStream, ShellOutput, parse_devices,
};

/// This computer pretending to be a phone; records the shell commands and programs it ran.
#[derive(Default)]
struct LocalPhone {
    shells: Mutex<Vec<String>>,
    execs: Mutex<Vec<Vec<String>>>,
}

struct Running(Child, ChildStdout);

impl Read for Running {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.1.read(buf)
    }
}

impl ExecStream for Running {
    fn finish(mut self: Box<Self>) -> Result<(), AdbError> {
        io::copy(&mut self.1, &mut io::sink())?;
        self.0.wait()?;
        Ok(())
    }
}

impl AndroidTransport for LocalPhone {
    fn devices(&self) -> Result<Vec<Device>, AdbError> {
        Ok(parse_devices("PHONE device model:Local\n"))
    }

    fn push(&self, _serial: &str, local: &Path, remote: &str) -> Result<(), AdbError> {
        fs::copy(local, remote)?;
        Ok(())
    }

    fn exec(&self, _serial: &str, argv: &[String]) -> Result<Box<dyn ExecStream>, AdbError> {
        self.execs.lock().unwrap().push(argv.to_vec());
        let mut child = Command::new(&argv[0])
            .args(&argv[1..])
            .stdout(Stdio::piped())
            .spawn()?;
        let stdout = child.stdout.take().unwrap();
        Ok(Box::new(Running(child, stdout)))
    }

    fn shell(&self, _serial: &str, command: &str) -> Result<ShellOutput, AdbError> {
        self.shells.lock().unwrap().push(command.to_owned());
        let output = Command::new("bash").arg("-c").arg(command).output()?;
        Ok(ShellOutput {
            status: output.status.code().unwrap_or(-1),
            stdout: output.stdout,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

struct Phone {
    dir: tempfile::TempDir,
    transport: Arc<LocalPhone>,
}

impl Phone {
    /// A "phone" with `DCIM` holding a few photos, and an empty push directory.
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let dcim = dir.path().join("sdcard/DCIM/Camera");
        fs::create_dir_all(&dcim).unwrap();
        fs::write(dcim.join("a.jpg"), "aaaa").unwrap();
        fs::write(dcim.join("b c.jpg"), vec![9_u8; 300_000]).unwrap();
        fs::create_dir_all(dir.path().join("data/local/tmp")).unwrap();
        Self {
            dir,
            transport: Arc::new(LocalPhone::default()),
        }
    }

    fn root(&self) -> PathBuf {
        self.dir.path().join("sdcard/DCIM")
    }

    fn source(&self, agent: Option<PathBuf>) -> AdbSource {
        let tmp = self.dir.path().join("data/local/tmp");
        let mut options = AdbOptions::default();
        tmp.to_str().unwrap().clone_into(&mut options.remote_dir);
        AdbSource::new(
            Arc::clone(&self.transport) as Arc<dyn AndroidTransport>,
            "PHONE",
            self.root().to_str().unwrap(),
            &[],
            agent,
            options,
        )
        .unwrap()
    }

    fn pushed(&self) -> Vec<String> {
        fs::read_dir(self.dir.path().join("data/local/tmp"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect()
    }
}

fn agent() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_symbolon-agent"))
}

/// Scans `source` as eager location `phone` with `algo` and returns the stored
/// `(path, full digest)` of each file, beside those of a local scan of the same directory.
fn scan_both(
    phone: &Phone,
    source: AdbSource,
    algo: HashAlgo,
) -> [Vec<(String, Option<Vec<u8>>)>; 2] {
    let dir = tempfile::tempdir().unwrap();
    let catalog = Catalog::open(dir.path().join("c.db")).unwrap();
    for label in ["phone", "local"] {
        catalog
            .add_location(
                NewLocation::local(label, phone.root())
                    .hash_policy(HashPolicy::Eager)
                    .hash_algo(algo),
            )
            .unwrap();
    }
    Scanner::start_with_source(
        &catalog,
        "phone",
        Arc::new(source) as Arc<dyn Source>,
        ScanOptions::default(),
    )
    .unwrap()
    .wait()
    .unwrap();
    Scanner::start(&catalog, "local", ScanOptions::default())
        .unwrap()
        .wait()
        .unwrap();
    let conn = rusqlite::Connection::open(catalog.path()).unwrap();
    ["phone", "local"].map(|label| {
        conn.prepare(
            "SELECT path_key, hash_full FROM entries e JOIN locations l ON l.id = e.location_id \
             WHERE l.label = ?1 AND e.kind = 'file' ORDER BY path_key",
        )
        .unwrap()
        .query_map([label], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
    })
}

#[test]
fn the_agent_scans_like_a_local_scan_and_is_removed_afterwards() {
    let phone = Phone::new();
    let source = phone.source(Some(agent()));
    assert_eq!(source.mode(), AdbMode::Agent);
    assert!(!phone.pushed().is_empty(), "the agent was pushed");
    let [remote, local] = scan_both(&phone, source, HashAlgo::Blake3);
    assert_eq!(remote, local);
    assert!(remote.iter().all(|(_, digest)| digest.is_some()));
    assert_eq!(
        phone.pushed(),
        Vec::<String>::new(),
        "nothing left on the phone"
    );
}

#[test]
fn the_agent_hashes_with_four_threads_by_default() {
    let phone = Phone::new();
    scan_both(&phone, phone.source(Some(agent())), HashAlgo::Blake3);
    let execs = phone.transport.execs.lock().unwrap();
    let hash = execs.iter().find(|argv| argv[1] == "hash").unwrap();
    let at = hash.iter().position(|arg| arg == "--threads").unwrap();
    assert_eq!(hash[at + 1], "4");
}

#[test]
fn without_an_agent_the_fallback_hashes_sha256() {
    let phone = Phone::new();
    let source = phone.source(None);
    assert!(
        matches!(source.mode(), AdbMode::Fallback { reason } if reason.contains("no copy of the agent"))
    );
    let [remote, local] = scan_both(&phone, source, HashAlgo::Sha256);
    assert_eq!(remote, local);
    assert!(remote.iter().all(|(_, digest)| digest.is_some()));
    assert!(
        phone
            .transport
            .shells
            .lock()
            .unwrap()
            .iter()
            .any(|c| c.contains("set -o pipefail") && c.contains("sha256sum"))
    );
}

#[test]
fn the_fallback_cannot_make_blake3_digests() {
    let phone = Phone::new();
    let [remote, _] = scan_both(&phone, phone.source(None), HashAlgo::Blake3);
    assert_eq!(remote.len(), 2, "still listed");
    assert!(remote.iter().all(|(_, digest)| digest.is_none()));
}

#[test]
fn an_agent_that_cannot_run_falls_back_and_is_removed() {
    let phone = Phone::new();
    let broken = phone.dir.path().join("not-an-agent");
    fs::write(
        &broken,
        "#!/bin/sh\necho 'cannot execute: Permission denied' >&2; exit 126\n",
    )
    .unwrap();
    let source = phone.source(Some(broken));
    match source.mode() {
        AdbMode::Fallback { reason } => {
            assert!(reason.contains("cannot run on this phone"), "{reason}");
            assert!(reason.contains("Permission denied"), "{reason}");
        }
        other => panic!("{other:?}"),
    }
    drop(source);
    assert_eq!(phone.pushed(), Vec::<String>::new());
}

#[test]
fn an_unreadable_file_is_a_warning_in_the_fallback() {
    use std::os::unix::fs::PermissionsExt as _;
    let phone = Phone::new();
    let a = phone.root().join("Camera/a.jpg");
    fs::set_permissions(&a, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read(&a).is_ok() {
        return; // running as root: permissions are not enforced
    }
    let [remote, _] = scan_both(&phone, phone.source(None), HashAlgo::Sha256);
    fs::set_permissions(&a, fs::Permissions::from_mode(0o644)).unwrap();
    let a_digest = remote.iter().find(|(p, _)| p == "Camera/a.jpg").unwrap();
    assert_eq!(a_digest.1, None, "a failed read is never a digest");
}

#[test]
fn a_missing_root_fails_the_walk_and_cleans_up() {
    let phone = Phone::new();
    fs::remove_dir_all(phone.root()).unwrap();
    for agent in [Some(agent()), None] {
        let source = phone.source(agent);
        assert!(
            source
                .walk(&mut |_| std::ops::ControlFlow::Continue(()))
                .is_err()
        );
        drop(source);
        assert_eq!(phone.pushed(), Vec::<String>::new());
    }
}
