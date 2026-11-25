//! Exporting one location to a standalone `.symbolon` file and importing it into another
//! catalog (spec §4.4; file identity and conflicts in ADR-0005, imported locations in ADR-0013).
//!
//! An export is a catalog database with `meta.kind = 'export'` holding one location, its newest
//! completed scan and its entries. Rows are copied between two connections rather than with
//! `ATTACH`, so no path has to be valid UTF-8.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use rusqlite::types::Value;
use rusqlite::{Connection, OpenFlags, OptionalExtension as _, TransactionBehavior, params};

use super::{
    APPLICATION_ID, Catalog, CatalogError, Location, SCHEMA_VERSION, ms_to_system_time, now_ms,
    validate_label,
};
use crate::pathkey::PathEncoding;
use crate::scan::ScanError;
use crate::scan::lock::ScanLock;

/// What [`Catalog::export`] wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ExportSummary {
    /// The exported location.
    pub label: String,
    /// Entries written.
    pub entries: u64,
    /// When the exported scan started.
    pub scanned_at: SystemTime,
}

/// How [`Catalog::import`] resolves conflicts (ADR-0005). The default imports only when
/// nothing conflicts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ImportOptions {
    /// Import as a new location under this label instead of the file's.
    pub as_label: Option<String>,
    /// Replace the entries of this existing location: the file's device saved under another
    /// label.
    pub merge_into: Option<String>,
    /// Replace the location with the file's label even though it is another device.
    pub replace: bool,
    /// Replace entries even when the catalog's scan is newer than the file's.
    pub force: bool,
}

impl ImportOptions {
    /// Sets [`ImportOptions::as_label`].
    #[must_use]
    pub fn as_label(mut self, label: impl Into<String>) -> Self {
        self.as_label = Some(label.into());
        self
    }

    /// Sets [`ImportOptions::merge_into`].
    #[must_use]
    pub fn merge_into(mut self, label: impl Into<String>) -> Self {
        self.merge_into = Some(label.into());
        self
    }

    /// Sets [`ImportOptions::replace`].
    #[must_use]
    pub fn replace(mut self, replace: bool) -> Self {
        self.replace = replace;
        self
    }

    /// Sets [`ImportOptions::force`].
    #[must_use]
    pub fn force(mut self, force: bool) -> Self {
        self.force = force;
        self
    }
}

/// What [`Catalog::import`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ImportAction {
    /// A new location was added.
    Added,
    /// An existing location's scan and entries were replaced.
    Replaced,
}

/// Result of [`Catalog::import`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ImportSummary {
    /// The location that received the data.
    pub label: String,
    /// Whether it was added or replaced.
    pub action: ImportAction,
    /// Entries imported.
    pub entries: u64,
    /// When the imported scan started.
    pub scanned_at: SystemTime,
}

/// An import that needs the user's choice; each variant says which [`ImportOptions`] resolve
/// it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ImportConflict {
    /// Same location, but the catalog's scan is newer: resolved by `force`.
    #[error("location {label:?} has a newer scan than the file")]
    OlderScan {
        /// The location.
        label: String,
        /// When the file's scan started.
        imported: SystemTime,
        /// When the catalog's newest completed scan started.
        existing: SystemTime,
    },
    /// The label belongs to another device: resolved by `as_label` or `replace`.
    #[error("a location labelled {label:?} already exists for another device")]
    LabelTaken {
        /// The file's label.
        label: String,
    },
    /// The file's device is saved under another label: resolved by `merge_into` or
    /// `as_label`.
    #[error("the file's device is already saved as {existing:?}")]
    SameDevice {
        /// The file's label.
        label: String,
        /// The saved location with the same fingerprint.
        existing: String,
    },
}

