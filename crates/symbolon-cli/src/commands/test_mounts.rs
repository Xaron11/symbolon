//! Debug builds only: `SYMBOLON_TEST_MOUNTS` names a file that replaces the system's mount
//! table, so tests can plug simulated devices in and out (ADR-0016). Release builds never
//! read it.
//!
//! One mount per line: `<filesystem UUID or ->` and the mount point, separated by a tab.

use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use symbolon_core::catalog::Catalog;
use symbolon_core::identity::{Mount, Volumes};

pub(super) fn apply(catalog: Catalog) -> Catalog {
    match std::env::var_os("SYMBOLON_TEST_MOUNTS") {
        Some(path) => catalog.with_volumes(Arc::new(TestMounts(PathBuf::from(path)))),
        None => catalog,
    }
}

struct TestMounts(PathBuf);

impl Volumes for TestMounts {
    fn mounts(&self) -> io::Result<Vec<Mount>> {
        let text = fs::read_to_string(&self.0)?;
        Ok(text
            .lines()
            .filter_map(|line| line.split_once('\t'))
            .map(|(uuid, at)| Mount {
                uuid: (uuid != "-").then(|| uuid.to_owned()),
                root: b"/".to_vec(),
                // Stored roots are canonical (`\\?\` on Windows), so mount points must be too.
                mount_point: fs::canonicalize(at).unwrap_or_else(|_| PathBuf::from(at)),
            })
            .collect())
    }
}
