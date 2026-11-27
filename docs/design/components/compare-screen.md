# Compare screen

Choosing what to compare, then seeing how it compares (`/compare`, `/compare/view?…`).

**Picker** (`/compare`) has two parts that look different, because they do different things:

- **Locations to compare:** a card per location (its label as a checkbox, its root, "1,804 files · 794 MiB · scanned 8 minutes ago" or "Not scanned yet"); the whole card toggles it, the chosen ones are outlined in the accent, and a chosen card shows "Only this subfolder (optional)".
- **Comparison options:** a quieter panel beside the cards (below them when the content is narrower than 720px), each option with a line saying what it does: Ignore case ("Photo.JPG and photo.jpg count as the same file."), Unicode normalization (NFC), Fast mode ("Files with equal size and time count as equal, without hashing; quicker, but can miss edited files.") with a time tolerance in seconds, written with a point or a comma and refused below zero ("Enter a tolerance of 0 seconds or more."), Find duplicates within each location.

Below both, a bar says what will be compared ("Backup and Laptop") beside **Compare**. At least two locations are needed, or one with Find duplicates on ("Choose at least two locations, or one with Find duplicates on."). Locations hashed with different algorithms are named with their algorithms ("Photos uses BLAKE3 and Phone uses SHA-256; compare locations with the same algorithm.") and cannot be compared. The options panel is `picker.options.width` wide.

**URL:** the comparison lives in its address: `sides=Photos,Backup:2024` (each label and subfolder percent-encoded first, so commas and colons survive), `nfc=1`, `ci=1`, `fast=<ms>`, `dupes=1`. A reload or a shared link opens the same comparison; a link that names none, cannot be decoded, or has a `fast` that is not a whole number of milliseconds says so with a link back to the picker.

**View:** a heading naming the sides ("Photos and Backup"), the verification line, **New comparison** (closes this one and opens the picker), the dashboard and the [drill-down](drill-down.md).

- **Verification line:** "Verifying 1,204 files — 38%" with **Cancel** while unverified rows are hashed; "All verified"; "Verification cancelled" or "Verification failed: …" with **Verify**. Every hashing round updates the numbers on the page.
- **Dashboard:** a [stat tile](stat-tile.md) per category that has rows (rows, and bytes of all sides), all one height, as toggle buttons in a group named Categories: choosing one filters the page by it, choosing it again clears the filter (and any Present on choice); hovering one lights it, the chosen one is outlined in the accent. Only-in rows get a tile per set of sides holding them ("Only in Photos", "Only in Backup", "Only in Photos and Laptop"), which also sets **Present on** to those sides. A category that verification empties drops out of the filter. Below the tiles, the [composition bar](composition-bar.md) of category shares, whose segments filter the same way, and in a two-sided comparison with moves or renames one line naming the direction ("Moved and renamed: path on Photos → path on Backup"). Two titled panels hold horizontal [bar charts](chart-frame.md), so labels never rotate: "Bytes by extension" (the ten largest, then Other; ticks on round binary steps) and "Files by size" (`< 1 KiB` … `≥ 1 GiB`), each with its table for assistive technology. Charts follow the filter.

**Duplicates** (when the comparison looks for them): per side its label and totals ("130 groups · 390 files · 260 MiB redundant", or "No duplicates"), then its groups as a [tree](tree-view.md) named "Duplicates in Photos": each group reads as its first copy's path with "3 copies · 2.0 MiB redundant" at the end and opens (a click on its row) to every copy's path. Groups are read from the backend page by page and only the visible rows exist, so thousands of groups need no Show more; the tree is only as tall as its rows, up to `pane.height`. It is read again after every verification round.

**Lifetime:** the comparison stays open while people visit other screens: Compare in the sidebar (or `/compare`) brings back the open one, with its filters and verification as they were. It closes when another comparison opens, on **New comparison**, and when the app quits; a window that starts again closes comparisons it lost.

**Errors:** a comparison that cannot be opened shows why, with a link back to the picker.
