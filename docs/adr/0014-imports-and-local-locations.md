# 0014. Imports never replace a scanned local location silently

- **Status:** Accepted
- **Date:** 2026-11-25

## Context

ADR-0005 treats a file whose label and fingerprint match a saved location as the same location and replaces its entries without a flag. ADR-0013 counts two absent fingerprints as a match. Fingerprints arrive in M4, so today every location has none. As a result, any export whose label matches a location scanned on this computer replaced that location's scans and entries with the file's (#81). The location stayed local and scannable. A later scan kept the file's digests for every file whose path, size and mtime matched, so another catalog decided which local files counted as identical or duplicate. This breaks ADR-0005's own rule that import "never deletes local data without an explicit choice".

## Decision

- **Matching.** An import into a location whose root is `LocationRoot::Local` always needs a choice:
  - Equal fingerprints (both absent included) are refused as `ImportConflict::LocalData`.
  - Different fingerprints are refused as `LabelTaken`, as before.
  - Either case is resolved by `as_label` (`--as`) or `replace` (`--replace`).
- **Unchanged cases.** The ADR-0005 table still applies to imported and foreign locations, so re-importing a newer export of an imported location needs no flag.
- **Replacing.** Replacing a local location's data, whether by `--replace` or `--into`, turns it into an imported location: an `imported_locations` row is added. Its digests come from the file, so it is compared but never scanned or hashed on demand here. To scan the directory again, remove the location and add it again.

## Consequences

- Digests from another catalog never become the identity of local files, and no flagless import deletes local scan data.
- The CLI's exit code 4 message for `LocalData` names `--as` and `--replace`, and warns that replacing makes the location imported.
- When M4 brings fingerprints, a verified same-device match into a local location still asks for a choice. If merging scans of the same device turns out to be useful, it needs its own design: for example, keeping imported digests separate from locally read ones.
- This amends the matching and replacing rules of ADR-0013 for local locations.