/// Errors from [`Catalog::export`] and [`Catalog::import`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum TransferError {
    /// Looking up a location, opening a catalog, or a newer schema than this build knows.
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    /// A database error.
    #[error("catalog database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// Writing, renaming or copying a file failed.
    #[error("{context}: {source}")]
    Io {
        /// What was being done.
        context: String,
        /// The cause.
        #[source]
        source: std::io::Error,
    },
    /// The file is not a `.symbolon` export (another file, or a whole catalog).
    #[error("{} is not a Symbolon export", .0.display())]
    NotAnExport(PathBuf),
    /// Only completed scans are exported.
    #[error("location {0:?} has no completed scan to export")]
    NeverScanned(String),
    /// The import needs a choice; see [`ImportConflict`].
    #[error(transparent)]
    Conflict(#[from] ImportConflict),
    /// Entries of one path encoding cannot be merged into a location of another.
    #[error(
        "location {label:?} has {existing} paths but the file has {imported} paths; import it under another label"
    )]
    EncodingMismatch {
        /// The receiving location.
        label: String,
        /// Its encoding.
        existing: PathEncoding,
        /// The file's encoding.
        imported: PathEncoding,
    },
    /// The receiving location is being scanned.
    #[error("location {0:?} is being scanned; import again when the scan ends")]
    Busy(String),
}

fn io(context: impl Into<String>) -> impl FnOnce(std::io::Error) -> TransferError {
    let context = context.into();
    move |source| TransferError::Io { context, source }
}

/// A scan row as copied between files.
#[derive(Debug)]
struct ScanRow {
    columns: Vec<Value>,
    started_at: i64,
}

const SCAN_COLUMNS: &str = "started_at, finished_at, status, files, dirs, symlinks, bytes, \
                            hashed_files, hashed_bytes, reused_hashes, warnings, error";
const ENTRY_COLUMNS: &str =
    "rel_path_raw, path_key, parent_key, kind, size, mtime_ns, hash_partial, hash_full, flags";
const LOCATION_COPY_COLUMNS: &str = "root_kind, root_raw, path_encoding, fingerprint, hash_algo, \
                                     hash_policy, ignore_patterns";

impl Catalog {
    /// Writes the location labelled `label` to a standalone `.symbolon` file at `path`: its
    /// newest completed scan and the entries as recorded now. An existing file at `path` is
    /// replaced; a failed export leaves nothing behind.
    ///
    /// # Errors
    ///
    /// [`TransferError::Catalog`] for an unknown label, [`TransferError::NeverScanned`] without
    /// a completed scan, [`TransferError::Io`] or a database error while writing.
    pub fn export(&self, label: &str, path: &Path) -> Result<ExportSummary, TransferError> {
        let location = self.location(label)?;
        let partial = with_suffix(path, ".partial");
        remove_if_exists(&partial)?;
        match self.write_export(&location, &partial) {
            Ok(summary) => {
                fs::rename(&partial, path)
                    .map_err(io(format!("cannot move the export to {}", path.display())))?;
                Ok(summary)
            }
            Err(error) => {
                remove_if_exists(&partial).ok();
                Err(error)
            }
        }
    }

    fn write_export(
        &self,
        location: &Location,
        path: &Path,
    ) -> Result<ExportSummary, TransferError> {
        let mut src = self.connect_read_only()?;
        // One read transaction: a scan running meanwhile cannot mix two states.
        let src = src.transaction()?;
        let scan = newest_completed_scan(&src, location.id.get())?
            .ok_or_else(|| TransferError::NeverScanned(location.label.clone()))?;

        Catalog::open(path)?;
        let mut dst = Connection::open(path)?;
        let tx = dst.transaction()?;
        let location_row: Vec<Value> = src.query_row(
            &format!("SELECT {LOCATION_COPY_COLUMNS}, created_at FROM locations WHERE id = ?1"),
            [location.id.get()],
            |row| (0..8).map(|i| row.get(i)).collect(),
        )?;
        tx.execute(
            &format!(
                "INSERT INTO locations (label, {LOCATION_COPY_COLUMNS}, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
            ),
            rusqlite::params_from_iter(
                std::iter::once(Value::Text(location.label.clone())).chain(location_row),
            ),
        )?;
        let to = tx.last_insert_rowid();
        let scan_id = insert_scan(&tx, to, &scan)?;
        let entries = copy_entries(&src, &tx, location.id.get(), to, scan_id)?;
        let meta = [
            ("kind", "export".to_owned()),
            ("exported_at", now_ms().to_string()),
            ("exported_by", crate::VERSION.to_owned()),
            ("label", location.label.clone()),
            ("hash_algo", location.hash_algo.name().to_owned()),
            ("path_encoding", location.path_encoding.name().to_owned()),
        ];
        for (key, value) in meta {
            tx.execute(
                "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
                params![key, value],
            )?;
        }
        if let Some(fingerprint) = &location.fingerprint {
            tx.execute(
                "INSERT OR REPLACE INTO meta (key, value) VALUES ('fingerprint', ?1)",
                [fingerprint],
            )?;
        }
        tx.commit()?;
        // A file to hand around: no `-wal` and `-shm` companions.
        let _mode: String = dst.query_row("PRAGMA journal_mode = DELETE", [], |r| r.get(0))?;
        Ok(ExportSummary {
            label: location.label.clone(),
            entries,
            scanned_at: ms_to_system_time(scan.started_at),
        })
    }

