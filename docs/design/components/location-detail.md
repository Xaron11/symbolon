# Location page

One location: what its catalog holds, its scans, and its files, browsable while a scan runs (`/locations/<label>`).

**Anatomy:** a [breadcrumb](breadcrumb.md) ("Locations › Photos") leading back to the list; a header with the label and the status badge, below it the root in monospace on one line (cut with an ellipsis, full in the tooltip), then one line of facts: "Last scanned 3 minutes ago · Complete · Hashed when compared", the time linking to [that scan](scan-detail.md) (while a scan runs, its progress instead). At the end of the header **Scan** (busy while this location is being scanned) and an actions menu (Rename…, Hash during every scan / Hash only when compared, Remove…); renaming opens the page under the new label, removing goes back to Locations. Below, four [stat tiles](stat-tile.md): Files, Folders, Size, Hashed (the share of files with a full digest). Then tabs:

- **Browse:** the catalog as a [tree](tree-view.md) of folders, files and links (each with its icon), each file with its size at the end. While a scan writes, it refreshes at most once a second and once at the end, keeping open folders and the active row.
- **Scans:** the scan history as a [table](data-table.md), newest first: started (the newest ended scan marked "Latest"), how long it took, outcome after a dot in its color, files, size, warnings. One click opens the scan's page.
- **Warnings** (with the count in the tab's name when there are any): the warnings of the last scan: path, kind, message.

Trees sit in panes `pane.height` tall; tables are only as tall as their rows, up to that height, and say "Not scanned yet." or "No warnings in the last scan." when empty.

**States:** loading (spinner); a location that no longer exists says "This location no longer exists." with a link back to Locations; Scan is disabled with the reason for imported locations. Scan errors behave as on the [Locations screen](location-table.md).

**Accessibility:** the tree is named "Files of Photos", the tables "Scans of Photos" and "Warnings of the last scan". The scan history is read again when a scan of this location ends. Scan progress is announced by the status bar, not here.
