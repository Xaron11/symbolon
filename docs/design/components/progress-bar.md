# ProgressBar

Shows how far a task is, such as a scan.

**Anatomy:** a thin track (`color.surface.active`) with an accent fill.

**Modes:** determinate (a fraction from 0 to 1, shown in percent) and indeterminate (while the total is unknown: a moving segment, still under reduced motion only by its static position).

**Accessibility:** `role="progressbar"` with a label naming the task, `aria-valuemin` 0, `aria-valuemax` 100 and `aria-valuenow` when determinate. Out-of-range values are clamped.
