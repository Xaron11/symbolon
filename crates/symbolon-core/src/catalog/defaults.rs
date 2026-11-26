//! Where clients keep the catalog and find the Android agent, shared so the CLI and the app use
//! the same files. Reading the environment and the executable's location stays in the clients.

use std::path::{Path, PathBuf};

/// File name of the agent pushed to phones (ADR-0017), as release archives ship it next to the
/// client's executable.
pub const AGENT_FILE_NAME: &str = "symbolon-agent-aarch64-linux-musl";

/// The default catalog under the platform's local data directory `local_data_dir`.
pub fn default_catalog_path(local_data_dir: &Path) -> PathBuf {
    local_data_dir.join("symbolon").join("catalog.db")
}

/// Where to look for the agent, in order, for a client whose executable is in `exe_dir`: next
/// to it, then in `../libexec/symbolon/` (Nix).
pub fn agent_candidates(exe_dir: &Path) -> [PathBuf; 2] {
    [
        exe_dir.join(AGENT_FILE_NAME),
        exe_dir.join("../libexec/symbolon").join(AGENT_FILE_NAME),
    ]
}

/// Creates the directories holding the catalog at `path`. They hold a catalog of private paths,
/// so on Unix only the owner can enter them.
///
/// # Errors
///
/// An I/O error if a directory cannot be created.
pub fn create_catalog_dir(path: &Path) -> std::io::Result<()> {
    let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) else {
        return Ok(());
    };
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(parent)
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn default_catalog_path_is_under_symbolon() {
        assert_eq!(
            default_catalog_path(Path::new("/data")),
            Path::new("/data/symbolon/catalog.db")
        );
    }

    #[test]
    fn agent_candidates_order() {
        assert_eq!(
            agent_candidates(Path::new("/opt/bin")),
            [
                PathBuf::from("/opt/bin/symbolon-agent-aarch64-linux-musl"),
                PathBuf::from("/opt/bin/../libexec/symbolon/symbolon-agent-aarch64-linux-musl"),
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn create_catalog_dir_is_owner_only() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let catalog = dir.path().join("a/b/catalog.db");
        create_catalog_dir(&catalog).unwrap();
        let mode = std::fs::metadata(dir.path().join("a/b"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o700);
        // A bare file name has no directory to create.
        create_catalog_dir(Path::new("catalog.db")).unwrap();
    }
}
