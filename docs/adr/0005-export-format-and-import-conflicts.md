# 0005. `.symbolon` export identity and import conflict policy

- **Status:** Accepted
- **Date:** 2026-11-24
- **Amended by:** [0014](0014-imports-and-local-locations.md): a location scanned on this computer is never replaced without a choice

## Context

Per-location export is a standalone SQLite file with a Symbolon-specific `application_id` (spec §3, §4.4). Open question §15.3: pick the value and define how import resolves conflicts between the file's location and saved locations (identity = user label, fingerprint = best-effort hint, spec §3).

## Decision

**File identity.**

- `PRAGMA application_id = 0x53594D42` (ASCII `SYMB`, decimal 1398361410). Not in SQLite's registry of known IDs (`magic.txt`) at the time of writing.
- `PRAGMA user_version` = catalog schema version. The `meta` table also records `app_version`, `exported_at`, `kind = 'export'`, and the exported location's label, fingerprint, hash algorithm and path encoding.
- The same `application_id` is set on the main catalog with `meta.kind = 'catalog'`, so tooling can recognise both.

**Opening an export.**

- Wrong `application_id` → refuse ("not a Symbolon file").
- `user_version` newer than this build supports → refuse with the required app version.
- Older `user_version` → migrate a temporary copy, never the user's file.

**Import conflicts.** The file's location is matched against saved locations by label and by fingerprint:

| Label    | Fingerprint                    | Result                                                                                               |
| -------- | ------------------------------ | ---------------------------------------------------------------------------------------------------- |
| no match | no match                       | Import as a new location.                                                                            |
| match    | match (or both absent)         | Same location: replace its entries if the imported scan is newer; if older, refuse unless `--force`. |
| match    | different                      | Refuse. Ask the user: import under a new label (`--as <label>`) or replace (`--replace`).            |
| no match | matches another saved location | Ask the user: merge into that location (`--into <label>`) or import separately (`--as <label>`).     |

In the CLI, an ambiguous case without a resolving flag is an error with a clear message listing the options (stable exit code). The GUI shows the same choices as a dialog. Import never renames silently and never deletes local data without an explicit choice.

## Consequences

- Import is deterministic and scriptable; every destructive outcome needs a flag or a click.
- A `.symbolon.zst` variant can wrap the same file later without changing this policy.
