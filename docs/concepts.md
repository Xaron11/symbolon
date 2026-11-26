# Concepts

This chapter defines the words used everywhere else: in the CLI, the GUI, the code and the catalog.

## Location

A **location** is something you want to compare: a directory, a mounted disk or pendrive, or a directory on an Android phone (for example `adb:<serial>:/sdcard/DCIM`, the camera folder in the phone's internal storage). Each location has:

- a **label** you choose, such as `laptop-photos` or `blue-pendrive`. The label is the location's identity.
- a **fingerprint** Symbolon detects automatically (filesystem UUID; ADB serial from M5). It only helps recognize a device plugged in at a different mount point; when it is ambiguous, or another device is in its place, Symbolon asks you instead of guessing (see below).
- a **hash algorithm** (BLAKE3 by default; xxh3 or SHA-256 possible). Locations hashed with different algorithms cannot be compared until one is rehashed.
- a **hash policy**: `lazy` (hash only what a comparison needs; good for always-connected storage) or `eager` (hash everything during the scan; good for removable media you will not have later).

A comparison can target a sub-path of a location, written `label:subpath`.

### Devices and moved locations

A fingerprint names a directory on a specific device: `fs:<filesystem UUID>:<path inside the filesystem>`, for example `fs:5ADB-A50A:/DCIM`. It does not depend on where the filesystem is mounted, so it stays the same whether a pendrive shows up at `/media/usb` or `/run/media/me/USB`. It is detected on Linux. Filesystems without a UUID (tmpfs, network shares) and other operating systems have none yet, and for those locations the root path is trusted ([ADR-0016](adr/0016-device-fingerprints.md)).

Before scanning, or hashing during a comparison, Symbolon checks where the location's device is:

| Situation                                                      | What happens                                                                   |
| -------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| The device is at the location's root                           | It is read as usual.                                                           |
| The device is mounted at exactly one other place               | The location follows it there, and you are told.                               |
| The device matches several mounted directories (a cloned disk) | You pick one; until then it is not scanned.                                    |
| Another device, or an empty mount point, is at the root        | You confirm before it is scanned as the location; it is never hashed silently. |
| Neither the root nor the device is there                       | The device is not connected: nothing is read, and the catalog still compares.  |

This is what keeps an unplugged pendrive's empty mount point from being scanned as a location whose files all vanished. `symbolon location set <label> --root <path>` points a location somewhere else by hand.

## Catalog

The **catalog** is a local SQLite database holding every location, scan and file entry. It is the single source of truth: comparisons run against the catalog, not the disk, which is why a disconnected device can still be compared. A single location can be exported to a standalone `.symbolon` file and imported into another catalog ([ADR-0005](adr/0005-export-format-and-import-conflicts.md)).

Catalogs contain the full paths and content hashes of your files. Treat them like the data itself.

## Scan and entries

A **scan** walks a location and records one **entry** per file, directory or symlink: its relative path, size, modification time, kind, a symlink's target, and hashes once known. Symlinks are recorded with their target exactly as the system returns it, never followed, also when a file is opened for hashing ([ADR-0015](adr/0015-opening-files-beneath-the-root.md)). Unreadable files become warnings on their entries; they never abort the scan.

Rescans are incremental: entries whose size and modification time did not change keep their hashes, so only new or changed files are read.

## Paths and path keys

Paths are stored exactly as the filesystem reports them (raw bytes), and separately as a normalized **path key** used for matching ([ADR-0004](adr/0004-raw-path-encoding.md)). Per comparison you can match case-insensitively and treat Unicode-equivalent names (NFC/NFD) as equal. When several files collapse to one key, the comparison reports the collision instead of merging them.

## Hash tiers

Reading every byte of 100 GB is slow, so Symbolon hashes in tiers and only as far as needed:

| Tier         | What is known                                   | Can prove identical?       |
| ------------ | ----------------------------------------------- | -------------------------- |
| **unhashed** | size and modification time                      | no                         |
| **partial**  | hash of the size plus the first and last 64 KiB | no, only rules out matches |
| **verified** | hash of the full content                        | yes                        |

A file whose size is unique among the compared locations cannot match anything and is never read. Two files are called identical only when both are **verified** with equal hashes. A fast mode that assumes "same size and time means same content" exists, and its results are labelled unverified.

## Comparison categories

Comparing two or more locations (or subpaths of them) sorts every file into exactly one category ([ADR-0010](adr/0010-comparison-engine.md)):

| Category              | Meaning                                                                                                                   |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| **Identical**         | Same path and same full hash in every compared location.                                                                  |
| **Different content** | Same path in two or more locations, different size or hash.                                                               |
| **Only in …**         | Present in some locations only, with equal content where present; with three or more locations this is a presence matrix. |
| **Moved**             | Same content in different directories, same file name.                                                                    |
| **Renamed**           | Same content under a different file name (possibly also moved).                                                           |
| **Collision**         | Two files of one location become the same path under case-insensitive or Unicode matching.                                |
| **Unverified**        | Same path and size, not hashed yet, so equality is unknown.                                                               |

Duplicates within one location are listed separately: a file can be identical across locations and also duplicated inside one.

Example with two locations, `laptop` and `pendrive`:

| Path on `laptop`    | Path on `pendrive`         | Category          |
| ------------------- | -------------------------- | ----------------- |
| `docs/cv.pdf`       | `docs/cv.pdf` (same bytes) | identical         |
| `notes/todo.md`     | `notes/todo.md` (edited)   | different content |
| `photos/2024/a.jpg` | `archive/a.jpg`            | moved             |
| `music/song.flac`   | `music/song (1).flac`      | renamed           |
| `taxes/2023.ods`    | —                          | only in `laptop`  |

Comparing lazily hashed locations reads only what decides a category: files with a size no other candidate has are never read, partial hashes rule out most differences cheaply, and full hashes are computed only where partial hashes match. A fast mode can assume "same size and modification time means same content"; such results are always marked as unverified.

## Write operations

Symbolon is read-only until milestone M7. Writes will always be planned first (an action plan you can review), dry-run by default, and reversible: "delete" moves files to a quarantine directory, copies are verified by hashing the destination, and every action is journaled.
