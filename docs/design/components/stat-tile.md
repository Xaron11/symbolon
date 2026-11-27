# StatTile

A single headline number, such as the count of different files.

**Anatomy:** small secondary label; value in monospace at heading size; optional monospace detail (for example a size) in muted text; a leading accent edge in the category color, or the strong border color without a category. Label and detail stay on one line (cut with an ellipsis, the label in full as its tooltip), so tiles in a row keep one height; a row of tiles wraps below `stat.tile.min-width` each.

**Accessibility:** plain text in reading order: label, value, detail. Color only marks the tile; the label names it.
