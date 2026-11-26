# AppShell

The frame every view sits in: a sidebar of sections, a toolbar, the view itself and a status bar.

**Anatomy:** the sidebar runs down the left edge; the toolbar (`toolbar.height`) spans the rest of the top, with the section's title and, at the end, the view's actions and a button for the [command palette](command-palette.md); the view fills the remaining space and scrolls on its own, so the frame never scrolls; the status bar (`statusbar.height`) spans the bottom in the sunken surface, with activity at the start and context (such as the catalog path) in monospace at the end.

**Sidebar:** one link per section (Locations, Scans, Compare), each with an icon and label; the current one has the accent bar and the selected surface. Expanded it is `sidebar.width.expanded` wide and shows the [theme switch](theme-switch.md) at the bottom; collapsed (`sidebar.width.collapsed`) it shows icons only, with each label kept for screen readers and as a tooltip with its shortcut. The choice is remembered. Under a coarse pointer links grow to `control.height.touch`. Below 768px wide the sidebar becomes a bar of sections under the view _(planned: M10 Android)_.

**Status bar:** the activity is a link to Scans that reads "No scans running", "Scanning Photos — 1,204 files" while a scan lists files, "Scanning Photos — 42%" while it hashes, or "2 scans running". Screen readers are told only when a scan starts, changes phase or ends ("Scanning Photos", "Hashing Photos", "No scans running"), through a hidden status region: the visible counts change up to ten times a second. When a scan ends a toast says so: "Photos: scan finished", "Photos: scan cancelled", or, until dismissed, "Photos: scan failed." with the reason.

**Keyboard:** `g l`, `g s`, `g c` or `Ctrl+1`–`Ctrl+3` open a section; `Ctrl+B` collapses or expands the sidebar; `Ctrl+K` opens the command palette; `?` lists every shortcut ([keyboard](../keyboard.md)). They work while a table or tree has focus too: a focused list matches its own and the app-wide bindings together and hands the app-wide ones to the shell, so `g l` opens Locations even though `g g` is a list key.

**Accessibility:** the first Tab stop is a "Skip to content" link, shown when focused, that moves focus to the view. Landmarks: the sidebar is navigation named "Sections" (the current link has `aria-current="page"`), the toolbar a banner, the view `main`, and the status bar a footer named "Status". The toolbar title is the page's only `h1`.
