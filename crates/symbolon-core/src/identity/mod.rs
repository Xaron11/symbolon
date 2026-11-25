//! Device fingerprints: recognizing a location's device wherever it is mounted (ADR-0016).
//!
//! A location's label is its identity; a [`Fingerprint`] is a best-effort hint naming a
//! directory on a specific device, so a pendrive plugged in at another mount point can be found
//! again, and a different device at the old mount point is not mistaken for it.
//!
//! [`Volumes`] lists what is mounted. [`SystemVolumes`] reads the running system (Linux, with
//! the `identity` feature); tests and other platforms can supply their own.

use std::fmt;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;

use crate::pathkey;

#[cfg(all(feature = "identity", target_os = "linux"))]
mod linux;

/// A best-effort identity of a directory on a device, compared only for equality.
///
/// Local directories use `fs:<filesystem UUID>:<path inside that filesystem>`; the path is
/// absolute within the filesystem, so it does not depend on where the filesystem is mounted.
/// Bytes that are not valid UTF-8, and `%`, are written as `%XX`.
///
/// # Examples
///
/// ```
/// use symbolon_core::identity::Fingerprint;
///
/// let fingerprint = Fingerprint::filesystem("5ADB-A50A", b"/DCIM/100%");
/// assert_eq!(fingerprint.as_str(), "fs:5ADB-A50A:/DCIM/100%25");
/// assert_eq!(fingerprint.as_str().parse::<Fingerprint>(), Ok(fingerprint));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Fingerprint(String);

impl Fingerprint {
    /// The fingerprint of `path` (absolute, `/`-separated bytes) inside the filesystem `uuid`.
    pub fn filesystem(uuid: &str, path: &[u8]) -> Self {
        let mut text = format!("fs:{uuid}:");
        if path.first() != Some(&b'/') {
            text.push('/');
        }
        for chunk in path.utf8_chunks() {
            for c in chunk.valid().chars() {
                if c == '%' {
                    text.push_str("%25");
                } else {
                    text.push(c);
                }
            }
            for &byte in chunk.invalid() {
                pathkey::push_escaped_byte(&mut text, byte);
            }
        }
        Self(text)
    }

    /// The stored text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// For a filesystem fingerprint: its UUID and the raw path inside it.
    pub fn filesystem_parts(&self) -> Option<(&str, Vec<u8>)> {
        let rest = self.0.strip_prefix("fs:")?;
        let (uuid, path) = rest.split_once(':')?;
        Some((uuid, unescape(path)?))
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Text that is not a fingerprint: no `<kind>:` prefix, or a malformed `fs:` fingerprint.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("not a device fingerprint: {0:?}")]
pub struct InvalidFingerprint(String);

impl FromStr for Fingerprint {
    type Err = InvalidFingerprint;

    /// Parses stored text. Only `fs:` fingerprints are interpreted; other kinds (`adb:` from
    /// M5) are kept as they are.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = || InvalidFingerprint(s.to_owned());
        let (kind, rest) = s.split_once(':').ok_or_else(invalid)?;
        if kind.is_empty() || rest.is_empty() {
            return Err(invalid());
        }
        let fingerprint = Self(s.to_owned());
        if kind == "fs" {
            let (uuid, path) = fingerprint.filesystem_parts().ok_or_else(invalid)?;
            if uuid.is_empty() || path.first() != Some(&b'/') {
                return Err(invalid());
            }
        }
        Ok(fingerprint)
    }
}

/// Reverses the `%XX` escapes of [`Fingerprint::filesystem`].
fn unescape(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len());
    let mut bytes = text.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let hex = [bytes.next()?, bytes.next()?];
            out.push(u8::from_str_radix(std::str::from_utf8(&hex).ok()?, 16).ok()?);
        } else {
            out.push(byte);
        }
    }
    Some(out)
}

/// One mounted filesystem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mount {
    /// The filesystem's UUID, if it has one (block devices do; tmpfs and network shares not).
    pub uuid: Option<String>,
    /// Which directory of the filesystem is mounted: `/` usually, another path for bind
    /// mounts and subvolumes. Raw bytes, `/`-separated.
    pub root: Vec<u8>,
    /// Where it is mounted.
    pub mount_point: PathBuf,
}

/// Lists the mounted filesystems.
pub trait Volumes: Send + Sync {
    /// Every mount, in mount order (a later mount at the same point hides an earlier one).
    ///
    /// # Errors
    ///
    /// The mount table cannot be read.
    fn mounts(&self) -> io::Result<Vec<Mount>>;
}

/// The running system's mounts: `/proc/self/mountinfo` and `/dev/disk/by-uuid` on Linux with
/// the `identity` feature; nothing elsewhere, so no fingerprints are detected there yet.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemVolumes;

impl Volumes for SystemVolumes {
    fn mounts(&self) -> io::Result<Vec<Mount>> {
        #[cfg(all(feature = "identity", target_os = "linux"))]
        return linux::mounts();
        #[cfg(not(all(feature = "identity", target_os = "linux")))]
        Ok(Vec::new())
    }
}

