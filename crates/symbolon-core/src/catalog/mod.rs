//! The local catalog: one SQLite database holding locations, scans and file entries.
//!
//! [`Catalog`] is a cheap handle around the database path. It manages locations directly;
//! scanning and reading go through `Scanner` and `Query`, which open their own connections.
//! The database runs in WAL mode, so readers see committed data while a scan writes.
//!
//! The schema is a versioned public contract (`docs/schema.md`): opening a catalog applies the
//! forward-only migrations it is missing, after backing it up, and refuses catalogs written by a
//! newer version.

mod locate;
mod schema;
mod transfer;

use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{
    Connection, ErrorCode, OpenFlags, OptionalExtension as _, TransactionBehavior, params,
};

pub use self::locate::Whereabouts;
pub use self::schema::SCHEMA_VERSION;
pub use self::transfer::{
    ExportSummary, ImportAction, ImportConflict, ImportOptions, ImportSummary, TransferError,
};
use crate::hash::HashAlgo;
use crate::identity::{Fingerprint, SystemVolumes, Volumes};
use crate::pathkey::{self, PathEncoding};

/// SQLite `application_id` identifying Symbolon catalogs and exports (ASCII `SYMB`, ADR-0005).
pub const APPLICATION_ID: i32 = 0x5359_4D42;

const BUSY_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_LABEL_CHARS: usize = 64;

/// Errors from opening or changing the catalog.
#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    /// An SQLite error.
    #[error("catalog database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// A filesystem error outside SQLite (for example resolving a location root).
    #[error("{context}: {source}")]
    Io {
        /// What was being done.
        context: String,
        /// The underlying error.
        source: std::io::Error,
    },
    /// The file exists but is not a Symbolon catalog.
    #[error("{} is not a Symbolon catalog", .0.display())]
    NotSymbolon(PathBuf),
    /// The catalog was written by a newer Symbolon with a schema this build does not know.
    #[error(
        "catalog schema version {found} is newer than this build supports ({supported}); upgrade Symbolon"
    )]
    TooNew {
        /// Schema version in the file.
        found: u32,
        /// Newest schema version this build understands.
        supported: u32,
    },
    /// A label that is empty, too long, or contains `:`, `/`, `\`, control characters or
    /// surrounding whitespace.
    #[error(
        "invalid location label {0:?}: use 1-64 characters without ':', '/', '\\' or surrounding spaces"
    )]
    InvalidLabel(String),
    /// Another location already uses this label.
    #[error("a location labelled {0:?} already exists")]
    LabelTaken(String),
    /// No location has this label.
    #[error("no location labelled {0:?}")]
    LocationNotFound(String),
    /// A location root that is not an existing directory.
    #[error("location root {} is not a directory", .0.display())]
    InvalidRoot(PathBuf),
    /// A stored value this build cannot interpret.
    #[error("catalog contains an invalid value: {0}")]
    Corrupt(String),
}

/// Handle to a catalog database file.
///
/// Cloning is cheap; connections are opened per use, so handles can move between threads.
#[derive(Clone)]
pub struct Catalog {
    path: PathBuf,
    /// The mounted filesystems, for device fingerprints (ADR-0016).
    volumes: Arc<dyn Volumes>,
}

