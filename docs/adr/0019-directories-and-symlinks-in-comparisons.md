# 0019. Directories and symlinks in comparisons

- **Status:** Accepted
- **Date:** 2026-11-25

## Context

Scans record files, directories, symlinks and other objects, but comparisons look at regular files only ([ADR-0010](0010-comparison-engine.md), issue #49). A backup that lost its empty folders or its symlinks therefore compares as complete. That is common with FAT sticks, copies from phones and some sync tools.

Symlinks are never followed (spec §6, [ADR-0015](0015-opening-files-beneath-the-root.md)), so the only thing to compare about a link is its target, and the catalog does not store targets yet. A directory that holds files is already visible through its files' rows; only directories with nothing compared beneath them are invisible today.

## Decision

### Storing link targets

1. **Schema v4** adds `entries.link_target BLOB NULL`, an additive migration with a v4 fixture. It holds the target exactly as the system returns it (`readlink`), as raw bytes in the location's `path_encoding`. It is never resolved, normalized or checked against the root. It is set only on `symlink` entries. NULL means the target is unknown.
2. **Local scans** read each symlink's target with `fs::read_link`. On Windows the target is stored as WTF-8, like paths ([ADR-0004](0004-raw-path-encoding.md)). If reading it fails, the scan records a warning and leaves the target unknown.
3. **Every scan rechecks every symlink's target**, even when size and mtime are unchanged, because retargeting a link does not reliably change its mtime. This also fills in targets for catalogs migrated from v3 on their next scan.
4. **The agent** adds an optional `target` field to `walk` entry lines, escaped like paths ([ADR-0017](0017-android-over-adb.md)). A missing field means unknown. The protocol version stays 1.
5. **The toybox fallback** reads targets after listing, with `readlink` in batches. A target containing a line break becomes a warning and stays unknown.
6. **Exports** carry `link_target` unchanged. **Imports** check it before writing anything, as they check paths: it may be set only on symlink entries, it is at most 4,096 bytes, and it must be valid in the location's encoding. Exports from schema v3 import with unknown targets.

### Comparing

Comparisons take in files, symlinks and directories. Each side's own root (the location root or the subpath) is not an entry, and `other` entries are still ignored. Rows stay grouped by normalized key; each cell now has a kind. A row's category is checked in this order:

1. **Collision**, as in ADR-0010: one side holds two or more entries with this key. The exception is a collision made only of directories, for example `Photos/` and `photos/` under case-insensitive matching. It produces no row; the directories merge and their contents are compared as usual.
2. **Different** when the cells hold different kinds, for example a file on one side and a directory on another. Nothing beneath a directory side changes: its files and links have their own rows.
3. **Files** are classified exactly as in ADR-0010.
4. **Symlinks**, when every cell is a symlink:
   - **Different** if two known targets differ.
   - **Identical** if the link is on every side and all targets are known and equal.
   - **Only in P** if it is on some sides only and, when on more than one, all their targets are known and equal. A link on a single side is Only in that side even if its target is unknown.
   - **Unverified** otherwise: on two or more sides, a target is unknown, and no two known targets differ. Fast mode does not apply to symlinks.
5. **Directories**, when every cell is a directory, never become Identical or Different. A directory row is reported only as **Only in P**, and only if it is on some sides and not all, and no compared entry of any kind lies beneath it on the sides that have it. Otherwise it produces no row.

Moves, renames and duplicates stay limited to files. `Comparison::hash_needs` returns files only; a symlink with an unknown target needs a rescan, not hashing.

### Results

- Summaries count rows and entries of every kind. Byte totals count files only.
- Rows carry each cell's kind, and the target for symlinks.
- Row pages can be filtered by kind (`file`, `dir`, `symlink`).
- In the CLI's `diff`:
  - directories end in `/` in the path column;
  - a cell shows `dir` for a directory and `-> <target>` for a symlink, with control characters escaped like paths;
  - the summary's count column becomes `ENTRIES`, and the JSON and CSV field `files` becomes `entries`;
  - `--kind` filters the list.

### Invariants (added to the property tests)

- Every compared file and symlink is in exactly one row; a directory is in at most one.
- Identical symlink rows are present on every side with one known target.
- A directory row is Only in P, or Different because of a kind clash.
- The file invariants of ADR-0010 hold unchanged.

## Consequences

- A copy that dropped empty folders or symlinks shows up as incomplete, and a link pointing somewhere else shows up as different.
- Targets are compared as bytes. A link to an absolute path such as `/home/a/x` differs from one to `/mnt/backup/x` even when both point to the same file within their own copy. Relative links compare cleanly.
- Existing catalogs show their symlinks as Unverified until the next scan. Phones scanned by an older agent do the same.
- The JSON and CSV summary field `files` is renamed to `entries`. Scripts that read it need updating; the changelog says so.
- Collecting targets costs one `readlink` per symlink per scan.
- Revisit if links whose targets point inside the location root should be compared relative to that root, or if moves of empty directories or symlinks turn out to matter.
