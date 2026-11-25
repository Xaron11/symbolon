# 0015. Opening files beneath the location root

- **Status:** Accepted
- **Date:** 2026-11-25

## Context

Spec §6 says symlinks are never followed out of the location root. The walk respected that: it records entries with `lstat`. Hashing did not. It reopened each listed file with `fs::File::open(root.join(rel))`, which follows links in every component, and checked the result only by re-stating the path for size and mtime. Someone who can write into a scanned location could replace a listed directory with a symlink after the walk. Hashing then read a file outside the root and stored its digest under the in-root path, and a FIFO in the same place blocked the hashing thread (#82).

## Decision

`LocalSource::open` resolves the relative path one component at a time, beneath a handle on the root, and never follows a link.

- **Unix.**
  - Each directory is opened with `openat(parent, name, O_DIRECTORY | O_NOFOLLOW)`.
  - The file is opened with `O_NOFOLLOW | O_NONBLOCK`, so a FIFO cannot block.
  - This uses `rustix`, a safe wrapper that is already in the dependency graph through `tempfile`. Core stays free of `unsafe`.
- **Windows.**
  - std has no `openat`. Each directory is opened with `FILE_FLAG_OPEN_REPARSE_POINT` and checked not to be a reparse point.
  - Each directory handle is held without delete sharing until the file is open, so no component can be renamed or replaced in between.
  - The file itself is opened without following reparse points.
- **Every platform.** Whatever was opened must be a regular file according to the opened handle's own metadata, not a second path lookup.

A refused open is an I/O warning on the entry, like any unreadable file. The post-hash size and mtime check is unchanged.

## Consequences

- Hashing cannot read outside the root through links created after the walk, and cannot hang on special files.
- `rustix` becomes a direct, Unix-only dependency of core behind the `local` feature.
- The Windows path cannot be tested for symlinks in CI, because creating them needs a privilege. CI does run the normal hashing tests through it.
- The Android source (M5) reads files on the device. It needs the same rule in the agent: `O_NOFOLLOW` per component, enforced on the device.