/// The fingerprint of the directory `path` (absolute and canonical), if its filesystem has a
/// UUID.
///
/// # Errors
///
/// The mount table cannot be read.
pub fn fingerprint(path: &Path, volumes: &dyn Volumes) -> io::Result<Option<Fingerprint>> {
    let mounts = volumes.mounts()?;
    let mut best: Option<&Mount> = None;
    for mount in &mounts {
        if path.starts_with(&mount.mount_point)
            && best.is_none_or(|b| {
                mount.mount_point.components().count() >= b.mount_point.components().count()
            })
        {
            best = Some(mount);
        }
    }
    let Some(mount) = best else {
        return Ok(None);
    };
    let Some(uuid) = &mount.uuid else {
        return Ok(None);
    };
    let mut inner = mount.root.clone();
    for component in path
        .components()
        .skip(mount.mount_point.components().count())
    {
        if let Component::Normal(name) = component {
            if inner.last() != Some(&b'/') {
                inner.push(b'/');
            }
            pathkey::push_native(&mut inner, name);
        }
    }
    Ok(Some(Fingerprint::filesystem(uuid, &inner)))
}

/// The existing directories that `fingerprint` names right now, one per distinct directory,
/// sorted. A directory reachable through several mounts of its filesystem (a subdirectory
/// mounted elsewhere too) is listed once, under the most specific mount, as [`fingerprint`]
/// would name it.
///
/// # Errors
///
/// The mount table cannot be read.
pub fn candidates(fingerprint: &Fingerprint, volumes: &dyn Volumes) -> io::Result<Vec<PathBuf>> {
    let Some((uuid, inner)) = fingerprint.filesystem_parts() else {
        return Ok(Vec::new());
    };
    let mut found: Vec<Candidate> = Vec::new();
    for mount in volumes.mounts()? {
        if mount.uuid.as_deref() != Some(uuid) {
            continue;
        }
        let Some(rest) = beneath(&inner, &mount.root) else {
            continue;
        };
        let candidate = if rest.is_empty() {
            mount.mount_point.clone()
        } else {
            match pathkey::native_from_bytes(rest) {
                Some(rel) => mount.mount_point.join(rel),
                None => continue,
            }
        };
        let Ok(meta) = std::fs::metadata(&candidate) else {
            continue;
        };
        if !meta.is_dir() {
            continue;
        }
        let depth = mount.root.len();
        let id = file_id(&meta);
        let new = Candidate {
            path: candidate,
            depth,
            id,
        };
        match found
            .iter_mut()
            .find(|c| c.path == new.path || (id.is_some() && c.id == id))
        {
            Some(existing) if depth > existing.depth => *existing = new,
            Some(_) => {}
            None => found.push(new),
        }
    }
    let mut paths: Vec<PathBuf> = found.into_iter().map(|c| c.path).collect();
    paths.sort();
    Ok(paths)
}

/// A directory named by a fingerprint, found through one mount.
struct Candidate {
    path: PathBuf,
    /// Length of the mounted directory's path inside the filesystem: the most specific mount
    /// names a directory reachable through several.
    depth: usize,
    /// Device and inode, to recognize one directory reached through several mounts.
    id: Option<(u64, u64)>,
}

#[cfg(unix)]
#[expect(
    clippy::unnecessary_wraps,
    reason = "no stable identity elsewhere; paths are compared instead"
)]
fn file_id(meta: &std::fs::Metadata) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt as _;
    Some((meta.dev(), meta.ino()))
}

#[cfg(not(unix))]
fn file_id(_meta: &std::fs::Metadata) -> Option<(u64, u64)> {
    None
}

