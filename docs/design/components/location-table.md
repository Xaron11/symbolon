# Locations screen

The list of saved locations and what can be done with each.

**Anatomy:** an "Add location" button above a table with one row per location: the label (in medium weight; a link to the location's page), its root in monospace (cut with an ellipsis, full in a tooltip), a status badge, the last scan ("3 hours ago", "Never", and the outcome when it did not complete), file count and size in monospace, when files are hashed ("When compared" or "Every scan"), a small **Scan** button ("Scan Photos"; busy while the location is being scanned) and an actions menu of the same height, which opens aligned to its right edge so it stays in the window. The last scan's time links to [its page](scan-detail.md); while a scan runs the column says "Scanning…" and links to Scans. Cells stay on one line; the root takes the room the other columns leave.

**Rows:** clicking anywhere on a row that is not a button or link opens the location's page; rows show the hover tint and a pointer, and the label is underlined on hover.

**Status badges** are flat (see [Badge](badge.md)) and pair an icon with a word, so color is never the only signal: Connected, Missing, Moved, Several places, Other device, Phone not attached, Imported. A moved location offers **Use new path**, which points it at where its device is now; one found at several places lists them in its actions menu. Imported locations say "Imported locations cannot be scanned." and their Scan button is disabled.

**Actions menu** ("Actions for Photos"): the places of an ambiguous location · Rename… · Hash during every scan / Hash only when compared · Remove…. Removing asks "Remove Photos from the catalog? Files on disk are not touched." with **Remove Photos** and **Keep Photos**.

**Errors** become sentences people can act on: a phone that is not ready says "Connect the phone and allow USB debugging, then try again."; scanning a location that is already being scanned opens Scans; removing or renaming one says "Stop the scan first.". Other failures show the bridge's message in a toast.

**Empty state:** "No locations yet", one sentence on what a location is, and **Add location**.

**Keyboard:** the label link is the keyboard's way to a location's page; Tab reaches each location link, Use new path button, Scan button and actions menu; the palette's "Add location" (`Ctrl+K`) opens the dialog from anywhere. Locations are few, so this is a plain table, not a virtualized [DataTable](data-table.md).

**Accessibility:** a table with column headers and the label as each row's header; the actions button is named after its location.
