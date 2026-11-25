//! Phone locations end to end without a phone (ADR-0017): the catalog reaches "phones"
//! through a mock transport whose devices are directories of this computer, scans them with
//! the host-built agent, and recognizes them by their hardware serial.

#![cfg(unix)]
#![expect(clippy::unwrap_used, reason = "test helpers fail loudly by design")]

use std::collections::HashMap;
use std::fs;
use std::io::{self, Read};
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};

use symbolon_core::catalog::{
    Catalog, CatalogError, HashPolicy, LocationRoot, NewLocation, Whereabouts,
};
use symbolon_core::compare::{Category, CompareOptions, Comparison, Side};
use symbolon_core::scan::{ScanError, ScanOptions, Scanner};
use symbolon_core::source::AdbOptions;
use symbolon_core::transport::android::{
    AdbError, AndroidTransport, Device, ExecStream, ShellOutput, parse_devices,
};

/// Phones that are this computer: `devices` is `adb devices -l` output, `hardware` maps
/// connection serials to `ro.serialno`.
#[derive(Default)]
struct Phones {
    devices: Mutex<String>,
    hardware: Mutex<HashMap<String, String>>,
}

impl Phones {
    fn connect(&self, devices: &str, hardware: &[(&str, &str)]) {
        devices.clone_into(&mut self.devices.lock().unwrap());
        *self.hardware.lock().unwrap() = hardware
            .iter()
            .map(|(s, h)| ((*s).to_owned(), (*h).to_owned()))
            .collect();
    }
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

impl AndroidTransport for Phones {
    fn devices(&self) -> Result<Vec<Device>, AdbError> {
        Ok(parse_devices(&self.devices.lock().unwrap()))
    }

    fn push(&self, _serial: &str, local: &Path, remote: &str) -> Result<(), AdbError> {
        fs::copy(local, remote)?;
        Ok(())
    }

    fn exec(&self, _serial: &str, argv: &[String]) -> Result<Box<dyn ExecStream>, AdbError> {
        let mut child = Command::new(&argv[0])
            .args(&argv[1..])
            .stdout(Stdio::piped())
            .spawn()?;
        let stdout = child.stdout.take().unwrap();
        Ok(Box::new(Running(child, stdout)))
    }

