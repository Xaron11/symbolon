# DropdownMenu

A list of actions opened from a button, such as a location's actions.

**Anatomy:** trigger button; floating menu (raised surface, overlay shadow) of items, each with an optional icon, a label and an optional shortcut hint; separators between groups. Destructive items use the error color.

**Keyboard:** Enter, Space or ↓ on the trigger opens the menu with the first item active; ↑/↓ move and skip disabled items; Enter or Space runs the item; Escape closes and returns focus to the trigger; typing a letter jumps to an item starting with it.

**Accessibility:** the trigger needs an accessible name (for example "Location actions"); items are `menuitem`s, disabled ones `aria-disabled`.