impl fmt::Debug for Catalog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Catalog")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl Catalog {
    /// Opens the catalog at `path`, creating it if the file does not exist and migrating it to
    /// [`SCHEMA_VERSION`] if it is older.
    ///
    /// Before migrating an existing catalog, a copy is written next to it as
    /// `<file>.v<old version>.bak`.
    ///
    /// # Errors
    ///
    /// - [`CatalogError::NotSymbolon`] if the file is not a Symbolon catalog.
    /// - [`CatalogError::TooNew`] if a newer Symbolon wrote it.
    /// - [`CatalogError::Sqlite`] for database errors, for example a missing parent directory.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, CatalogError> {
        let path = path.as_ref().to_path_buf();
        // SQLite would create it with the umask's permissions; catalogs list private files.
        // Failures are left for SQLite to report.
        private::create_file(&path).ok();
        let mut conn = Connection::open(&path)?;
        configure(&conn).map_err(|e| not_a_database(e, &path))?;
        migrate(&mut conn, &path)?;
        Ok(Self {
            path,
            volumes: Arc::new(SystemVolumes),
        })
    }

    /// This catalog, reading mounted filesystems from `volumes` instead of the system when
    /// it records and checks device fingerprints (tests, platforms without detection).
    #[must_use]
    pub fn with_volumes(mut self, volumes: Arc<dyn Volumes>) -> Self {
        self.volumes = volumes;
        self
    }

    /// Path of the database file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Adds a location.
    ///
    /// A local root is resolved to an absolute, canonical path and must be an existing
    /// directory.
    ///
    /// # Errors
    ///
    /// [`CatalogError::InvalidLabel`], [`CatalogError::LabelTaken`],
    /// [`CatalogError::InvalidRoot`], or a database error.
    pub fn add_location(&self, new: NewLocation) -> Result<Location, CatalogError> {
        validate_label(&new.label)?;
        let root = match &new.root {
            LocationRoot::Local(root) => root,
            LocationRoot::Foreign { display, .. } | LocationRoot::Imported { display, .. } => {
                return Err(CatalogError::InvalidRoot(PathBuf::from(display)));
            }
        };
        let root = std::fs::canonicalize(root)
            .ok()
            .filter(|p| p.is_dir())
            .ok_or_else(|| CatalogError::InvalidRoot(root.clone()))?;
        let mut root_raw = Vec::new();
        pathkey::push_native(&mut root_raw, root.as_os_str());
        let fingerprint = self.fingerprint_of(&root)?;

        let conn = self.connect()?;
        let inserted = conn.execute(
            "INSERT INTO locations (label, root_kind, root_raw, path_encoding, fingerprint, \
             hash_algo, hash_policy, ignore_patterns, created_at) \
             VALUES (?1, 'local', ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                new.label,
                root_raw,
                PathEncoding::native().name(),
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
        self.location(&new.label)
    }

    /// The location with `label`.
    ///
    /// # Errors
    ///
    /// [`CatalogError::LocationNotFound`], or a database error.
    pub fn location(&self, label: &str) -> Result<Location, CatalogError> {
        let conn = self.connect_read_only()?;
        conn.query_row(
            &format!("SELECT {LOCATION_COLUMNS} FROM locations WHERE label = ?1"),
            [label],
            RawLocation::from_row,
        )
        .optional()?
        .ok_or_else(|| CatalogError::LocationNotFound(label.to_owned()))?
        .into_location()
    }

    /// All locations, ordered by label.
    ///
    /// # Errors
    ///
    /// A database error, or [`CatalogError::Corrupt`] for unreadable stored values.
    pub fn locations(&self) -> Result<Vec<Location>, CatalogError> {
        let conn = self.connect_read_only()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {LOCATION_COLUMNS} FROM locations ORDER BY label"
        ))?;
        let rows = stmt.query_map([], RawLocation::from_row)?;
        rows.map(|row| row?.into_location()).collect()
    }

    /// Removes a location with all its scans and entries.
    ///
    /// # Errors
    ///
    /// [`CatalogError::LocationNotFound`], or a database error.
    pub fn remove_location(&self, label: &str) -> Result<(), CatalogError> {
        let conn = self.connect()?;
        if conn.execute("DELETE FROM locations WHERE label = ?1", [label])? == 0 {
            return Err(CatalogError::LocationNotFound(label.to_owned()));
        }
        Ok(())
    }

    /// Gives a location a new label, its identity in every command.
    ///
    /// # Errors
    ///
    /// [`CatalogError::LocationNotFound`], [`CatalogError::InvalidLabel`],
    /// [`CatalogError::LabelTaken`], or a database error.
    pub fn rename(&self, label: &str, new_label: &str) -> Result<Location, CatalogError> {
        validate_label(new_label)?;
        let renamed = self.connect()?.execute(
            "UPDATE locations SET label = ?2 WHERE label = ?1",
            params![label, new_label],
        );
        match renamed {
            Ok(0) => Err(CatalogError::LocationNotFound(label.to_owned())),
            Err(rusqlite::Error::SqliteFailure(e, _))
                if e.code == ErrorCode::ConstraintViolation =>
            {
                Err(CatalogError::LabelTaken(new_label.to_owned()))
            }
            other => {
                other?;
                self.location(new_label)
            }
        }
    }

    /// Changes when a location's files are hashed. Nothing is hashed now; the next scan
    /// follows the new policy.
    ///
    /// # Errors
    ///
    /// [`CatalogError::LocationNotFound`], or a database error.
    pub fn set_hash_policy(
        &self,
        label: &str,
        policy: HashPolicy,
    ) -> Result<Location, CatalogError> {
        let updated = self.connect()?.execute(
            "UPDATE locations SET hash_policy = ?2 WHERE label = ?1",
            params![label, policy.name()],
        )?;
        if updated == 0 {
            return Err(CatalogError::LocationNotFound(label.to_owned()));
        }
        self.location(label)
    }

    /// A read-write connection, configured for this catalog.
    pub(crate) fn connect(&self) -> Result<Connection, CatalogError> {
        let conn = Connection::open_with_flags(
            &self.path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        configure(&conn)?;
        Ok(conn)
    }

    /// A read-only connection; in WAL mode it reads committed data while a writer works.
    pub(crate) fn connect_read_only(&self) -> Result<Connection, CatalogError> {
        let conn = Connection::open_with_flags(
            &self.path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        conn.busy_timeout(BUSY_TIMEOUT)?;
        conn.pragma_update(None, "query_only", true)?;
        Ok(conn)
    }
}

fn configure(conn: &Connection) -> rusqlite::Result<()> {
    conn.busy_timeout(BUSY_TIMEOUT)?;
    // WAL is persistent once set; repeating it is cheap and repairs a catalog copied without it.
    let _mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    Ok(())
}

fn not_a_database(error: rusqlite::Error, path: &Path) -> CatalogError {
    match &error {
        rusqlite::Error::SqliteFailure(e, _) if e.code == ErrorCode::NotADatabase => {
            CatalogError::NotSymbolon(path.to_path_buf())
        }
        _ => CatalogError::Sqlite(error),
    }
}

fn migrate(conn: &mut Connection, path: &Path) -> Result<(), CatalogError> {
    let application_id: i32 = conn.pragma_query_value(None, "application_id", |r| r.get(0))?;
    let version: u32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    let empty: bool =
        conn.query_row("SELECT NOT EXISTS (SELECT 1 FROM sqlite_schema)", [], |r| {
            r.get(0)
        })?;
    if application_id != APPLICATION_ID && !(application_id == 0 && empty) {
        return Err(CatalogError::NotSymbolon(path.to_path_buf()));
    }
    if version > SCHEMA_VERSION {
        return Err(CatalogError::TooNew {
            found: version,
            supported: SCHEMA_VERSION,
        });
    }
    if version == SCHEMA_VERSION {
        return Ok(());
    }
    if version > 0 {
        // The name as bytes, so a non-Unicode path is not written somewhere else.
        let mut backup = path.as_os_str().to_owned();
        backup.push(format!(".v{version}.bak"));
        private::create_file(Path::new(&backup)).ok();
        conn.execute("VACUUM INTO CAST(?1 AS TEXT)", [backup.as_encoded_bytes()])?;
    }
    for (index, sql) in schema::MIGRATIONS.iter().enumerate() {
        let target = u32::try_from(index + 1)
            .map_err(|_| CatalogError::Corrupt("too many migrations".into()))?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        // Re-read under the write lock: another process may have migrated meanwhile.
        let current: u32 = tx.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if current >= target {
            continue;
        }
        tx.execute_batch(sql)?;
        if target == 1 {
            tx.pragma_update(None, "application_id", APPLICATION_ID)?;
            tx.execute(
                "INSERT INTO meta (key, value) VALUES ('kind', 'catalog'), ('created_at', ?1), \
                 ('created_by', ?2)",
                params![now_ms().to_string(), crate::VERSION],
            )?;
        }
        tx.pragma_update(None, "user_version", target)?;
        tx.commit()?;
    }
    Ok(())
}

fn validate_label(label: &str) -> Result<(), CatalogError> {
    let valid = !label.is_empty()
        && label.chars().count() <= MAX_LABEL_CHARS
        && label.trim() == label
        && !label
            .chars()
            .any(|c| matches!(c, ':' | '/' | '\\') || c.is_control());
    if valid {
        Ok(())
    } else {
        Err(CatalogError::InvalidLabel(label.to_owned()))
    }
}

/// Human-readable form of a raw absolute path, escaping like [`pathkey::PathKey`].
fn display_raw(encoding: PathEncoding, raw: &[u8]) -> String {
    let stripped: Vec<u8> = raw.iter().copied().skip_while(|&b| b == b'/').collect();
    let prefix = &raw[..raw.len() - stripped.len()];
    pathkey::RelPath::from_raw(encoding, stripped).map_or_else(
        |_| String::from_utf8_lossy(raw).into_owned(),
        |rel| format!("{}{}", String::from_utf8_lossy(prefix), rel.key()),
    )
}

/// Milliseconds since the Unix epoch, the catalog's timestamp unit.
pub(crate) fn now_ms() -> i64 {
    system_time_to_ms(SystemTime::now())
}

pub(crate) fn system_time_to_ms(time: SystemTime) -> i64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(after) => i64::try_from(after.as_millis()).unwrap_or(i64::MAX),
        Err(before) => i64::try_from(before.duration().as_millis()).map_or(i64::MIN, |ms| -ms),
    }
}

