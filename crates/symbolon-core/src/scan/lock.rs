//! One scan per location at a time, across threads and processes (ADR-0012).

use std::ffi::OsString;
use std::fs::{File, TryLockError};
use std::path::{Path, PathBuf};

use rusqlite::{OptionalExtension as _, params};

use super::ScanError;
use crate::catalog::{Catalog, Location, ms_to_system_time, private};

/// An exclusive OS lock on the location's lock file, held for the scan's lifetime. The OS
/// releases it when the file is closed or the process dies, so a crashed scan never blocks
/// the next one.
#[derive(Debug)]
pub(crate) struct ScanLock {
    _file: File,
}

impl ScanLock {
    pub(crate) fn acquire(catalog: &Catalog, location: &Location) -> Result<Self, ScanError> {
        let dir = lock_dir(catalog.path());
        private::create_dir_all(&dir).map_err(ScanError::Lock)?;
        let path = dir.join(format!("{}.lock", location.id.get()));
        private::create_file(&path).map_err(ScanError::Lock)?;
        let file = File::options()
            .write(true)
            .open(&path)
            .map_err(ScanError::Lock)?;
        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file }),
            Err(TryLockError::WouldBlock) => Err(ScanError::AlreadyScanning {
                label: location.label.clone(),
                since: running_since(catalog, location)?,
            }),
            Err(TryLockError::Error(e)) => Err(ScanError::Lock(e)),
        }
    }
}

/// `<catalog>-locks`, next to the database like SQLite's own `-wal` and `-shm` files.
fn lock_dir(catalog: &Path) -> PathBuf {
    let mut name = OsString::from(catalog.as_os_str());
    name.push("-locks");
    PathBuf::from(name)
}

/// When the scan holding the lock started; `None` if it has not recorded itself yet.
fn running_since(
    catalog: &Catalog,
    location: &Location,
) -> Result<Option<std::time::SystemTime>, ScanError> {
    let conn = catalog.connect_read_only()?;
    let started: Option<i64> = conn
        .query_row(
            "SELECT started_at FROM scans WHERE location_id = ?1 AND status = 'running' \
             ORDER BY id DESC LIMIT 1",
            params![location.id.get()],
            |row| row.get(0),
        )
        .optional()?;
    Ok(started.map(ms_to_system_time))
}
