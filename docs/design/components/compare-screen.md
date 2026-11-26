# Compare screen

Choosing what to compare, then seeing how it compares (`/compare`, `/compare/view?…`).

**Picker** (`/compare`): a checkbox per location, with an optional subfolder field for each chosen one; matching options (Ignore case, Unicode normalization (NFC), Fast mode with a time tolerance in seconds, Find duplicates within each location); **Compare**. At least two locations are needed, or one with Find duplicates on ("Choose at least two locations, or one with Find duplicates on."). Locations hashed with different algorithms are named with their algorithms ("Photos uses BLAKE3 and Phone uses SHA-256; compare locations with the same algorithm.") and cannot be compared. The form is at most `form.width` wide.

**URL:** the comparison lives in its address: `sides=Photos,Backup:2024` (each label and subfolder percent-encoded first, so commas and colons survive), `nfc=1`, `ci=1`, `fast=<ms>`, `dupes=1`. A reload or a shared link opens the same comparison; a link that names none says so with a link back to the picker.

**View:** a heading naming the sides ("Photos and Backup"), the verification line, the dashboard and the [drill-down](drill-down.md).

- **Verification line:** "Verifying 1,204 files — 38%" with **Cancel** while unverified rows are hashed; "All verified"; "Verification cancelled" or "Verification failed: …" with **Verify**. Every hashing round updates the numbers on the page.
- **Dashboard:** a [stat tile](stat-tile.md) per category that has rows (rows, and bytes of all sides), as toggle buttons in a group named Categories: choosing one filters the page by it, choosing it again clears the filter; the [composition bar](composition-bar.md) of category shares, whose segments filter the same way; two [charts](chart-frame.md): bytes by extension (the ten largest, then Other) and rows by size (`< 1 KiB` … `≥ 1 GiB`), each with its table for assistive technology. Charts follow the filter.

**Lifetime:** the comparison is opened when the view appears and closed when it goes; a window that starts again closes comparisons it lost.

**Errors:** a comparison that cannot be opened shows why, with a link back to the picker.
