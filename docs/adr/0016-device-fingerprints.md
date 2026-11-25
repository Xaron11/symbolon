# 0016. Device fingerprints and finding moved locations

- **Status:** Accepted
- **Date:** 2026-11-25

## Context

Spec §3 and §6.5: the user's label is a location's identity, and an automatic fingerprint is a best-effort hint. The fingerprint is used to recognize a device at a different mount point and to ask the user when the match is ambiguous. Until M4, every location had an empty `fingerprint`, and scans and comparisons trusted whatever was at the stored root. If a pendrive was not mounted but its empty mount point existed, a scan recorded every file as deleted. If another device was mounted at the same path, on-demand hashing read it and stored its digests for the wrong location. ADR-0006 reserves an `identity` feature for this.

## Decision

**What a fingerprint is.** A fingerprint is a string naming a directory on a specific device. Fingerprints are compared only for equality.

- **Local:** `fs:<filesystem UUID>:<path inside that filesystem>`, for example `fs:9ee6adca-…:/photos`. The path is absolute within the filesystem, not within the mount. So a bind or subdirectory mount (for example `/home` mounted from `/home` on the disk) and a btrfs subvolume (`/@home/…`) give the same fingerprint wherever they are mounted. Bytes that are not valid UTF-8, and `%`, are escaped as `%XX`, as in path keys.
- **Android (M5):** `adb:<serial>:<path>`.

**Where it comes from (Linux).** The `identity` feature is a default feature, and its Linux implementation lives behind `cfg(target_os = "linux")`. Detection reads `/proc/self/mountinfo`:

- **Finding the mount.** The mount of a canonical root is the entry with the longest mount point containing it. When mounts are stacked at the same path, the last entry wins.
- **Finding the UUID.** The filesystem's UUID comes from the `/dev/disk/by-uuid` link whose target has the same device number as the mount source.
- **No UUID, no fingerprint.** Mounts without a block device with a UUID (tmpfs, overlay, FUSE, network shares) get no fingerprint. The same goes for Windows and macOS for now; Windows volume GUIDs can follow without changing the format.

All of this sits behind a `Volumes` trait that lists mounts. Tests and other platforms can supply their own mount list.

**Recording.**

- `add_location` records the root's fingerprint.
- A completed scan records it if the location had none.
- `Catalog::relocate(label, root)` sets a new root and records its fingerprint.
- A fingerprint is never replaced silently.

**Locating.** `Catalog::locate(label)` reports where a local location is now:

| Recorded fingerprint | At the stored root                   | Elsewhere         | Result                     |
| -------------------- | ------------------------------------ | ----------------- | -------------------------- |
| none                 | a directory                          |                   | `Here`                     |
| some                 | the same fingerprint                 |                   | `Here`                     |
| some                 | missing, or another / no fingerprint | exactly one match | `Moved { to }`             |
| some                 | missing, or another / no fingerprint | several matches   | `Ambiguous { candidates }` |
| some                 | a directory with another / none      | no match          | `OtherDevice { found }`    |
| any                  | missing                              | no match          | `Missing`                  |

"Elsewhere" means the mounted filesystems with the recorded UUID on which the recorded inner path exists as a directory.

**Who decides.** Core never prompts.

- **Scans.** `Scanner::start` scans only a location that is `Here`. Every other case is a typed `ScanError` (`Moved`, `Ambiguous`, `OtherDevice`), and `Missing` stays `RootUnavailable`.
- **Comparisons.** A comparison hashes on demand only in a location that is `Here`. Any other location is reported as unreachable, with the reason.
- **The CLI.**
  - `scan` and `diff` follow an unambiguous move: they relocate and print a notice.
  - For `Ambiguous`, they ask on a terminal which candidate to use.
  - For `OtherDevice`, they ask whether to accept the device now at the root. Without a terminal, `--yes` accepts the other device.
  - Otherwise they stop with exit code 4 and name `symbolon location set <label> --root <path>`.
- **`location set`.** It changes a location's label, root or hash policy.

## Consequences

- A missing or foreign device is no longer scanned or hashed as if it were the location. A replugged device is found at its new mount point.
- Exports carry fingerprints, so the ADR-0005 import table starts to matter: the same device saved under another label is detected. ADR-0014 still requires a choice before replacing a local location's data.
- Copying a whole filesystem image (`dd`) duplicates its UUID, so both copies match. When both are mounted, that is reported as ambiguous.
- Locations without a fingerprint behave as before. Their mount point is trusted.
