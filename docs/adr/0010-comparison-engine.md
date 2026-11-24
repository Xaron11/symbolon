# 0010. Comparison engine

- **Status:** Accepted
- **Date:** 2026-11-24

## Context

M2 compares two or more locations from the catalog (spec §7): identical, different content, only in some locations, moved, renamed, and duplicates within a location. It must be a deterministic function of catalog state, refuse mixed hash algorithms, hash lazily (only what decides a category), and serve results page by page without loading 200k rows into memory. Path matching follows ADR-0004 (path keys, optional NFC and case-insensitive matching).

## Decision

### Inputs

- A comparison has N ≥ 2 **sides**. A side is a location label with an optional subpath (`label:sub/path`); the same location may appear twice with different subpaths. Keys are taken relative to the subpath.
- All sides must use the same hash algorithm; otherwise `Comparison::new` fails with an error naming every location and its algorithm. Rehashing is a client action.
- Only regular files take part in M2 (directories and symlinks: issue #49).
- Options: `KeyOptions` (NFC, case-insensitive), an opt-in fast mode that assumes equal content from equal size and mtime (with a tolerance), and whether to list duplicates.

### Rows and categories

Compared files are grouped by normalized relative key into **path rows**. For a path row, _P_ is the set of sides holding the key. Each path row gets exactly one category, checked in this order:

1. **Collision**: some side holds two or more files with this normalized key (for example `Readme` and `README` with case-insensitive matching). Reported as is; never matched further.
2. **Different**: |P| ≥ 2 and the sizes differ (decided without reading files), or full digests differ, or partial digests differ.
3. **Identical**: P is every side and all full digests are equal.
4. **Only in P**: P is a strict subset and the files in P (if more than one) have equal full digests. With one side this is a plain "only in X".
5. **Unverified**: |P| ≥ 2 with equal sizes but missing digests, so equality is unknown. In fast mode, a row whose sizes are equal and whose mtimes agree within the tolerance becomes **Identical** or **Only in P** with an `assumed` flag instead, and clients must label it as unverified.

**Moves.** Among verified _Only in_ rows, rows are grouped by digest. A row _participates_ in a move if another row of the same digest has a presence set disjoint from its own. All participating rows of a digest form one **move group**, which replaces them:

- **Moved** if every placement has the same normalized file name (different directories).
- **Renamed** otherwise (a rename, possibly combined with a move).

Examples (sides A, B, C): A `x` + B `y` is a move; A,B `x` + C `y` is a move; A `x` + A,B `y` is not (A just holds an extra copy, which the duplicates listing shows). The disjointness rule makes the result independent of side order.

**Duplicates** within one side (same full digest at several paths of that side's scope) are a separate listing, not a row category: a file can be identical across locations and duplicated within one.

Every compared file belongs to exactly one row.

### Hashing on demand

`Comparison::hash_needs` returns the entries whose digests would change a category, using what the catalog already knows:

- **Candidates:** path rows with |P| ≥ 2 and one size; _Only in_ files whose size matches an _Only in_ file with a disjoint presence set (possible moves); size collisions within a side when duplicates are requested. Files whose size is unique among the candidates are never read.
- **Tier 1, partial:** candidates without a partial digest. Files of at most `2 × PARTIAL_BLOCK` bytes go straight to a full digest, since a partial digest would read the whole file anyway.
- **Tier 2, full:** candidates whose partial digest still matches another candidate's.

Hashing reuses the scanner's pipeline (thread pool, single writer, re-check after hashing, guarded updates, events, cancel). A location that cannot be read leaves its rows unverified instead of failing the comparison. `Comparison::refresh` re-reads the catalog and reclassifies; a client loops `hash_needs` → hash → `refresh` (at most two rounds).

### Evaluation and results

- `Comparison::new` copies the compared entries into temporary tables on its own connection within one read transaction (a consistent snapshot) and classifies them with SQL. Normalized keys, file names and extensions come from deterministic SQLite functions backed by `pathkey`.
- Results stay in those temporary tables on disk. Clients read a summary (count and bytes per category) and pages of rows with keyset cursors, filtered by category, extension, size range, path prefix and presence set. No call returns an unbounded list.

### Invariants (checked by property tests against an in-memory reference model)

- Every compared file is in exactly one row.
- _Identical_ rows are present on every side with one digest; no other category claims equality without digests, except fast-mode rows marked `assumed`.
- Reordering the sides permutes presence sets and cells but changes no category.
- Move-group placements share a digest; their rows were _Only in_ before matching.
- Fast mode turns only _Unverified_ rows into assumed results; verified rows are unchanged.

## Consequences

- Comparisons scale with SQLite, not with memory, and results can be browsed or exported page by page.
- A comparison is a snapshot: catalog changes after `new`/`refresh` are not visible until the next `refresh`.
- Presence sets are bitmasks, which limits a comparison to 63 sides.
- Moves need full digests; unhashed lazy locations show moves only after `hash_needs` has been resolved.