pub(crate) fn ms_to_system_time(ms: i64) -> SystemTime {
    let magnitude = Duration::from_millis(ms.unsigned_abs());
    if ms >= 0 {
        UNIX_EPOCH + magnitude
    } else {
        UNIX_EPOCH - magnitude
    }
}

/// Database id of a location.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LocationId(pub(crate) i64);

impl LocationId {
    /// The numeric id.
    pub const fn get(self) -> i64 {
        self.0
    }
}

/// When a location's files are hashed (spec §5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum HashPolicy {
    /// Hash only what a comparison needs. Suits storage that stays connected.
    #[default]
    Lazy,
    /// Hash every file during the scan, so the catalog is fully comparable offline. Suits
    /// removable media you will not have later.
    Eager,
}

impl HashPolicy {
    /// The stable name stored in the catalog.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Lazy => "lazy",
            Self::Eager => "eager",
        }
    }
}

impl fmt::Display for HashPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for HashPolicy {
    type Err = CatalogError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "lazy" => Ok(Self::Lazy),
            "eager" => Ok(Self::Eager),
            _ => Err(CatalogError::Corrupt(format!("unknown hash policy {s:?}"))),
        }
    }
}

/// Where a location's files live.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum LocationRoot {
    /// A directory on a local or mounted filesystem.
    Local(PathBuf),
    /// A local root recorded on a platform with a different path encoding (for example a Linux
    /// location in a catalog opened on Windows). It can be compared but not scanned here.
    Foreign {
        /// Encoding of the recorded path.
        encoding: PathEncoding,
        /// The path as recorded, with non-Unicode bytes shown as `%XX`.
        display: String,
    },
    /// A location imported from a `.symbolon` file ([`Catalog::import`]): its files are on
    /// another computer. It can be compared but not scanned here.
    Imported {
        /// Encoding of the recorded path.
        encoding: PathEncoding,
        /// The root on the computer that exported it, with non-Unicode bytes shown as `%XX`.
        display: String,
    },
}

