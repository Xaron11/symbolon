# Toast

A brief message not tied to the current view, such as "Scan finished".

**Anatomy:** raised panel at the bottom right, above the status bar, with a left stripe in the status color, the message and a Dismiss button.

**Timing:** information, success and warnings disappear after 5 seconds; errors stay until dismissed, so a failure is never missed.

**Accessibility:** two live regions: errors are announced assertively, everything else politely. Each toast has a labelled Dismiss button.
