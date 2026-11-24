# 0013. Imported locations

- **Status:** Accepted
- **Date:** 2026-11-24

## Context

ADR-0005 fixes the `.symbolon` file identity and the import conflict table. Implementing it (#66) raised questions the table does not answer:

- A location imported on another computer of the same platform has a root path that may exist here and mean something else. Scanning it would silently replace the imported data with whatever this computer has at that path.
- Schema v1 allows only `root_kind = 'local'`. Changing that `CHECK` means rebuilding `locations`, which with foreign keys on deletes every scan and entry through `ON DELETE CASCADE`.
- What "newer" means, what happens when only one side has a fingerprint, and what replacing does to a location's hash algorithm and path encoding.

## Decision

- **Schema v2** adds `imported_locations (location_id, exported_at, exported_by, imported_at)`. It is additive: no table is rebuilt. A location created by an import gets a row; it reads as `LocationRoot::Imported`, which compares like any location and is never scanned or hashed on demand (`ScanError::NotScannable`).
- **What is exported:** the location row, its newest completed scan and all its entries as recorded, read in one transaction. A location without a completed scan cannot be exported. The file is written next to the target as `<file>.partial`, switched to `journal_mode = DELETE` so it travels as one file, and renamed into place.
- **Matching** follows ADR-0005. Fingerprints match only when equal, both absent included. If only one side has a fingerprint, the label belongs to another device, which errs on the side of asking.
- **Newer:** a scan is newer when it started later. An import of a scan that started at the same time or later replaces the location's data; an older one needs `force`. `replace` (another device under the same label) skips the age check, since the two scans are unrelated.
- **Replacing** deletes the location's scans and entries and inserts the file's in one transaction, under the location's scan lock (ADR-0012), so it cannot interleave with a scan. The location keeps its label and root. Its hash algorithm becomes the file's, since all digests now come from the file. It takes the file's fingerprint if it had none, or always with `replace`. A different path encoding is refused; import under another label instead.
- **Opening:** a file is an export only with Symbolon's `application_id` and `meta.kind = 'export'`. A newer schema is refused; an older one is migrated in a temporary copy, and the user's file is never written.

## Consequences

- Imported data cannot be overwritten by an accidental scan; refreshing it means importing a newer export.
- An imported location records when and by which version it was exported, for the CLI and GUI to show.
- When M4 brings fingerprints and M5 brings Android roots, `imported_locations` stays separate from how a root is reached, so neither needs to rebuild `locations`.