impl LocationRoot {
    /// The root for people and scripts: the path as it is when it is valid Unicode, otherwise
    /// with the invalid bytes escaped as `%XX` like path keys. Windows' `\\?\` prefix on
    /// drive paths is left out.
    pub fn display(&self) -> String {
        match self {
            Self::Local(path) => path.to_str().map_or_else(
                || {
                    let mut raw = Vec::new();
                    pathkey::push_native(&mut raw, path.as_os_str());
                    display_raw(PathEncoding::native(), &raw)
                },
                |text| strip_verbatim(text).to_owned(),
            ),
            Self::Foreign { display, .. } | Self::Imported { display, .. } => {
                strip_verbatim(display).to_owned()
            }
        }
    }
}

/// `\\?\C:\dir` → `C:\dir`; other paths (including `\\?\UNC\…`) unchanged.
fn strip_verbatim(path: &str) -> &str {
    match path.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1..3) == Some(b":\\") => rest,
        _ => path,
    }
}

/// A location to add with [`Catalog::add_location`].
#[derive(Debug, Clone)]
pub struct NewLocation {
    label: String,
    root: LocationRoot,
    hash_algo: HashAlgo,
    hash_policy: HashPolicy,
    ignore_patterns: Vec<String>,
}

impl NewLocation {
    /// A local directory with the default algorithm (BLAKE3) and policy (lazy).
    pub fn local(label: impl Into<String>, root: impl Into<PathBuf>) -> Self {
        Self {
            label: label.into(),
            root: LocationRoot::Local(root.into()),
            hash_algo: HashAlgo::default(),
            hash_policy: HashPolicy::default(),
            ignore_patterns: Vec::new(),
        }
    }

