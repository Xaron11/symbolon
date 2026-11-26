# Drill-down

The rows of a comparison, filtered, as a list or a tree.

**Filters** (a group named Filters): a toggle chip per category with rows (several can be on), an extension field applied with **Apply**, a size range (Any size, Under 1 MiB, 1–100 MiB, Over 100 MiB) and **Present on** checkboxes that keep rows held by exactly the checked sides. The dashboard's tiles and composition bar set the same filter.

**List | Tree** switch (a radio group named View).

- **List:** a [table](data-table.md) named Rows: path (monospace, sortable; for moves and renames each side's path, "old/a.jpg → new/a.jpg"), what the row says, naming the sides it concerns ("Only in Backup", "Moved Photos → Backup", "Identical"), size (sortable), a size column per side ("On Backup": the size, `dir`, `symlink`, or — when the side lacks it), modified. Sorting by path or size is done by the backend.
- **Tree:** a [tree](tree-view.md) named "Rows by folder": folders read "Photos — 1,204 · 3.1 GiB" (matching rows and bytes beneath), files by name and what the row says ("b.jpg — Only in Backup").

Both refresh in place after every verification round, keeping scroll position and the active row. Enter, `o` or a double-click opens a row's [details](row-detail.md) beside the list; Escape or Close closes them and gives focus back to the list or tree.
