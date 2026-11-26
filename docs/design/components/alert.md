# Alert

An inline message about the current view, such as a failed scan.

**Tones:** `error`, `warning`, `info`, `success`, each with its icon and `status.*` colors; the title takes the status color, the body the secondary text color.

**Accessibility:** errors use `role="alert"` (announced immediately); other tones use `role="status"`. For messages that are not tied to a view, use a toast.
