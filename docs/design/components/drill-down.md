# Drill-down

The rows of a comparison, filtered, as a list or a tree.

**Filters** (a group named Filters): a toggle button per category with rows (several can be on), outlined and hoverable with the category's icon, a check mark and the accent outline when on, so they read as controls, unlike the flat category chips elsewhere; an extension field applied with **Apply**, a size range (Any size, Under 1 MiB, 1–100 MiB, Over 100 MiB) and **Present on** checkboxes that keep rows held by exactly the checked sides. The dashboard's tiles and composition bar set the same filter.

**List | Tree** switch: a segmented control (a radio group named View); the chosen segment is raised.

- **List:** a [table](data-table.md) named Rows: path (monospace, sortable; for moves and renames each side's path, "old/a.jpg → new/a.jpg"), what the row says, after a dot in the category's color, naming the sides it concerns ("Only in Backup", "Moved: Photos → Backup", "Identical"), size (sortable), a size column per side ("On Backup": the size, `dir`, `symlink`, or — when the side lacks it), modified. Sorting by path or size is done by the backend. Paths and names are shown as written, also when matching ignores case or Unicode form. "Nothing matches the filters." when no row does.
- **Tree:** a [tree](tree-view.md) named "Rows by folder": names on the left; on the right, for folders the matching rows and bytes beneath ("1,204 files · 3.1 GiB"), for files what the row says ("Only in Backup"). A folder that is a row itself (an empty folder on one side) keeps the folder icon. It says "Loading…" until the first level arrives.

Both refresh in place after every verification round, keeping scroll position and the active row. A click or the keys mark the current row the same way: a gray fill, with the accent bar while the list has focus; the fill stays when focus moves to the details. Enter, `o` or a double-click opens a row's [details](row-detail.md) in a `detail.width` panel beside the list, which gives way; while the panel is open it follows the current row; Escape or Close closes them and gives focus back to the list or tree.