    /// Imports a `.symbolon` file written by [`Catalog::export`], resolving conflicts with
    /// saved locations as ADR-0005 describes. A new location is added as
    /// [`LocationRoot::Imported`](super::LocationRoot::Imported); an existing one has its
    /// scans and entries replaced. The file itself is never modified; an older schema is
    /// migrated in a temporary copy.
    ///
    /// # Errors
    ///
    /// - [`TransferError::NotAnExport`] for other files, [`TransferError::Catalog`] for a
    ///   newer schema or an invalid or taken `as_label`.
    /// - [`TransferError::Conflict`] when a choice is needed; see [`ImportConflict`].
    /// - [`TransferError::EncodingMismatch`], [`TransferError::Busy`], or I/O and database
    ///   errors.
    pub fn import(
        &self,
        path: &Path,
        options: &ImportOptions,
    ) -> Result<ImportSummary, TransferError> {
        let file = ExportFile::open(path)?;
        match self.target(&file, options)? {
            Target::New(label) => self.add_imported(&file, &label),
            Target::Existing {
                location,
                check_age,
            } => self.replace_entries(&file, &location, check_age, options.force),
        }
    }

    /// Where the file's data goes, following the ADR-0005 table.
    fn target(&self, file: &ExportFile, options: &ImportOptions) -> Result<Target, TransferError> {
        if let Some(label) = &options.as_label {
            return Ok(Target::New(label.clone()));
        }
        if let Some(label) = &options.merge_into {
            return Ok(Target::Existing {
                location: self.location(label)?,
                check_age: true,
            });
        }
        match self.location(&file.label) {
            Ok(existing) if existing.fingerprint == file.fingerprint => Ok(Target::Existing {
                location: existing,
                check_age: true,
            }),
            Ok(existing) if options.replace => Ok(Target::Existing {
                location: existing,
                check_age: false,
            }),
            Ok(_) => Err(ImportConflict::LabelTaken {
                label: file.label.clone(),
            }
            .into()),
            Err(CatalogError::LocationNotFound(_)) => {
                let same_device = file.fingerprint.as_ref().and_then(|fingerprint| {
                    self.locations().ok().and_then(|all| {
                        all.into_iter()
                            .find(|l| l.fingerprint.as_ref() == Some(fingerprint))
                    })
                });
                match same_device {
                    Some(existing) => Err(ImportConflict::SameDevice {
                        label: file.label.clone(),
                        existing: existing.label,
                    }
                    .into()),
                    None => Ok(Target::New(file.label.clone())),
                }
            }
            Err(error) => Err(error.into()),
        }
    }