    /// Sets the hash algorithm.
    #[must_use]
    pub fn hash_algo(mut self, algo: HashAlgo) -> Self {
        self.hash_algo = algo;
        self
    }

    /// Sets the hash policy.
    #[must_use]
    pub fn hash_policy(mut self, policy: HashPolicy) -> Self {
        self.hash_policy = policy;
        self
    }

    /// Sets ignore patterns, in `.gitignore` syntax, matched relative to the root.
    #[must_use]
    pub fn ignore_patterns(
        mut self,
        patterns: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.ignore_patterns = patterns.into_iter().map(Into::into).collect();
        self
    }
}

/// A saved location.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Location {
    /// Database id.
    pub id: LocationId,
    /// User-assigned label, the location's identity.
    pub label: String,
    /// Where the files live.
    pub root: LocationRoot,
    /// Encoding of the location's relative paths.
    pub path_encoding: PathEncoding,
    /// Best-effort device fingerprint ([`Fingerprint`], ADR-0016); `None` when the root's
    /// filesystem has none or it was never detected.
    pub fingerprint: Option<String>,
    /// Hash algorithm for all of the location's digests.
    pub hash_algo: HashAlgo,
    /// When files are hashed.
    pub hash_policy: HashPolicy,
    /// Ignore patterns in `.gitignore` syntax.
    pub ignore_patterns: Vec<String>,
    /// When the location was added.
    pub created_at: SystemTime,
}

const LOCATION_COLUMNS: &str = "id, label, root_kind, root_raw, path_encoding, fingerprint, \
     hash_algo, hash_policy, ignore_patterns, created_at, \
     EXISTS (SELECT 1 FROM imported_locations i WHERE i.location_id = locations.id)";

struct RawLocation {
    id: i64,
    label: String,
    root_kind: String,
    root_raw: Vec<u8>,
    path_encoding: String,
    fingerprint: Option<String>,
    hash_algo: String,
    hash_policy: String,
    ignore_patterns: String,
    created_at: i64,
    imported: bool,
}

