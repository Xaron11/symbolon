# CategoryChip

Shows a comparison category ([Concepts](../../concepts.md#comparison-categories)) wherever one appears: rows, filters, legends.

**Anatomy:** category icon in the category color, label in the primary text color, optional count in monospace, on the category tint with its border.

| Category   | Icon             | Color tokens            |
| ---------- | ---------------- | ----------------------- |
| Identical  | check            | `category.identical.*`  |
| Different  | not equal        | `category.different.*`  |
| Only in    | dashed circle    | `category.only-in.*`    |
| Moved      | arrow right      | `category.moved.*`      |
| Renamed    | pencil           | `category.renamed.*`    |
| Collision  | warning triangle | `category.collision.*`  |
| Unverified | question mark    | `category.unverified.*` |

**Variants:** full (icon, label, optional count) and compact (icon only).

**Accessibility:** color is never the only signal: the icon and the label always go with it. A compact chip is an image whose accessible name is the label.