    fn add_imported(&self, file: &ExportFile, label: &str) -> Result<ImportSummary, TransferError> {
        validate_label(label)?;
        let mut conn = self.connect()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let taken: bool = tx.query_row(
            "SELECT EXISTS (SELECT 1 FROM locations WHERE label = ?1)",
            [label],
            |r| r.get(0),
        )?;
        if taken {
            return Err(CatalogError::LabelTaken(label.to_owned()).into());
        }
        let row: Vec<Value> = file.conn.query_row(
            &format!("SELECT {LOCATION_COPY_COLUMNS} FROM locations WHERE id = ?1"),
            [file.location_id],
            |row| (0..7).map(|i| row.get(i)).collect(),
        )?;
        let values = std::iter::once(Value::Text(label.to_owned()))
            .chain(row)
            .chain(std::iter::once(Value::Integer(now_ms())));
        tx.execute(
            &format!(
                "INSERT INTO locations (label, {LOCATION_COPY_COLUMNS}, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
            ),
            rusqlite::params_from_iter(values),
        )?;
        let to = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO imported_locations (location_id, exported_at, exported_by, imported_at) \
             VALUES (?1, ?2, ?3, ?4)",
            params![to, file.exported_at, file.exported_by, now_ms()],
        )?;
        let scan_id = insert_scan(&tx, to, &file.scan)?;
        let entries = copy_entries(&file.conn, &tx, file.location_id, to, scan_id)?;
        tx.commit()?;
        Ok(ImportSummary {
            label: label.to_owned(),
            action: ImportAction::Added,
            entries,
            scanned_at: ms_to_system_time(file.scan.started_at),
        })
    }

    fn replace_entries(
        &self,
        file: &ExportFile,
        location: &Location,
        check_age: bool,
        force: bool,
    ) -> Result<ImportSummary, TransferError> {
        if location.path_encoding != file.path_encoding {
            return Err(TransferError::EncodingMismatch {
                label: location.label.clone(),
                existing: location.path_encoding,
                imported: file.path_encoding,
            });
        }
        let _lock = ScanLock::acquire(self, location).map_err(|error| match error {
            ScanError::AlreadyScanning { label, .. } => TransferError::Busy(label),
            ScanError::Lock(source) => TransferError::Io {
                context: "cannot lock the location".to_owned(),
                source,
            },
            other => TransferError::Catalog(CatalogError::Corrupt(other.to_string())),
        })?;
        let mut conn = self.connect()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let id = location.id.get();
        if check_age
            && !force
            && let Some(existing) = newest_completed_scan(&tx, id)?
            && existing.started_at > file.scan.started_at
        {
            return Err(ImportConflict::OlderScan {
                label: location.label.clone(),
                imported: ms_to_system_time(file.scan.started_at),
                existing: ms_to_system_time(existing.started_at),
            }
            .into());
        }
        tx.execute("DELETE FROM entries WHERE location_id = ?1", [id])?;
        tx.execute("DELETE FROM scans WHERE location_id = ?1", [id])?;
        // The digests now come from the file, so its algorithm applies.
        tx.execute(
            "UPDATE locations SET hash_algo = ?2, fingerprint = coalesce(?3, fingerprint) \
             WHERE id = ?1",
            params![id, file.hash_algo, file.fingerprint],
        )?;
        tx.execute(
            "UPDATE imported_locations SET exported_at = ?2, exported_by = ?3, imported_at = ?4 \
             WHERE location_id = ?1",
            params![id, file.exported_at, file.exported_by, now_ms()],
        )?;
        let scan_id = insert_scan(&tx, id, &file.scan)?;
        let entries = copy_entries(&file.conn, &tx, file.location_id, id, scan_id)?;
        tx.commit()?;
        Ok(ImportSummary {
            label: location.label.clone(),
            action: ImportAction::Replaced,
            entries,
            scanned_at: ms_to_system_time(file.scan.started_at),
        })
    }
}

enum Target {
    New(String),
    Existing { location: Location, check_age: bool },
}

/// An export file opened for reading, possibly through a migrated temporary copy.
struct ExportFile {
    conn: Connection,
    _copy: Option<TempCopy>,
    location_id: i64,
    label: String,
    fingerprint: Option<String>,
    hash_algo: String,
    path_encoding: PathEncoding,
    exported_at: i64,
    exported_by: String,
    scan: ScanRow,
}

