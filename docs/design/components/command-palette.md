# CommandPalette

Runs any command by name: navigation, theme, and later the actions of the current view. Opened with `Ctrl+K` from anywhere.

**Anatomy:** a floating panel near the top of the window over the scrim: a search field, then commands grouped under muted headings, each with its label and, if bound, its shortcut. The active command has an accent bar and a hover tint.

**Behaviour:** typing filters the commands; the first match is active, so Enter runs it and closes the palette. ↑/↓ move (wrapping at the ends); Escape closes. When nothing matches, the list says "No matching commands".

**Keyboard:** while the palette is open, single-letter shortcuts are typed as text (the panel is a text-entry area for the [keymap](../keyboard.md)).

**Accessibility:** a dialog named "Command palette"; the field is a combobox controlling the list of options.