/// `path` relative to `root` (both absolute, `/`-separated), if it is `root` or below it.
fn beneath<'a>(path: &'a [u8], root: &[u8]) -> Option<&'a [u8]> {
    let root = root.strip_suffix(b"/").unwrap_or(root);
    let rest = path.strip_prefix(root)?;
    if rest.is_empty() {
        Some(rest)
    } else {
        rest.strip_prefix(b"/")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake(Vec<Mount>);

    impl Volumes for Fake {
        fn mounts(&self) -> io::Result<Vec<Mount>> {
            Ok(self.0.clone())
        }
    }

    fn mount(uuid: Option<&str>, root: &str, at: &Path) -> Mount {
        Mount {
            uuid: uuid.map(str::to_owned),
            root: root.as_bytes().to_vec(),
            mount_point: at.to_path_buf(),
        }
    }

    #[test]
    fn fingerprints_round_trip_and_escape() {
        let fp = Fingerprint::filesystem("abc", b"/a b/%/\xff");
        assert_eq!(fp.as_str(), "fs:abc:/a b/%25/%FF");
        assert_eq!(fp.as_str().parse::<Fingerprint>().unwrap(), fp);
        assert_eq!(
            fp.filesystem_parts(),
            Some(("abc", b"/a b/%/\xff".to_vec()))
        );
        assert_eq!(Fingerprint::filesystem("u", b"").as_str(), "fs:u:/");
        let android: Fingerprint = "adb:R58M:/sdcard/DCIM".parse().unwrap();
        assert_eq!(android.filesystem_parts(), None);
        for bad in [
            "", "fs", "fs:", "fs:u", "fs::/x", "fs:u:x", "fs:u:/%Z", ":x",
        ] {
            assert!(bad.parse::<Fingerprint>().is_err(), "{bad:?}");
        }
    }

    #[test]
    fn the_innermost_mount_names_the_device() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let volumes = Fake(vec![
            mount(Some("disk"), "/", Path::new("/")),
            mount(Some("usb"), "/", &root.join("media/usb")),
            mount(None, "/", &root.join("media/usb/tmp")),
        ]);
        let fp = |p: &Path| fingerprint(p, &volumes).unwrap().map(|f| f.0);
        assert_eq!(
            fp(&root.join("media/usb/DCIM")).as_deref(),
            Some("fs:usb:/DCIM")
        );
        assert_eq!(fp(&root.join("media/usb")).as_deref(), Some("fs:usb:/"));
        assert_eq!(fp(&root.join("media/usb/tmp/x")), None, "no UUID");
        assert!(fp(&root.join("media")).unwrap().starts_with("fs:disk:/"));
    }

    #[test]
    fn a_later_mount_at_the_same_point_wins() {
        let at = Path::new("/mnt");
        let volumes = Fake(vec![
            mount(Some("old"), "/", at),
            mount(Some("new"), "/", at),
        ]);
        let fp = fingerprint(&at.join("x"), &volumes).unwrap().unwrap();
        assert_eq!(fp.as_str(), "fs:new:/x");
    }

    #[test]
    fn subdirectory_mounts_give_the_path_inside_the_filesystem() {
        let volumes = Fake(vec![mount(Some("disk"), "/@home", Path::new("/home"))]);
        let fp = fingerprint(Path::new("/home/me/photos"), &volumes)
            .unwrap()
            .unwrap();
        assert_eq!(fp.as_str(), "fs:disk:/@home/me/photos");
    }

    #[test]
    fn candidates_follow_the_device_to_every_mount_point() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for path in ["a/DCIM", "b/sub/DCIM", "c"] {
            std::fs::create_dir_all(root.join(path)).unwrap();
        }
        let fp = Fingerprint::filesystem("usb", b"/DCIM");
        let volumes = Fake(vec![
            mount(Some("usb"), "/", &root.join("a")),
            mount(Some("other"), "/", &root.join("x")),
            mount(Some("usb"), "/", &root.join("c")),
        ]);
        assert_eq!(candidates(&fp, &volumes).unwrap(), [root.join("a/DCIM")]);

        // Two separate copies (two mounts of the same filesystem UUID) are both candidates.
        let volumes = Fake(vec![
            mount(Some("usb"), "/", &root.join("a")),
            mount(Some("usb"), "/", &root.join("a")),
            mount(Some("usb"), "/DCIM", &root.join("b/sub/DCIM")),
            mount(Some("usb"), "/Music", &root.join("c")),
        ]);
        assert_eq!(
            candidates(&fp, &volumes).unwrap(),
            [root.join("a/DCIM"), root.join("b/sub/DCIM")]
        );
        assert_eq!(
            candidates(&"adb:s:/x".parse().unwrap(), &volumes).unwrap(),
            Vec::<PathBuf>::new()
        );
    }

    /// A subdirectory of a mounted filesystem mounted a second time is one directory, not an
    /// ambiguity: the more specific mount names it.
    #[cfg(unix)]
    #[test]
    fn one_directory_reachable_twice_is_one_candidate() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("home/opt/tools")).unwrap();
        std::os::unix::fs::symlink(root.join("home/opt"), root.join("opt")).unwrap();
        let fp = Fingerprint::filesystem("disk", b"/opt/tools");
        let volumes = Fake(vec![
            mount(Some("disk"), "/", &root.join("home")),
            mount(Some("disk"), "/opt", &root.join("opt")),
        ]);
        assert_eq!(candidates(&fp, &volumes).unwrap(), [root.join("opt/tools")]);
    }

    #[test]
    fn beneath_matches_whole_components() {
        assert_eq!(beneath(b"/DCIM/a", b"/"), Some(&b"DCIM/a"[..]));
        assert_eq!(beneath(b"/DCIM/a", b"/DCIM"), Some(&b"a"[..]));
        assert_eq!(beneath(b"/DCIM", b"/DCIM"), Some(&b""[..]));
        assert_eq!(beneath(b"/DCIMX", b"/DCIM"), None);
        assert_eq!(beneath(b"/a", b"/b"), None);
    }
}
