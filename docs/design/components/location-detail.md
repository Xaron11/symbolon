# Location page

One location: what its catalog holds, its scans, and its files, browsable while a scan runs (`/locations/<label>`).

**Anatomy:** a header with the label, the root in monospace, the status badge and **Scan** at the end; while a scan of this location runs, its progress ("Scanning Photos — 1,204 files") stands before the button, which is disabled. Below, four [stat tiles](stat-tile.md): Files, Folders, Size, Hashed (the share of files with a full digest). Then tabs:

- **Browse:** the catalog as a [tree](tree-view.md) of folders and files. While a scan writes, it refreshes at most once a second and once at the end, keeping open folders and the active row.
- **Scans:** the scan history as a [table](data-table.md), newest first: started, outcome, files, size, warnings.
- **Warnings:** the warnings of the last scan: path, kind, message.

Tables and trees sit in panes `pane.height` tall.

**States:** loading (spinner); a location that no longer exists says "This location no longer exists." with a link back to Locations; Scan is disabled with the reason for imported locations. Scan errors behave as on the [Locations screen](location-table.md).

**Accessibility:** the tree is named "Files of Photos", the tables "Scans of Photos" and "Warnings of the last scan". The scan history is read again when a scan of this location ends. Scan progress is announced by the status bar, not here.
