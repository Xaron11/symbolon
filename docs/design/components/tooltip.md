# Tooltip

A short text describing a control, shown on hover after 400 ms and immediately on keyboard focus; hidden on blur, pointer leave or Escape.

**Anatomy:** small floating label (raised surface, small type) next to the trigger.

**Rules:** tooltips repeat or explain, never hold the only copy of information; touch devices do not show them. [IconButton](icon-button.md) uses its label as the tooltip.

**Accessibility:** the content has `role="tooltip"` and describes the trigger (`aria-describedby`).
