# Input

A single-line text field.

**Anatomy:** visible label above the field, the field (sunken surface, strong border), optional error message below.

**Types:** `text`, `search`.

**States:** focus (focus-colored border and ring) · invalid (error-colored border, `aria-invalid`, the message linked with `aria-describedby`) · disabled.

**Sizes:** `control.height.md`; `control.height.touch` under a coarse pointer.

**Keyboard:** standard text editing. Single-letter shortcuts never act while typing.

**Accessibility:** the label is always visible and associated with the field; a placeholder is only a hint, never the label. After a failed submit, typed values are kept and only the fields that need changes are marked.
