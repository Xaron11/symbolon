# Concepts

This chapter defines the words used everywhere else: in the CLI, the GUI, the code and the catalog.

## Location

A **location** is something you want to compare: a directory, a mounted disk or pendrive, or a directory on an Android phone (for example `adb:<serial>:/sdcard/DCIM`). Each location has:

- a **label** you choose, such as `laptop-photos` or `blue-pendrive`. The label is the location's identity.
- a **fingerprint** Symbolon detects automatically (filesystem UUID, ADB serial). It only helps recognize a device plugged in at a different mount point; when it is missing or ambiguous, Symbolon asks you instead of guessing.
- a **hash algorithm** (BLAKE3 by default; xxh3 or SHA-256 possible). Locations hashed with different algorithms cannot be compared until one is rehashed.
- a **hash policy**: `lazy` (hash only what a comparison needs; good for always-connected storage) or `eager` (hash everything during the scan; good for removable media you will not have later).

A comparison can target a sub-path of a location, written `label:subpath`.

## Catalog

The **catalog** is a local SQLite database holding every location, scan and file entry. It is the single source of truth: comparisons run against the catalog, not the disk, which is why a disconnected device can still be compared. A single location can be exported to a standalone `.symbolon` file and imported into another catalog ([ADR-0005](adr/0005-export-format-and-import-conflicts.md)).

Catalogs contain the full paths and content hashes of your files. Treat them like the data itself.

## Scan and entries

A **scan** walks a location and records one **entry** per file, directory or symlink: its relative path, size, modification time, kind, and hashes once known. Symlinks are recorded, never followed. Unreadable files become warnings on their entries; they never abort the scan.

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

Comparing two or more locations sorts every file into these categories:

| Category              | Meaning                                                                       |
| --------------------- | ----------------------------------------------------------------------------- |
| **Identical**         | Same path key and same full hash in every compared location.                  |
| **Different content** | Same path key, different size or hash.                                        |
| **Only in …**         | Present in some locations only; with three or more this is a presence matrix. |
| **Moved**             | Same content, same file name, different directory.                            |
| **Renamed**           | Same content, different file name.                                            |
| **Duplicate**         | Same content at several paths within one location.                            |

Example with two locations, `laptop` and `pendrive`:

| Path on `laptop`    | Path on `pendrive`         | Category          |
| ------------------- | -------------------------- | ----------------- |
| `docs/cv.pdf`       | `docs/cv.pdf` (same bytes) | identical         |
| `notes/todo.md`     | `notes/todo.md` (edited)   | different content |
| `photos/2024/a.jpg` | `archive/a.jpg`            | moved             |
| `music/song.flac`   | `music/song (1).flac`      | renamed           |
| `taxes/2023.ods`    | —                          | only in `laptop`  |

## Write operations

Symbolon is read-only until milestone M7. Writes will always be planned first (an action plan you can review), dry-run by default, and reversible: "delete" moves files to a quarantine directory, copies are verified by hashing the destination, and every action is journaled.
