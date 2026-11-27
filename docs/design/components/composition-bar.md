# CompositionBar

The summary of a comparison: how its rows divide among the [categories](category-chip.md). It replaces a pie or doughnut chart: there are seven categories, and the small ones (collision, unverified) would be unreadable slices ([ADR-0022](../../adr/0022-design-system.md)).

**Anatomy:** one horizontal bar split into a segment per non-zero category, proportional to its count, in the category color; every segment is at least 4px wide, so a single collision among 100,000 rows still shows. A legend table (category chip, entries, share with one decimal, size) carries the same numbers; it is read by assistive technology and shown only with `showTable`, since the Compare screen's tiles already show them.

**Behaviour:** activating a segment selects its category (the Compare view filters to it).

**Keyboard:** Tab reaches the bar once (on the first segment); ←/→ move between segments, Home/End to the ends; Enter or Space activates.

**Accessibility:** the bar is a named group of buttons, each named "Category: count (share%)"; the legend table carries the same numbers as text. Under a coarse pointer the bar grows to the touch height.
