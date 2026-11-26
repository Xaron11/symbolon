//! What the bridge keeps between commands.

use std::path::PathBuf;
use std::sync::Arc;

use symbolon_core::catalog::{self, Catalog};
use symbolon_core::source::AdbOptions;
use symbolon_core::transport::android::{AndroidTransport, SystemAdb};

use crate::error::{ApiError, ErrorKind};

/// The open catalog and the work running on it; managed by Tauri as `Arc<AppState>`.
pub(crate) struct AppState {
    pub(crate) catalog: Catalog,
    pub(crate) transport: Arc<dyn AndroidTransport>,
}

impl AppState {
    /// State over `catalog`, reaching phones through `transport`.
    pub(crate) fn new(catalog: Catalog, transport: Arc<dyn AndroidTransport>) -> Self {
        Self { catalog, transport }
    }

    /// Opens the catalog the CLI uses: `SYMBOLON_CATALOG`, else the default path; phones are
    /// read with the agent from `SYMBOLON_AGENT` or next to this executable (ADR-0017).
    pub(crate) fn open_default() -> Result<Self, ApiError> {
        let path = match std::env::var_os("SYMBOLON_CATALOG") {
            Some(path) => PathBuf::from(path),
            None => catalog::default_catalog_path(&dirs::data_local_dir().ok_or_else(|| {
                ApiError::new(ErrorKind::Io, "no local data directory on this system")
            })?),
        };
        catalog::create_catalog_dir(&path).map_err(|e| {
            ApiError::new(
                ErrorKind::Io,
                format!("cannot create the catalog directory: {e}"),
            )
        })?;
        let transport: Arc<dyn AndroidTransport> = Arc::new(SystemAdb::new());
        let catalog = Catalog::open(&path)?.with_android(
            Arc::clone(&transport),
            find_agent(),
            AdbOptions::default(),
        );
        Ok(Self::new(catalog, transport))
    }
}

fn find_agent() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("SYMBOLON_AGENT") {
        return Some(PathBuf::from(path));
    }
    let exe = std::env::current_exe().ok()?;
    catalog::agent_candidates(exe.parent()?)
        .into_iter()
        .find(|path| path.is_file())
}

#[cfg(test)]
pub(crate) mod testing {
    //! A state over a temporary catalog and a fake adb.

    use std::path::Path;
    use std::sync::{Arc, Mutex, PoisonError};

    use symbolon_core::catalog::Catalog;
    use symbolon_core::source::AdbOptions;
    use symbolon_core::transport::android::{
        AdbError, AndroidTransport, Device, ExecStream, ShellOutput, parse_devices,
    };

    use super::AppState;

    /// The devices a [`FakeAdb`] reports; tests replace them to plug and unplug phones.
    pub(crate) type Plugged = Arc<Mutex<Vec<Device>>>;

    /// Reports its devices and fails everything else.
    pub(crate) struct FakeAdb(pub(crate) Plugged);

    impl AndroidTransport for FakeAdb {
        fn devices(&self) -> Result<Vec<Device>, AdbError> {
            Ok(self
                .0
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone())
        }
        fn push(&self, _: &str, _: &Path, _: &str) -> Result<(), AdbError> {
            Err(AdbError::Io(std::io::Error::other("fake adb")))
        }
        fn exec(&self, _: &str, _: &[String]) -> Result<Box<dyn ExecStream>, AdbError> {
            Err(AdbError::Io(std::io::Error::other("fake adb")))
        }
        fn shell(&self, _: &str, _: &str) -> Result<ShellOutput, AdbError> {
            Err(AdbError::Io(std::io::Error::other("fake adb")))
        }
    }

    /// Devices as `adb devices -l` prints them, one `serial state` pair per line.
    pub(crate) fn devices(lines: &str) -> Vec<Device> {
        parse_devices(lines)
    }

    /// A state over a fresh catalog in `dir`.
    pub(crate) fn state(dir: &Path, devices: Vec<Device>) -> AppState {
        plugged(dir, devices).0
    }

    /// Like [`state`], with the handle to change which devices are attached.
    pub(crate) fn plugged(dir: &Path, devices: Vec<Device>) -> (AppState, Plugged) {
        let plugged = Arc::new(Mutex::new(devices));
        let transport: Arc<dyn AndroidTransport> = Arc::new(FakeAdb(Arc::clone(&plugged)));
        let catalog = Catalog::open(dir.join("catalog.db")).unwrap().with_android(
            Arc::clone(&transport),
            None,
            AdbOptions::default(),
        );
        (AppState::new(catalog, transport), plugged)
    }
}
