//! Phone locations (ADR-0017): adding them, recognizing the phone by its hardware serial, and
//! reading them through an [`AdbSource`].

use std::path::PathBuf;
use std::sync::Arc;

use rusqlite::{ErrorCode, TransactionBehavior, params};

use super::{Catalog, CatalogError, Location, LocationRoot, NewLocation, Whereabouts, now_ms};
use crate::identity::Fingerprint;
use crate::pathkey::PathEncoding;
use crate::source::{AdbOptions, AdbSource, SourceError};
use crate::transport::android::{AndroidTransport, Device, DeviceState, SystemAdb};

/// How a catalog reaches phones.
pub(crate) struct Access {
    transport: Arc<dyn AndroidTransport>,
    agent: Option<PathBuf>,
    options: AdbOptions,
}

impl Default for Access {
    fn default() -> Self {
        Self {
            transport: Arc::new(SystemAdb::new()),
            agent: None,
            options: AdbOptions::default(),
        }
    }
}

impl Catalog {
    /// This catalog, reaching phones through `transport` and pushing `agent` (the host's copy
    /// of the on-device scanner; without one, phones are read with the toybox fallback).
    #[must_use]
    pub fn with_android(
        mut self,
        transport: Arc<dyn AndroidTransport>,
        agent: Option<PathBuf>,
        options: AdbOptions,
    ) -> Self {
        self.android = Arc::new(Access {
            transport,
            agent,
            options,
        });
        self
    }

    /// The transport phones are reached through.
    pub fn android_transport(&self) -> Arc<dyn AndroidTransport> {
        Arc::clone(&self.android.transport)
    }

