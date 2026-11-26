# Drill-down

The rows of a comparison, filtered, as a list or a tree.

**Filters** (a group named Filters): a toggle chip per category with rows (several can be on), an extension field applied with **Apply**, a size range (Any size, Under 1 MiB, 1–100 MiB, Over 100 MiB) and, for three sides or more, **Present on** checkboxes that keep rows held by exactly the checked sides. The dashboard's tiles and composition bar set the same filter.

**List | Tree** switch (a radio group named View).

- **List:** a [table](data-table.md) named Rows: path (monospace, sortable), category, size (sortable), a size column per side ("On Backup": the size, `dir`, `symlink`, or — when the side lacks it), modified. Sorting by path or size is done by the backend.
- **Tree:** a [tree](tree-view.md) named "Rows by folder": folders read "Photos — 1,204 · 3.1 GiB" (matching rows and bytes beneath), files by name.

Both refresh in place after every verification round, keeping scroll position and the active row. Enter, `o` or a double-click opens a row's [details](row-detail.md) beside the list; Escape closes them.
