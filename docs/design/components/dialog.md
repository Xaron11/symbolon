# Dialog

A modal window for a focused task, such as adding a location or confirming a removal.

**Anatomy:** scrim (`color.scrim`) over the page; panel on the raised surface with the overlay shadow, `panel.radius`; title; optional description; body; optional footer with actions aligned to the end (primary action last).

**Behaviour:** opening moves focus into the dialog, to its first text field when it has one (so typing can start at once), otherwise to its first control; Tab and Shift+Tab stay inside it; Escape closes it; closing returns focus to the control that opened it.

**Responsive:** below 768px the dialog becomes a full-screen sheet.

**Accessibility:** `role="dialog"`, named by its title and described by its description.