impl ExportFile {
    fn open(path: &Path) -> Result<Self, TransferError> {
        let not_export = || TransferError::NotAnExport(path.to_path_buf());
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let header = || -> rusqlite::Result<(i32, u32)> {
            Ok((
                conn.pragma_query_value(None, "application_id", |r| r.get(0))?,
                conn.pragma_query_value(None, "user_version", |r| r.get(0))?,
            ))
        };
        let (application_id, version) = header().map_err(|_| not_export())?;
        if application_id != APPLICATION_ID {
            return Err(not_export());
        }
        if version > SCHEMA_VERSION {
            return Err(CatalogError::TooNew {
                found: version,
                supported: SCHEMA_VERSION,
            }
            .into());
        }
        if meta(&conn, "kind")?.as_deref() != Some("export") {
            return Err(not_export());
        }
        let (conn, copy) = if version < SCHEMA_VERSION {
            drop(conn);
            let copy = TempCopy::new(path)?;
            Catalog::open(&copy.path)?;
            let conn = Connection::open_with_flags(&copy.path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
            (conn, Some(copy))
        } else {
            (conn, None)
        };
        let required = |key: &str| meta(&conn, key)?.ok_or_else(not_export);
        let (location_id, count): (i64, i64) =
            conn.query_row("SELECT min(id), count(*) FROM locations", [], |r| {
                Ok((r.get(0).unwrap_or(0), r.get(1)?))
            })?;
        if count != 1 {
            return Err(not_export());
        }
        let scan = newest_completed_scan(&conn, location_id)?.ok_or_else(not_export)?;
        Ok(Self {
            location_id,
            label: required("label")?,
            fingerprint: meta(&conn, "fingerprint")?,
            hash_algo: required("hash_algo")?,
            path_encoding: required("path_encoding")?
                .parse()
                .map_err(|_| not_export())?,
            exported_at: required("exported_at")?.parse().map_err(|_| not_export())?,
            exported_by: required("exported_by")?,
            scan,
            conn,
            _copy: copy,
        })
    }
}

fn meta(conn: &Connection, key: &str) -> Result<Option<String>, TransferError> {
    Ok(conn
        .query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
        .optional()?)
}

/// A temporary copy of a file, removed with everything SQLite and migrations put next to it.
///
/// It lives in a new directory only its owner can enter, created exclusively, so other users
/// of the computer can neither read it nor plant files in its place.
struct TempCopy {
    path: PathBuf,
    _dir: tempfile::TempDir,
}

impl TempCopy {
    fn new(original: &Path) -> Result<Self, TransferError> {
        let dir = tempfile::Builder::new()
            .prefix("symbolon-import-")
            .tempdir()
            .map_err(io(
                "cannot create a temporary directory to migrate the file",
            ))?;
        let path = dir.path().join("export.symbolon");
        fs::copy(original, &path).map_err(io(format!(
            "cannot copy {} to migrate it",
            original.display()
        )))?;
        Ok(Self { path, _dir: dir })
    }
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

fn remove_if_exists(path: &Path) -> Result<(), TransferError> {
    match fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            Err(io(format!("cannot remove {}", path.display()))(e))
        }
        _ => Ok(()),
    }
}

fn newest_completed_scan(
    conn: &Connection,
    location_id: i64,
) -> Result<Option<ScanRow>, TransferError> {
    Ok(conn
        .query_row(
            &format!(
                "SELECT {SCAN_COLUMNS} FROM scans WHERE location_id = ?1 AND status = 'completed' \
                 ORDER BY id DESC LIMIT 1"
            ),
            [location_id],
            |row| {
                Ok(ScanRow {
                    started_at: row.get(0)?,
                    columns: (0..12).map(|i| row.get(i)).collect::<Result<_, _>>()?,
                })
            },
        )
        .optional()?)
}

fn insert_scan(dst: &Connection, location_id: i64, scan: &ScanRow) -> rusqlite::Result<i64> {
    dst.execute(
        &format!(
            "INSERT INTO scans (location_id, {SCAN_COLUMNS}) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"
        ),
        rusqlite::params_from_iter(
            std::iter::once(&Value::Integer(location_id)).chain(&scan.columns),
        ),
    )?;
    Ok(dst.last_insert_rowid())
}

/// Copies the entries of location `from` in `src` to location `to` in `dst`, all seen by
/// scan `scan`.
fn copy_entries(
    src: &Connection,
    dst: &Connection,
    from: i64,
    to: i64,
    scan: i64,
) -> rusqlite::Result<u64> {
    let mut read = src.prepare(&format!(
        "SELECT {ENTRY_COLUMNS} FROM entries WHERE location_id = ?1"
    ))?;
    let mut write = dst.prepare(&format!(
        "INSERT INTO entries (location_id, seen_scan_id, {ENTRY_COLUMNS}) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"
    ))?;
    let mut rows = read.query([from])?;
    let mut copied = 0;
    while let Some(row) = rows.next()? {
        let mut values = vec![Value::Integer(to), Value::Integer(scan)];
        for i in 0..9 {
            values.push(row.get(i)?);
        }
        write.execute(rusqlite::params_from_iter(values))?;
        copied += 1;
    }
    Ok(copied)
}
