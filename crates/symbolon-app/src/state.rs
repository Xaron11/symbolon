//! What the bridge keeps between commands.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use symbolon_core::catalog::{self, Catalog};
use symbolon_core::source::AdbOptions;
use symbolon_core::transport::android::{AndroidTransport, SystemAdb};

use crate::compare::CompareRegistry;
use crate::error::{ApiError, ErrorKind};
use crate::scans::ScanRegistry;

/// The open catalog and the work running on it; managed by Tauri as `Arc<AppState>`.
pub(crate) struct AppState {
    pub(crate) catalog: Catalog,
    pub(crate) transport: Arc<dyn AndroidTransport>,
    pub(crate) scans: Arc<ScanRegistry>,
    pub(crate) comparisons: Arc<CompareRegistry>,
}

impl AppState {
    /// State over `catalog`, reaching phones through `transport`.
    pub(crate) fn new(catalog: Catalog, transport: Arc<dyn AndroidTransport>) -> Self {
        Self {
            catalog,
            transport,
            scans: Arc::default(),
            comparisons: Arc::default(),
        }
    }

    /// Another state over the same catalog, transport and scans.
    #[cfg(test)]
    pub(crate) fn clone_for_test(&self) -> Self {
        Self {
            catalog: self.catalog.clone(),
            transport: Arc::clone(&self.transport),
            scans: Arc::clone(&self.scans),
            comparisons: Arc::clone(&self.comparisons),
        }
    }

    /// Opens the catalog the CLI uses: `SYMBOLON_CATALOG`, else the default path; phones are
    /// read with the agent from `SYMBOLON_AGENT`, next to this executable (ADR-0017), or in the
    /// installed app's `resources` (ADR-0026).
    pub(crate) fn open_default(resources: Option<&Path>) -> Result<Self, ApiError> {
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
            locate_agent(
                std::env::var_os("SYMBOLON_AGENT").map(PathBuf::from),
                std::env::current_exe()
                    .ok()
                    .as_deref()
                    .and_then(Path::parent),
                resources,
            ),
            AdbOptions::default(),
        );
        Ok(Self::new(catalog, transport))
    }
}

/// The agent to push to phones: `chosen` (from `SYMBOLON_AGENT`) if set, else the first that
/// exists of the places next to the executable in `exe_dir` and the bundle's `resources`, where
/// the packaged app keeps it (`/usr/lib/Symbolon` in the Linux packages).
fn locate_agent(
    chosen: Option<PathBuf>,
    exe_dir: Option<&Path>,
    resources: Option<&Path>,
) -> Option<PathBuf> {
    if chosen.is_some() {
        return chosen;
    }
    exe_dir
        .map(catalog::agent_candidates)
        .into_iter()
        .flatten()
        .chain(resources.map(|dir| dir.join(catalog::AGENT_FILE_NAME)))
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

#[cfg(test)]
mod tests {
    use std::fs;

    use symbolon_core::catalog::AGENT_FILE_NAME;

    use super::*;

    #[test]
    fn agent_is_found_next_to_the_app_then_in_its_resources() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("bin");
        let resources = dir.path().join("lib/Symbolon");
        fs::create_dir_all(&exe).unwrap();
        fs::create_dir_all(&resources).unwrap();
        assert_eq!(locate_agent(None, Some(&exe), Some(&resources)), None);

        fs::write(resources.join(AGENT_FILE_NAME), "agent").unwrap();
        assert_eq!(
            locate_agent(None, Some(&exe), Some(&resources)),
            Some(resources.join(AGENT_FILE_NAME))
        );

        fs::write(exe.join(AGENT_FILE_NAME), "agent").unwrap();
        assert_eq!(
            locate_agent(None, Some(&exe), Some(&resources)),
            Some(exe.join(AGENT_FILE_NAME))
        );

        let chosen = PathBuf::from("/elsewhere/agent");
        assert_eq!(
            locate_agent(Some(chosen.clone()), Some(&exe), Some(&resources)),
            Some(chosen)
        );
    }
}
