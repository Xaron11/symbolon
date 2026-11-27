# Scans screen

Running scans and the history of every scan (`/scans`).

At the top, a line summing up the history ("8 scans of 4 locations") and **Scan location**, a menu of the locations (those that cannot be scanned, or are being scanned, are disabled). Then two sections under level-two headings, **Running** and **Recent scans**.

**Running scans** are cards, `scan.card.min-width` wide at least and wrapping as the window narrows. Each card has the location's label (a link to its page, where the catalog can be browsed while the scan runs), the phase as a [badge](badge.md) ("Starting", "Listing files", "Hashing", or "Paused" in the warning tone) and, while hashing, the percentage. Below that is a [progress bar](progress-bar.md), indeterminate while files are listed and by bytes while hashing. Then the facts as label–value pairs: Files, Size, Speed, Elapsed and, while hashing, Time left (estimated from the hashing rate). Last come **Pause** (one button that becomes **Resume**, so focus stays on it) and **Cancel**. A scan that ends leaves the cards and a toast says how it ended.

**Recent scans** are a [table](data-table.md) of every location's scans, newest first: location (with a flat "Latest" badge on each location's newest ended scan), started, how long it took ("< 1 s", "3 min 20 s"), outcome after a dot in its color (Complete, Cancelled, Failed, Partial when the app stopped during the scan), files, size and warnings. A running scan's counts read "—" until it ends; its card shows them meanwhile. One click, Enter or `o` opens the [scan's page](scan-detail.md). The table is read again whenever a scan ends; if it cannot be read, the screen says so rather than showing the empty state.

**Empty state:** "No scans yet", one sentence, and a link to Locations.

**Accessibility:** each card is an article named by its location; the bar is named "Scanning Photos" and gives its value only when it is known; the controls are buttons with their action as the name.
