# TreeView

Shows a folder hierarchy that can hold very large folders, for browsing a location or a comparison by path.

## Data contract

A tree reads its nodes level by level through a tree source: `rootCount()`, `roots(offset, limit)`, `childCount(node)`, `children(node, offset, limit)`, a stable `key(node)`, and `hasChildren(node)` (a folder, even before its children are read). A folder's children are read the first time it is expanded (in pages of 500), once even if expanded again meanwhile, and kept when it is collapsed, so expanding it again reads nothing. Collapsing a folder while it loads keeps it collapsed. A failed read shows an inline error.

## Live updates

A source whose nodes can change (a location being scanned) offers `subscribe(onChange)`. On a change the tree reads its roots and every open folder again beside the tree shown, then swaps the new tree in at once, so it never shows a half-read tree; folders that still exist stay open and the active row stays on the same node. Changes that arrive while it reads cause one more refresh, not one each. The tree unsubscribes when it is removed.

## Anatomy

Rows of `table.row.height`, indented by `tree.indent` per level; a chevron for folders (rotated when open), an optional icon, the name on one line with an ellipsis and in full as its tooltip, and an optional `aside` at the end in small muted text (a file's size, a folder's count, what a comparison row says). Only the visible rows exist, so a folder with tens of thousands of entries scrolls smoothly.

**States:** hovered row · active row (accent bar and selected tint while the tree has focus) · "Loading…" until the first level arrives · `empty` text ("Nothing matches the filters.") when there is nothing to show. The messages sit beside the tree, not in it.

## Keyboard

Focus the tree with Tab; then the [list keys](../keyboard.md#in-tables-and-trees) apply: `j`/`k` or ↓/↑ move; `l` or → expands a closed folder, or in an open one moves to its first child; `h` or ← collapses an open folder, or moves to the parent; `g g`/`G`, `Ctrl+D`/`Ctrl+U` and PageUp/PageDown move as in tables; Enter or `o` opens the item. Clicking a chevron toggles its folder.

## Accessibility

`role="tree"` with a name; items are `treeitem`s with `aria-level`, `aria-setsize`, `aria-posinset`, and `aria-expanded` on folders. The active item is announced through `aria-activedescendant`.