impl RawLocation {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            label: row.get(1)?,
            root_kind: row.get(2)?,
            root_raw: row.get(3)?,
            path_encoding: row.get(4)?,
            fingerprint: row.get(5)?,
            hash_algo: row.get(6)?,
            hash_policy: row.get(7)?,
            ignore_patterns: row.get(8)?,
            created_at: row.get(9)?,
            imported: row.get(10)?,
        })
    }

    fn into_location(self) -> Result<Location, CatalogError> {
        let corrupt =
            |what: &str| CatalogError::Corrupt(format!("{what} of location {:?}", self.label));
        let path_encoding: PathEncoding = self
            .path_encoding
            .parse()
            .map_err(|_| corrupt("path encoding"))?;
        let root = match self.root_kind.as_str() {
            "local" if self.imported => LocationRoot::Imported {
                encoding: path_encoding,
                display: display_raw(path_encoding, &self.root_raw),
            },
            "local" if path_encoding == PathEncoding::native() => LocationRoot::Local(
                pathkey::native_from_bytes(&self.root_raw).ok_or_else(|| corrupt("root"))?,
            ),
            "local" => LocationRoot::Foreign {
                encoding: path_encoding,
                display: display_raw(path_encoding, &self.root_raw),
            },
            _ => return Err(corrupt("root kind")),
        };
        Ok(Location {
            id: LocationId(self.id),
            hash_algo: self
                .hash_algo
                .parse()
                .map_err(|_| corrupt("hash algorithm"))?,
            hash_policy: self.hash_policy.parse()?,
            ignore_patterns: self
                .ignore_patterns
                .lines()
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
                .collect(),
            created_at: ms_to_system_time(self.created_at),
            label: self.label,
            root,
            path_encoding,
            fingerprint: self.fingerprint,
        })
    }
}

/// Files and directories readable only by their owner on Unix. Elsewhere the default
/// permissions apply (on Windows, those of the parent directory).
pub(crate) mod private {
    use std::fs;
    use std::io;
    use std::path::Path;

