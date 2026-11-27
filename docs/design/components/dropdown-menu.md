# DropdownMenu

A list of actions opened from a button, such as a location's actions.

**Anatomy:** trigger button (`size` `md`, or `sm` to match small buttons in table rows); the menu lines up with the trigger's right edge by default (`align` `end`), so menus at the right of a row or page stay inside the window; floating menu (raised surface, overlay shadow) of items, each with an optional icon, a label and an optional shortcut hint; separators between groups. Destructive items use the error color.

**Highlight:** the active item has an accent bar on its leading edge; a hover tint is added where it shows.

**Keyboard:** Enter, Space or ↓ on the trigger opens the menu with the first item active; ↑/↓ move and skip disabled items; Enter or Space runs the item; Escape closes and returns focus to the trigger; typing a letter jumps to an item starting with it.

**Accessibility:** the trigger needs an accessible name (for example "Location actions"); items are `menuitem`s, disabled ones `aria-disabled`.