    fn shell(&self, serial: &str, command: &str) -> Result<ShellOutput, AdbError> {
        let answer = |text: &str| ShellOutput {
            status: 0,
            stdout: format!("{text}\n").into_bytes(),
            stderr: String::new(),
        };
        match command {
            "getprop ro.serialno" => {
                return Ok(answer(
                    self.hardware
                        .lock()
                        .unwrap()
                        .get(serial)
                        .map_or("", String::as_str),
                ));
            }
            "getprop ro.product.cpu.abi" => return Ok(answer("arm64-v8a")),
            _ => {}
        }
        let output = Command::new("bash").arg("-c").arg(command).output()?;
        Ok(ShellOutput {
            status: output.status.code().unwrap_or(-1),
            stdout: output.stdout,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

struct World {
    dir: tempfile::TempDir,
    phones: Arc<Phones>,
    catalog: Catalog,
}

impl World {
    /// One phone `PHONE` (hardware serial `HW1`) whose `DCIM` matches a local `copy`.
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        for side in ["phone/sdcard/DCIM", "copy"] {
            let root = dir.path().join(side);
            fs::create_dir_all(root.join("Camera")).unwrap();
            fs::write(root.join("Camera/a.jpg"), "aaaa").unwrap();
            fs::write(root.join("Camera/b.jpg"), "bbbb").unwrap();
        }
        fs::create_dir_all(dir.path().join("tmp")).unwrap();
        let phones = Arc::new(Phones::default());
        phones.connect("PHONE device model:Pixel_10\n", &[("PHONE", "HW1")]);
        let mut options = AdbOptions::default();
        dir.path()
            .join("tmp")
            .to_str()
            .unwrap()
            .clone_into(&mut options.remote_dir);
        let catalog = Catalog::open(dir.path().join("catalog.db"))
            .unwrap()
            .with_android(
                Arc::clone(&phones) as Arc<dyn AndroidTransport>,
                Some(PathBuf::from(env!("CARGO_BIN_EXE_symbolon-agent"))),
                options,
            );
        Self {
            dir,
            phones,
            catalog,
        }
    }

    fn dcim(&self) -> String {
        self.dir
            .path()
            .join("phone/sdcard/DCIM")
            .to_str()
            .unwrap()
            .to_owned()
    }

    fn add_phone(&self) {
        self.catalog
            .add_location(
                NewLocation::android("phone", "PHONE", self.dcim()).hash_policy(HashPolicy::Lazy),
            )
            .unwrap();
    }

    fn scan(&self, label: &str) -> Result<(), ScanError> {
        Scanner::start(&self.catalog, label, ScanOptions::default())?
            .wait()
            .map(drop)
    }

    fn pushed(&self) -> usize {
        fs::read_dir(self.dir.path().join("tmp")).unwrap().count()
    }
}

#[test]
fn a_phone_is_scanned_and_compared_on_the_phone() {
    let world = World::new();
    world.add_phone();
    let phone = world.catalog.location("phone").unwrap();
    assert_eq!(
        phone.root,
        LocationRoot::Android {
            serial: "PHONE".into(),
            path: world.dcim()
        }
    );
    assert_eq!(
        phone.fingerprint.as_deref(),
        Some(format!("adb:HW1:{}", world.dcim()).as_str())
    );
    world
        .catalog
        .add_location(NewLocation::local("copy", world.dir.path().join("copy")))
        .unwrap();
    world.scan("phone").unwrap();
    world.scan("copy").unwrap();

    // Lazy on both sides: the comparison hashes the phone's files on the phone.
    let mut cmp = Comparison::new(
        &world.catalog,
        &[Side::location("phone"), Side::location("copy")],
        CompareOptions::default(),
    )
    .unwrap();
    let resolved = cmp
        .resolve(&ScanOptions::default(), |_| ControlFlow::Continue(()))
        .unwrap();
    assert_eq!(resolved.unreachable, Vec::<String>::new());
    assert_eq!(cmp.summary().unwrap().get(Category::Identical).rows, 2);
    assert_eq!(world.pushed(), 0, "nothing left on the phone");
}

#[test]
fn a_phone_reconnected_under_another_serial_has_moved() {
    let world = World::new();
    world.add_phone();
    world.phones.connect(
        "192.168.1.5:5555 device model:Pixel_10\n",
        &[("192.168.1.5:5555", "HW1")],
    );
    let to = format!("adb:192.168.1.5:5555:{}", world.dcim());
    assert_eq!(
        world.catalog.locate("phone").unwrap(),
        Whereabouts::Moved {
            from: PathBuf::from(format!("adb:PHONE:{}", world.dcim())),
            to: PathBuf::from(&to),
        }
    );
    assert!(matches!(world.scan("phone"), Err(ScanError::Moved { .. })));
    world.catalog.relocate("phone", Path::new(&to)).unwrap();
    assert!(matches!(
        world.catalog.locate("phone").unwrap(),
        Whereabouts::Here(_)
    ));
    world.scan("phone").unwrap();
}

#[test]
fn another_phone_under_the_same_serial_is_not_the_location() {
    let world = World::new();
    world.add_phone();
    world.phones.connect("PHONE device\n", &[("PHONE", "HW2")]);
    assert!(matches!(
        world.catalog.locate("phone").unwrap(),
        Whereabouts::OtherDevice { .. }
    ));
    world.phones.connect("", &[]);
    assert!(matches!(
        world.catalog.locate("phone").unwrap(),
        Whereabouts::Missing(_)
    ));
}

#[test]
fn a_phone_that_is_not_ready_fails_with_what_to_do() {
    let world = World::new();
    world.add_phone();
    world.phones.connect("PHONE unauthorized usb:1-1\n", &[]);
    let error = world.scan("phone").unwrap_err().to_string();
    assert!(error.contains("Allow USB debugging"), "{error}");
    world.phones.connect("", &[]);
    let error = world.scan("phone").unwrap_err().to_string();
    assert!(error.contains("not connected"), "{error}");
}

#[test]
fn an_empty_serial_means_the_only_phone() {
    let world = World::new();
    world
        .catalog
        .add_location(NewLocation::android("one", "", world.dcim()))
        .unwrap();
    assert!(matches!(
        world.catalog.location("one").unwrap().root,
        LocationRoot::Android { serial, .. } if serial == "PHONE"
    ));
    world.phones.connect(
        "PHONE device\nOTHER device\n",
        &[("PHONE", "HW1"), ("OTHER", "HW3")],
    );
    let error = world
        .catalog
        .add_location(NewLocation::android("two", "", world.dcim()))
        .unwrap_err();
    assert!(
        matches!(&error, CatalogError::Device(message) if message.contains("several phones")),
        "{error}"
    );
    let relative = world
        .catalog
        .add_location(NewLocation::android("three", "PHONE", "sdcard"))
        .unwrap_err();
    assert!(matches!(relative, CatalogError::Device(_)));
}
