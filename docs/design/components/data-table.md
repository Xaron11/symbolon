# DataTable

Shows large result sets (hundreds of thousands of rows) as a sortable, selectable grid that stays smooth: only the visible rows exist, and rows are read page by page as the view moves.

## Data contract

A table reads its rows through a data source; the desktop app's bridge implements it over the catalog and comparison queries, a web client over HTTP, tests and the gallery in memory:

| Operation                    | Meaning                                                                                                              |
| ---------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| `count()`                    | Total number of rows.                                                                                                |
| `rows(offset, limit, sort?)` | Up to `limit` rows from `offset`, in the requested sort order (`column`, `asc` or `desc`) or the source's own order. |

The contract is offset-based, not cursor-based, so the table can jump anywhere (dragging the scrollbar, `G` for the last row) without reading earlier pages ([ADR-0022](../../adr/0022-design-system.md)). The table asks for whole pages (200 rows) around the viewport once scrolling has paused for 30 ms, so dragging the scrollbar reads only where it stops; pages already read show at once. It keeps at most 10 pages, never dropping the ones in view, and ignores answers for pages no longer in view or for an older sort. A failed read shows an inline error with Retry.

## Anatomy

Sticky header (sunken surface, small secondary text) with a sort button for sortable columns; rows of `table.row.height` on the base surface; cells on one line, truncated with an ellipsis. The full value is in the cell's text (read by assistive technology) and its tooltip. Columns can be monospace (paths, hashes, sizes) and end-aligned (numbers); columns without a width share the remaining space but never shrink below `table.column.min`; header and rows always share the same columns, so a long value is cut, never widening its row; tables wider than their space scroll horizontally.

**States:** hovered row (hover tint) · selected rows (selected tint) · active row (accent bar on its leading edge while the table has focus) · active cell (focus-colored outline).

## Keyboard

Focus the table with Tab; then the [list keys](../keyboard.md#in-tables-and-trees) apply: `j`/`k` or ↓/↑ move, `h`/`l` or ←/→ change the active column, `g g`/`G` or Home/End jump to the first or last row, `Ctrl+D`/`Ctrl+U` move half a page, PageUp/PageDown a page, Space or `x` select, Enter or `o` open, `s` sorts by the active column (ascending, descending, then the source's order), as does clicking a sortable header. Sorting clears the selection, which is by position. Clicking a row makes it active; focus stays on the table.

## Accessibility

`role="grid"` with a label, `aria-rowcount` (the full count, not the rendered rows), `aria-multiselectable`; rows carry `aria-rowindex` and `aria-selected`; sortable headers `aria-sort`. The active cell is announced through `aria-activedescendant`, so focus stays on the grid while rows come and go.

## Responsive

Below 768px the table keeps its rows and scrolls horizontally (untested in M6). A card layout for phones is left to the mobile client, since cards need a different row height than the virtualization assumes.
