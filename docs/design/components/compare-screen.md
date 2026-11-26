# Compare screen

Choosing what to compare, then seeing how it compares (`/compare`, `/compare/view?…`).

**Picker** (`/compare`): a checkbox per location, with an optional subfolder field for each chosen one; matching options (Ignore case, Unicode normalization (NFC), Fast mode with a time tolerance in seconds, written with a point or a comma and refused below zero ("Enter a tolerance of 0 seconds or more."), Find duplicates within each location); **Compare**. At least two locations are needed, or one with Find duplicates on ("Choose at least two locations, or one with Find duplicates on."). Locations hashed with different algorithms are named with their algorithms ("Photos uses BLAKE3 and Phone uses SHA-256; compare locations with the same algorithm.") and cannot be compared. The form is at most `form.width` wide.

**URL:** the comparison lives in its address: `sides=Photos,Backup:2024` (each label and subfolder percent-encoded first, so commas and colons survive), `nfc=1`, `ci=1`, `fast=<ms>`, `dupes=1`. A reload or a shared link opens the same comparison; a link that names none, cannot be decoded, or has a `fast` that is not a whole number of milliseconds says so with a link back to the picker.

**View:** a heading naming the sides ("Photos and Backup"), the verification line, the dashboard and the [drill-down](drill-down.md).

- **Verification line:** "Verifying 1,204 files — 38%" with **Cancel** while unverified rows are hashed; "All verified"; "Verification cancelled" or "Verification failed: …" with **Verify**. Every hashing round updates the numbers on the page.
- **Dashboard:** a [stat tile](stat-tile.md) per category that has rows (rows, and bytes of all sides), as toggle buttons in a group named Categories: choosing one filters the page by it, choosing it again clears the filter. Only-in rows get a tile per set of sides holding them ("Only in Photos", "Only in Backup", "Only in Photos and Laptop"), which also sets **Present on** to those sides; in a two-sided comparison the Moved and Renamed tiles name the direction ("Photos → Backup"), from the first side's path to the second's; a category that verification empties drops out of the filter; the [composition bar](composition-bar.md) of category shares, whose segments filter the same way; two [charts](chart-frame.md): bytes by extension (the ten largest, then Other) and rows by size (`< 1 KiB` … `≥ 1 GiB`), each with its table for assistive technology. Charts follow the filter.

**Duplicates** (when the comparison looks for them): a section per side with its totals ("Photos: 130 groups, 390 files, 260 MiB redundant", or "Backup: no duplicates") and its groups, a hundred at a time with **Show more**: the first 12 hex digits of the shared digest, the number of copies and the bytes the extra copies take; each group expands to its paths. It is read again after every verification round.

**Lifetime:** the comparison is opened when the view appears and closed when it goes; a window that starts again closes comparisons it lost.

**Errors:** a comparison that cannot be opened shows why, with a link back to the picker.
