# 0023. Offset reads, sort orders and a tree for results

- **Status:** Accepted
- **Date:** 2026-11-26

## Context

[ADR-0010](0010-comparison-engine.md) pages comparison rows, and the catalog queries page entries, scans and warnings, with keyset cursors: "continue after this row". Cursors suit the CLI, which streams a result from start to end, and they never skip or repeat rows while a scan inserts more.

The GUI shows results in virtualized tables and trees ([ADR-0022](0022-design-system.md) §6). Dragging a scrollbar to row 190,000 of 200,000, or pressing `G` for the last row, must read that page directly, which cursors cannot do without walking every page before it. The GUI also sorts tables by size, shows comparison results as a folder tree with totals per folder, and draws charts by file extension and by size.

## Decision

1. **Offsets alongside cursors.** This amends the paging of [ADR-0010](0010-comparison-engine.md). `Page::at(offset, limit)` reads a page by position. `Comparison::rows`, `Comparison::duplicates`, `Query::children`, `Query::scans` and `Query::warnings` accept both forms; a page with both is rejected. Offset pages return no next cursor. `Comparison::children` and `Query::recent_scans` are read by offset only; `Query::search` and `Query::duplicates` by cursor only, and they reject offset pages rather than ignore them. Each of these lists has a count (`Comparison::count`, `child_count`, `Query::child_count`, `scan_count`, `warning_count`, `recent_scan_count`), so a client knows how far it can scroll.
2. **Offsets are stable where it matters.** Comparison results do not change until `Comparison::refresh`, so an offset names the same row for as long as a client shows them. Catalog lists can grow during a scan; a client that pages them by offset re-reads the count when the scan reports progress.
3. **Sort orders.** `Comparison::rows` takes a `RowOrder`: by path or by the size of the row's largest file, ascending or descending; rows without files come last in both size orders, ties go by path. Cursor pages exist only for the default order, path ascending, and keep their own SQL, which seeks the index from the cursor: a keyset condition combined with the offset form in one statement made SQLite scan from the start on every page. Every order matches an index term for term, so a deep offset skips index entries instead of sorting. The size indexes are built on the first page read in that order, as the lookup indexes are built only when needed (ADR-0010).
4. **A tree of results.** `Comparison::children(filter, parent, page)` lists what lies directly in folder `parent`: rows there that match the filter, and folders with matching rows beneath them, with their row and byte totals. Folders come from ranges of path keys, not from a stored parent: a folder exists in the tree only while something under it matches. A moved or renamed row sits at its first placement, as in the CLI.
5. **Aggregates for charts.** `Comparison::by_extension(filter, limit)` and `Comparison::size_histogram(filter)` count rows and bytes (the files of all sides together, as in summaries) per extension (the largest `limit`, then the rest together) and per size bucket (`SIZE_BUCKETS`: below 1 KiB, 1 MiB, 10 MiB, 100 MiB, 1 GiB, and larger). Rows without files are not counted.

## Consequences

- The GUI can jump anywhere in a result. On the 200k × 2 acceptance comparison the last page of 221,000 rows takes about 12 ms by path and by size (after a one-time 110 ms to build the size index), 30–40 ms with a category filter; the root of the tree, with 22,001 children, takes 180 ms ([Benchmarks](../benchmarks.md)). `just test-acceptance` fails if a last page takes 50 ms or more.
- `OFFSET` cost grows with the offset and with how many rows a filter skips. If that becomes too slow, store a dense row number per order after classification and seek by it.
- The CLI and other streaming clients keep using cursors; walking all 221,000 rows of that comparison in pages of 1,000 takes 0.8 s, as before, and `just test-acceptance` fails above 1.5 s.
- Expanding a tree level groups the matching rows of the whole comparison by their next path segment: 30–40 ms for a folder of the 200k × 2 comparison, growing with the comparison rather than the folder. Restricting the scan to the folder's key range is the next step if that becomes slow.
- Folder names in the tree are path key text; `TreeChild::Folder::display` turns escapes back into bytes for people (lossy). Under case-insensitive or NFC matching the name is taken from one of the files beneath, so it keeps its spelling.