    /// Creates `path` empty if it does not exist.
    pub(crate) fn create_file(path: &Path) -> io::Result<()> {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        match options.open(path) {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// Creates `path` and missing parents; the ones created here are private.
    pub(crate) fn create_dir_all(path: &Path) -> io::Result<()> {
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        builder.create(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> (tempfile::TempDir, Catalog) {
        let dir = tempfile::tempdir().unwrap();
        let catalog = Catalog::open(dir.path().join("catalog.db")).unwrap();
        (dir, catalog)
    }

    #[test]
    fn locations_can_be_renamed_and_change_policy() {
        let (dir, catalog) = catalog();
        for label in ["photos", "music"] {
            catalog
                .add_location(NewLocation::local(label, dir.path()))
                .unwrap();
        }
        let renamed = catalog.rename("photos", "pictures").unwrap();
        assert_eq!(renamed.label, "pictures");
        assert!(matches!(
            catalog.location("photos"),
            Err(CatalogError::LocationNotFound(_))
        ));
        assert!(matches!(
            catalog.rename("pictures", "music"),
            Err(CatalogError::LabelTaken(label)) if label == "music"
        ));
        assert!(matches!(
            catalog.rename("pictures", "a:b"),
            Err(CatalogError::InvalidLabel(_))
        ));
        assert!(matches!(
            catalog.rename("nope", "x"),
            Err(CatalogError::LocationNotFound(_))
        ));

        let eager = catalog
            .set_hash_policy("pictures", HashPolicy::Eager)
            .unwrap();
        assert_eq!(eager.hash_policy, HashPolicy::Eager);
        assert!(matches!(
            catalog.set_hash_policy("nope", HashPolicy::Lazy),
            Err(CatalogError::LocationNotFound(_))
        ));
    }

    #[test]
    fn roots_display_without_verbatim_prefix_and_with_escapes() {
        assert_eq!(strip_verbatim(r"\\?\C:\data"), r"C:\data");
        assert_eq!(strip_verbatim(r"\\?\UNC\host\share"), r"\\?\UNC\host\share");
        assert_eq!(strip_verbatim("/home/me"), "/home/me");
        assert_eq!(
            LocationRoot::Local("/srv/data".into()).display(),
            "/srv/data"
        );
        let foreign = LocationRoot::Foreign {
            encoding: PathEncoding::Posix,
            display: "/bad%FF".into(),
        };
        assert_eq!(foreign.display(), "/bad%FF");
    }

    #[cfg(unix)]
    #[test]
    fn non_unicode_local_roots_are_escaped() {
        use std::os::unix::ffi::OsStrExt as _;
        let root = PathBuf::from(std::ffi::OsStr::from_bytes(b"/data/bad\xffname"));
        assert_eq!(LocationRoot::Local(root).display(), "/data/bad%FFname");
    }

    #[test]
    fn new_catalog_is_marked_and_versioned() {
        let (_dir, catalog) = catalog();
        let conn = catalog.connect().unwrap();
        let app_id: i32 = conn
            .pragma_query_value(None, "application_id", |r| r.get(0))
            .unwrap();
        let version: u32 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        let mode: String = conn
            .pragma_query_value(None, "journal_mode", |r| r.get(0))
            .unwrap();
        let kind: String = conn
            .query_row("SELECT value FROM meta WHERE key = 'kind'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(app_id, APPLICATION_ID);
        assert_eq!(version, SCHEMA_VERSION);
        assert_eq!(mode, "wal");
        assert_eq!(kind, "catalog");
    }

    #[test]
    fn reopening_is_a_no_op() {
        let (_dir, catalog) = catalog();
        Catalog::open(catalog.path()).unwrap();
        let backups = std::fs::read_dir(catalog.path().parent().unwrap())
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .ends_with(".bak")
            })
            .count();
        assert_eq!(backups, 0);
    }

    #[test]
    fn refuses_files_that_are_not_catalogs() {
        let dir = tempfile::tempdir().unwrap();
        let text = dir.path().join("notes.txt");
        std::fs::write(
            &text,
            "definitely not sqlite, but long enough to look like a header....",
        )
        .unwrap();
        assert!(matches!(
            Catalog::open(&text),
            Err(CatalogError::NotSymbolon(_))
        ));

        let other = dir.path().join("other.db");
        Connection::open(&other)
            .unwrap()
            .execute_batch("CREATE TABLE things (x)")
            .unwrap();
        assert!(matches!(
            Catalog::open(&other),
            Err(CatalogError::NotSymbolon(_))
        ));
    }

    #[test]
    fn refuses_catalogs_from_newer_versions() {
        let (_dir, catalog) = catalog();
        catalog
            .connect()
            .unwrap()
            .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
            .unwrap();
        assert!(matches!(
            Catalog::open(catalog.path()),
            Err(CatalogError::TooNew { found, supported }) if found == SCHEMA_VERSION + 1 && supported == SCHEMA_VERSION
        ));
    }

    #[test]
    fn locations_round_trip() {
        let (dir, catalog) = catalog();
        let added = catalog
            .add_location(
                NewLocation::local("photos", dir.path())
                    .hash_algo(HashAlgo::Xxh3)
                    .hash_policy(HashPolicy::Eager)
                    .ignore_patterns(["*.tmp", "cache/"]),
            )
            .unwrap();
        assert_eq!(added.label, "photos");
        assert_eq!(
            added.root,
            LocationRoot::Local(std::fs::canonicalize(dir.path()).unwrap())
        );
        assert_eq!(added.hash_algo, HashAlgo::Xxh3);
        assert_eq!(added.hash_policy, HashPolicy::Eager);
        assert_eq!(added.ignore_patterns, ["*.tmp", "cache/"]);
        assert_eq!(added.path_encoding, PathEncoding::native());
        assert_eq!(catalog.location("photos").unwrap(), added);

        catalog
            .add_location(NewLocation::local("archive", dir.path()))
            .unwrap();
        let labels: Vec<_> = catalog
            .locations()
            .unwrap()
            .into_iter()
            .map(|l| l.label)
            .collect();
        assert_eq!(labels, ["archive", "photos"]);

        catalog.remove_location("photos").unwrap();
        assert!(matches!(
            catalog.location("photos"),
            Err(CatalogError::LocationNotFound(_))
        ));
        assert!(matches!(
            catalog.remove_location("photos"),
            Err(CatalogError::LocationNotFound(_))
        ));
    }

    #[test]
    fn labels_are_unique_and_validated() {
        let (dir, catalog) = catalog();
        catalog
            .add_location(NewLocation::local("disk", dir.path()))
            .unwrap();
        assert!(matches!(
            catalog.add_location(NewLocation::local("disk", dir.path())),
            Err(CatalogError::LabelTaken(label)) if label == "disk"
        ));
        for bad in ["", " padded", "a:b", "a/b", "tab\tbed", &"x".repeat(65)] {
            assert!(
                matches!(
                    catalog.add_location(NewLocation::local(bad, dir.path())),
                    Err(CatalogError::InvalidLabel(_))
                ),
                "{bad:?}"
            );
        }
        catalog
            .add_location(NewLocation::local("telefon Ani 📱", dir.path()))
            .unwrap();
    }

    #[test]
    fn root_must_be_an_existing_directory() {
        let (dir, catalog) = catalog();
        let file = dir.path().join("file");
        std::fs::write(&file, b"x").unwrap();
        for root in [dir.path().join("missing"), file] {
            assert!(matches!(
                catalog.add_location(NewLocation::local("x", &root)),
                Err(CatalogError::InvalidRoot(_))
            ));
        }
    }

    #[test]
    fn removing_a_location_removes_its_scans_and_entries() {
        let (dir, catalog) = catalog();
        let location = catalog
            .add_location(NewLocation::local("disk", dir.path()))
            .unwrap();
        let conn = catalog.connect().unwrap();
        conn.execute(
            "INSERT INTO scans (location_id, started_at, status) VALUES (?1, 0, 'completed')",
            [location.id.get()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO entries (location_id, rel_path_raw, path_key, parent_key, kind, size, seen_scan_id) \
             VALUES (?1, x'61', 'a', '', 'file', 1, 1)",
            [location.id.get()],
        )
        .unwrap();
        catalog.remove_location("disk").unwrap();
        let left: i64 = conn
            .query_row(
                "SELECT (SELECT count(*) FROM scans) + (SELECT count(*) FROM entries)",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(left, 0);
    }

    #[test]
    fn roots_from_other_platforms_are_listed_not_rejected() {
        let (dir, catalog) = catalog();
        catalog
            .add_location(NewLocation::local("here", dir.path()))
            .unwrap();
        let (foreign, raw): (PathEncoding, &[u8]) = match PathEncoding::native() {
            PathEncoding::Posix => (PathEncoding::Wtf8, b"C:\\Users\\me"),
            PathEncoding::Wtf8 => (PathEncoding::Posix, b"/home/me/bad\xff"),
        };
        catalog
            .connect()
            .unwrap()
            .execute(
                "INSERT INTO locations (label, root_kind, root_raw, path_encoding, hash_algo, hash_policy, created_at) \
                 VALUES ('there', 'local', ?1, ?2, 'blake3', 'lazy', 0)",
                params![raw, foreign.name()],
            )
            .unwrap();
        let there = catalog.location("there").unwrap();
        assert!(
            matches!(there.root, LocationRoot::Foreign { encoding, .. } if encoding == foreign)
        );
        assert_eq!(catalog.locations().unwrap().len(), 2);
    }

    #[test]
    fn timestamps_round_trip_around_the_epoch() {
        for ms in [0, 1, -1, 1_795_000_000_000, -86_400_000] {
            assert_eq!(system_time_to_ms(ms_to_system_time(ms)), ms);
        }
    }
}
