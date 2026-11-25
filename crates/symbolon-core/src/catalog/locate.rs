//! Where a location's device is now (ADR-0016).

use std::path::{Path, PathBuf};

use rusqlite::params;

use super::{Catalog, CatalogError, Location, LocationRoot};
use crate::identity::{self, Fingerprint};
use crate::pathkey;

/// Where a saved location's files are now, from its recorded fingerprint and the mounted
/// filesystems; see [`Catalog::locate`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Whereabouts {
    /// At the stored root: its fingerprint matches, or none is recorded and the root exists.
    Here(PathBuf),
    /// Not at the stored root, but its device is mounted at exactly one other place.
    Moved {
        /// The stored root.
        from: PathBuf,
        /// Where the fingerprinted directory is now.
        to: PathBuf,
    },
    /// Not at the stored root, and its device matches several places.
    Ambiguous {
        /// The stored root.
        root: PathBuf,
        /// Every matching directory, sorted.
        candidates: Vec<PathBuf>,
    },
    /// The stored root exists but belongs to another device (or one without a fingerprint),
    /// and the location's own device is not mounted: for example the empty mount point of an
    /// unplugged pendrive.
    OtherDevice {
        /// The stored root.
        root: PathBuf,
        /// The recorded fingerprint.
        expected: Fingerprint,
        /// The fingerprint of what is at the root now, if it has one.
        found: Option<Fingerprint>,
    },
    /// Neither the stored root nor the device can be found.
    Missing(PathBuf),
    /// An imported or foreign location: its files are on another computer.
    NotLocal,
}

impl Catalog {
    /// Where the location's files are now. Locations without a recorded fingerprint are
    /// [`Whereabouts::Here`] whenever their root is a directory, as before M4.
    ///
    /// # Errors
    ///
    /// [`CatalogError::LocationNotFound`], a database error, or [`CatalogError::Io`] when the
    /// mount table cannot be read.
    pub fn locate(&self, label: &str) -> Result<Whereabouts, CatalogError> {
        let location = self.location(label)?;
        self.locate_location(&location)
    }

    pub(crate) fn locate_location(&self, location: &Location) -> Result<Whereabouts, CatalogError> {
        let LocationRoot::Local(root) = &location.root else {
            return Ok(Whereabouts::NotLocal);
        };
        let is_dir = root.is_dir();
        let Some(expected) = location
            .fingerprint
            .as_deref()
            .and_then(|text| text.parse::<Fingerprint>().ok())
        else {
            return Ok(if is_dir {
                Whereabouts::Here(root.clone())
            } else {
                Whereabouts::Missing(root.clone())
            });
        };
        let found = if is_dir {
            self.fingerprint_of(root)?
        } else {
            None
        };
        if found.as_ref() == Some(&expected) {
            return Ok(Whereabouts::Here(root.clone()));
        }
        let mut candidates = identity::candidates(&expected, &*self.volumes).map_err(io_error)?;
        candidates.retain(|candidate| candidate != root);
        Ok(match (candidates.len(), is_dir) {
            (1, _) => Whereabouts::Moved {
                from: root.clone(),
                to: candidates.remove(0),
            },
            (2.., _) => Whereabouts::Ambiguous {
                root: root.clone(),
                candidates,
            },
            (_, true) => Whereabouts::OtherDevice {
                root: root.clone(),
                expected,
                found,
            },
            (_, false) => Whereabouts::Missing(root.clone()),
        })
    }

    /// Moves the location to `root` (an existing directory) and records the fingerprint of
    /// what is there. Also used to accept another device at the same root.
    ///
    /// # Errors
    ///
    /// [`CatalogError::LocationNotFound`], [`CatalogError::InvalidRoot`] if `root` is not a
    /// directory or the location is not local, or a database or mount-table error.
    pub fn relocate(&self, label: &str, root: &Path) -> Result<Location, CatalogError> {
        let location = self.location(label)?;
        let LocationRoot::Local(_) = location.root else {
            return Err(CatalogError::InvalidRoot(root.to_path_buf()));
        };
        let root = std::fs::canonicalize(root)
            .ok()
            .filter(|p| p.is_dir())
            .ok_or_else(|| CatalogError::InvalidRoot(root.to_path_buf()))?;
        let fingerprint = self.fingerprint_of(&root)?;
        let mut root_raw = Vec::new();
        pathkey::push_native(&mut root_raw, root.as_os_str());
        self.connect()?.execute(
            "UPDATE locations SET root_raw = ?2, fingerprint = ?3 WHERE id = ?1",
            params![
                location.id.get(),
                root_raw,
                fingerprint.as_ref().map(Fingerprint::as_str)
            ],
        )?;
        self.location(label)
    }

    /// Records the root's fingerprint for a location that has none yet; a recorded
    /// fingerprint is never replaced here.
    pub(crate) fn record_missing_fingerprint(
        &self,
        location: &Location,
    ) -> Result<(), CatalogError> {
        let LocationRoot::Local(root) = &location.root else {
            return Ok(());
        };
        if location.fingerprint.is_some() {
            return Ok(());
        }
        if let Some(fingerprint) = self.fingerprint_of(root)? {
            self.connect()?.execute(
                "UPDATE locations SET fingerprint = ?2 WHERE id = ?1 AND fingerprint IS NULL",
                params![location.id.get(), fingerprint.as_str()],
            )?;
        }
        Ok(())
    }

    pub(crate) fn fingerprint_of(&self, root: &Path) -> Result<Option<Fingerprint>, CatalogError> {
        identity::fingerprint(root, &*self.volumes).map_err(io_error)
    }
}

fn io_error(source: std::io::Error) -> CatalogError {
    CatalogError::Io {
        context: "cannot read the mounted filesystems".to_owned(),
        source,
    }
}
