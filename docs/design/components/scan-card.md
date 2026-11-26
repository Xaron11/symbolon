# Scans screen

Running scans and the history of every scan (`/scans`).

**Running scans** are cards, `scan.card.min-width` wide at least and wrapping as the window narrows. Each card has the location's label (a link to its page, where the catalog can be browsed while the scan runs), the phase ("Starting", "Listing files", "Hashing" or "Paused") and, while hashing, the percentage. Below that is a [progress bar](progress-bar.md), indeterminate while files are listed and by bytes while hashing. Then the facts in monospace: files, size, rate, elapsed time and, while hashing, the time left (estimated from the hashing rate). Last come **Pause** (then **Resume**) and **Cancel**. A scan that ends leaves the cards and a toast says how it ended.

**Recent scans** are a [table](data-table.md) of every location's scans, newest first: location, started, outcome (Complete, Cancelled, Failed, Partial when the app stopped during the scan), files and size. Enter or a click opens the location. The table is read again whenever a scan ends.

**Empty state:** "No scans yet", one sentence, and a link to Locations.

**Accessibility:** each card is an article named by its location; the bar is named "Scanning Photos" and gives its value only when it is known; the controls are buttons with their action as the name.
