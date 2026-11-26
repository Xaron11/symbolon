# Tooltip

A short text describing a control, shown on hover after 400 ms and immediately on keyboard focus; hidden on blur, pointer leave or Escape.

**Anatomy:** small floating label (raised surface, small type) next to the trigger.

**Rules:** tooltips repeat or explain, never hold the only copy of information; touch devices do not show them. [IconButton](icon-button.md) shows its label as the platform's native tooltip on hover; wrap it in a Tooltip where the label must also appear on keyboard focus.

**Accessibility:** the content has `role="tooltip"` and describes the trigger (`aria-describedby`).
