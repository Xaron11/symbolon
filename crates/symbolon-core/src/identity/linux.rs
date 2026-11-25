//! Linux mounts: `/proc/self/mountinfo` for what is mounted where, `/dev/disk/by-uuid` for the
//! filesystem UUIDs of block devices.

use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::os::unix::ffi::OsStringExt as _;
use std::os::unix::fs::{FileTypeExt as _, MetadataExt as _};
use std::path::{Path, PathBuf};

use super::Mount;

pub(super) fn mounts() -> io::Result<Vec<Mount>> {
    let text = fs::read("/proc/self/mountinfo")?;
    let uuids = uuids_by_device(Path::new("/dev/disk/by-uuid"));
    Ok(parse(&text)
        .into_iter()
        .map(|entry| Mount {
            uuid: uuids.get(&device_of(&entry)).cloned(),
            root: entry.root,
            mount_point: entry.mount_point,
        })
        .collect())
}

/// One line of `mountinfo`, as far as it matters here.
#[derive(Debug, PartialEq, Eq)]
struct Entry {
    /// `st_dev` of files on the mount (anonymous for btrfs subvolumes).
    dev: u64,
    root: Vec<u8>,
    mount_point: PathBuf,
    source: Vec<u8>,
}

/// The block device behind a mount: its source when that is a device node (btrfs reports
/// anonymous device numbers), else its device number.
fn device_of(entry: &Entry) -> u64 {
    if entry.source.first() == Some(&b'/') {
        let source = PathBuf::from(OsString::from_vec(entry.source.clone()));
        if let Ok(meta) = fs::metadata(source)
            && meta.file_type().is_block_device()
        {
            return meta.rdev();
        }
    }
    entry.dev
}

/// Device number to filesystem UUID, from the links udev maintains. Unreadable entries are
/// skipped: a missing directory just means no fingerprints.
fn uuids_by_device(dir: &Path) -> HashMap<u64, String> {
    let mut uuids = HashMap::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return uuids;
    };
    for entry in entries.flatten() {
        let Ok(uuid) = entry.file_name().into_string() else {
            continue;
        };
        if let Ok(meta) = fs::metadata(entry.path())
            && meta.file_type().is_block_device()
        {
            uuids.insert(meta.rdev(), uuid);
        }
    }
    uuids
}

/// Parses `mountinfo` (see `proc_pid_mountinfo(5)`); malformed lines are skipped.
fn parse(text: &[u8]) -> Vec<Entry> {
    text.split(|&b| b == b'\n')
        .filter_map(|line| {
            let fields: Vec<&[u8]> = line.split(|&b| b == b' ').collect();
            let separator = fields.iter().position(|f| *f == b"-")?;
            let (major, minor) = std::str::from_utf8(fields.get(2)?).ok()?.split_once(':')?;
            Some(Entry {
                dev: makedev(major.parse().ok()?, minor.parse().ok()?),
                root: unescape(fields.get(3)?),
                mount_point: PathBuf::from(OsString::from_vec(unescape(fields.get(4)?))),
                source: unescape(fields.get(separator + 2)?),
            })
        })
        .collect()
}

/// glibc's `makedev`.
fn makedev(major: u64, minor: u64) -> u64 {
    ((major & 0xffff_f000) << 32)
        | ((major & 0x0000_0fff) << 8)
        | ((minor & 0xffff_ff00) << 12)
        | (minor & 0x0000_00ff)
}

/// Undoes the kernel's octal escapes (`\040` for a space, `\011`, `\012`, `\134`).
fn unescape(field: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(field.len());
    let mut i = 0;
    while let Some(&byte) = field.get(i) {
        let octal = field
            .get(i + 1..i + 4)
            .filter(|digits| byte == b'\\' && digits.iter().all(|d| (b'0'..=b'7').contains(d)))
            .and_then(|digits| u8::from_str_radix(std::str::from_utf8(digits).ok()?, 8).ok());
        if let Some(value) = octal {
            out.push(value);
            i += 4;
        } else {
            out.push(byte);
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &[u8] = b"\
33 2 259:2 / / rw,relatime shared:1 - ext4 /dev/nvme0n1p2 rw
59 33 0:48 / /tmp rw,nosuid,nodev shared:126 - tmpfs tmpfs rw,size=8105760k
55 33 8:1 /home /home rw,relatime shared:122 - ext4 /dev/sda1 rw
56 33 0:52 /@photos /mnt/My\\040Disk rw - btrfs /dev/sdb1 rw,subvol=/@photos
57 33 8:17 / /run/media/me/USB rw,nosuid master:1 - vfat /dev/sdc1 rw
broken line
";

    #[test]
    fn mountinfo_lines_are_parsed_with_escapes() {
        let entries = parse(SAMPLE);
        assert_eq!(entries.len(), 5);
        assert_eq!(
            entries[0],
            Entry {
                dev: makedev(259, 2),
                root: b"/".to_vec(),
                mount_point: PathBuf::from("/"),
                source: b"/dev/nvme0n1p2".to_vec(),
            }
        );
        assert_eq!(entries[2].root, b"/home");
        assert_eq!(entries[3].mount_point, PathBuf::from("/mnt/My Disk"));
        assert_eq!(entries[3].root, b"/@photos");
        assert_eq!(entries[3].source, b"/dev/sdb1");
        assert_eq!(entries[1].source, b"tmpfs");
    }

    #[test]
    fn octal_escapes_only_when_complete() {
        assert_eq!(unescape(br"a\040b\134c"), b"a b\\c");
        assert_eq!(unescape(br"end\04"), br"end\04");
        assert_eq!(unescape(br"\999"), br"\999");
    }

    #[test]
    fn device_numbers_match_glibc() {
        assert_eq!(makedev(8, 1), 0x801);
        assert_eq!(makedev(259, 2), 0x10302);
        assert_eq!(makedev(4096, 300), 0x1000_0010_002c);
    }

    #[test]
    fn the_running_system_lists_its_root() {
        let mounts = mounts().unwrap();
        assert!(
            mounts.iter().any(|m| m.mount_point == Path::new("/")),
            "{mounts:?}"
        );
    }
}