    pub(super) fn add_android(
        &self,
        new: NewLocation,
        serial: &str,
        path: &str,
    ) -> Result<Location, CatalogError> {
        if !path.starts_with('/') {
            return Err(CatalogError::Device(format!(
                "the path on the phone must be absolute, like /sdcard/DCIM (got {path:?})"
            )));
        }
        let device = self.ready_device(serial)?;
        let fingerprint = self.android_fingerprint(&device.serial, path);
        let mut conn = self.connect()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let inserted = tx.execute(
            "INSERT INTO locations (label, root_kind, root_raw, path_encoding, fingerprint, \
             hash_algo, hash_policy, ignore_patterns, created_at) \
             VALUES (?1, 'local', ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                new.label,
                path.as_bytes(),
                PathEncoding::Posix.name(),
                fingerprint.as_ref().map(Fingerprint::as_str),
                new.hash_algo.name(),
                new.hash_policy.name(),
                new.ignore_patterns.join("\n"),
                now_ms(),
            ],
        );
        match inserted {
            Err(rusqlite::Error::SqliteFailure(e, _))
                if e.code == ErrorCode::ConstraintViolation =>
            {
                return Err(CatalogError::LabelTaken(new.label));
            }
            other => other?,
        };
        tx.execute(
            "INSERT INTO adb_locations (location_id, serial) VALUES (?1, ?2)",
            params![tx.last_insert_rowid(), device.serial],
        )?;
        tx.commit()?;
        self.location(&new.label)
    }

    /// The connected, authorized device `serial`, or the only one when `serial` is empty.
    fn ready_device(&self, serial: &str) -> Result<Device, CatalogError> {
        let devices = self
            .android
            .transport
            .devices()
            .map_err(|e| CatalogError::Device(e.to_string()))?;
        let device = if serial.is_empty() {
            let mut ready = devices.iter().filter(|d| d.state == DeviceState::Device);
            match (ready.next(), ready.next()) {
                (Some(only), None) => only.clone(),
                (None, _) => {
                    let waiting: Vec<String> = devices
                        .iter()
                        .map(|d| format!("{} is {}", d.serial, d.state))
                        .collect();
                    return Err(CatalogError::Device(if waiting.is_empty() {
                        "no phone is connected over adb".to_owned()
                    } else {
                        format!("no phone is ready ({})", waiting.join(", "))
                    }));
                }
                (Some(_), Some(_)) => {
                    let serials: Vec<&str> = devices
                        .iter()
                        .filter(|d| d.state == DeviceState::Device)
                        .map(|d| d.serial.as_str())
                        .collect();
                    return Err(CatalogError::Device(format!(
                        "several phones are connected ({}); name one as adb:<serial>:<path>",
                        serials.join(", ")
                    )));
                }
            }
        } else {
            devices
                .into_iter()
                .find(|d| d.serial == serial)
                .ok_or_else(|| {
                    CatalogError::Device(format!("phone {serial} is not connected over adb"))
                })?
        };
        if device.state != DeviceState::Device {
            return Err(CatalogError::Device(not_ready(&device)));
        }
        Ok(device)
    }

    /// The phone's hardware serial (`ro.serialno`), which stays the same when the connection
    /// serial changes (wireless adb).
    fn hardware_serial(&self, serial: &str) -> Option<String> {
        let output = self
            .android
            .transport
            .shell(serial, "getprop ro.serialno")
            .ok()?;
        let hardware = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        (output.status == 0 && !hardware.is_empty() && !hardware.contains(':')).then_some(hardware)
    }

    fn android_fingerprint(&self, serial: &str, path: &str) -> Option<Fingerprint> {
        let hardware = self.hardware_serial(serial)?;
        format!("adb:{hardware}:{path}").parse().ok()
    }

    pub(super) fn locate_android(
        &self,
        location: &Location,
        serial: &str,
        path: &str,
    ) -> Whereabouts {
        let root = PathBuf::from(location.root.display());
        let Ok(devices) = self.android.transport.devices() else {
            return Whereabouts::Missing(root);
        };
        let expected = location
            .fingerprint
            .as_deref()
            .and_then(|f| f.parse::<Fingerprint>().ok())
            .filter(|f| f.as_str().starts_with("adb:"));
        let ready: Vec<&Device> = devices
            .iter()
            .filter(|d| d.state == DeviceState::Device)
            .collect();
        let Some(expected) = expected else {
            return if ready.iter().any(|d| d.serial == serial) {
                Whereabouts::Here(root)
            } else {
                Whereabouts::Missing(root)
            };
        };
        let found_here = ready
            .iter()
            .any(|d| d.serial == serial)
            .then(|| self.android_fingerprint(serial, path))
            .flatten();
        if found_here.as_ref() == Some(&expected) {
            return Whereabouts::Here(root);
        }
        let mut candidates: Vec<PathBuf> = ready
            .iter()
            .filter(|d| d.serial != serial)
            .filter(|d| self.android_fingerprint(&d.serial, path).as_ref() == Some(&expected))
            .map(|d| PathBuf::from(format!("adb:{}:{path}", d.serial)))
            .collect();
        candidates.sort();
        match (candidates.len(), ready.iter().any(|d| d.serial == serial)) {
            (1, _) => Whereabouts::Moved {
                from: root,
                to: candidates.remove(0),
            },
            (2.., _) => Whereabouts::Ambiguous { root, candidates },
            (_, true) => Whereabouts::OtherDevice {
                root,
                expected,
                found: found_here,
            },
            (_, false) => Whereabouts::Missing(root),
        }
    }

    /// Points a phone location at `spec` (`adb:<serial>:<path>`, serial empty for the only
    /// connected phone) and records that phone's fingerprint.
    pub(super) fn relocate_android(
        &self,
        location: &Location,
        spec: &str,
    ) -> Result<Location, CatalogError> {
        let invalid = || CatalogError::InvalidRoot(PathBuf::from(spec));
        let LocationRoot::Android { .. } = location.root else {
            return Err(invalid());
        };
        let (serial, path) = LocationRoot::parse_android(spec).ok_or_else(invalid)?;
        let device = self.ready_device(serial)?;
        let fingerprint = self.android_fingerprint(&device.serial, path);
        let mut conn = self.connect()?;
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE locations SET root_raw = ?2, fingerprint = ?3 WHERE id = ?1",
            params![
                location.id.get(),
                path.as_bytes(),
                fingerprint.as_ref().map(Fingerprint::as_str)
            ],
        )?;
        tx.execute(
            "UPDATE adb_locations SET serial = ?2 WHERE location_id = ?1",
            params![location.id.get(), device.serial],
        )?;
        tx.commit()?;
        self.location(&location.label)
    }

    pub(super) fn record_android_fingerprint(
        &self,
        location: &Location,
    ) -> Result<(), CatalogError> {
        let LocationRoot::Android { serial, path } = &location.root else {
            return Ok(());
        };
        if let Some(fingerprint) = self.android_fingerprint(serial, path) {
            self.connect()?.execute(
                "UPDATE locations SET fingerprint = ?2 WHERE id = ?1 AND fingerprint IS NULL",
                params![location.id.get(), fingerprint.as_str()],
            )?;
        }
        Ok(())
    }

    /// The source reading a phone location; fails clearly when the phone is not ready.
    pub(crate) fn adb_source(&self, location: &Location) -> Result<AdbSource, SourceError> {
        let LocationRoot::Android { serial, path } = &location.root else {
            return Err(SourceError::Remote(format!(
                "location {:?} is not on a phone",
                location.label
            )));
        };
        let unavailable = |why: String| SourceError::RootUnavailable {
            root: location.root.display(),
            source: std::io::Error::other(why),
        };
        let devices = self
            .android
            .transport
            .devices()
            .map_err(|e| unavailable(e.to_string()))?;
        match devices.iter().find(|d| &d.serial == serial) {
            None => {
                return Err(unavailable(
                    "the phone is not connected over adb".to_owned(),
                ));
            }
            Some(device) if device.state != DeviceState::Device => {
                return Err(unavailable(not_ready(device)));
            }
            Some(_) => {}
        }
        AdbSource::new(
            Arc::clone(&self.android.transport),
            serial.clone(),
            path.clone(),
            &location.ignore_patterns,
            self.android.agent.clone(),
            self.android.options.clone(),
        )
    }
}

fn not_ready(device: &Device) -> String {
    let hint = device
        .state
        .hint()
        .map_or_else(String::new, |hint| format!(": {hint}"));
    format!("phone {} is {}{hint}", device.serial, device.state)
}
