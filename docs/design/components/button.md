# Button

A text button for actions.

**Anatomy:** optional busy indicator, label.

**Variants:** `primary` (the one main action of a view: accent fill, on-accent text), `secondary` (default: base surface, strong border), `ghost` (no fill or border until hovered), `danger` (destructive actions: error fill).

**Sizes:** `md` uses `control.height.md`, `sm` uses `control.height.sm`; under a coarse pointer the height grows to `control.height.touch`. Corners use `control.radius`, the label uses the medium weight.

**States** (highest priority first): disabled (`opacity.disabled`, not-allowed cursor, no events) · loading (a busy indicator before the label, `aria-busy`; the button keeps keyboard focus and ignores activation until done) · pressed (`color.surface.active` and a muted border, since the pressed and hover fills are equal) · focus (focus ring) · hover (`color.surface.hover`).

**Keyboard:** Tab focuses; Enter or Space activates.

**Accessibility:** a native button; the label is the accessible name. Submit actions stay enabled until clicked and validate on submit, rather than being disabled until a form is valid.
