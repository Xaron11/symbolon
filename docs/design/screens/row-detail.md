# Row details

A side panel with what each side holds for one comparison row.

**Anatomy:** a panel like the others (base surface, panel border), as tall as the list beside it. At the top the category chip and a close button; the row's path in monospace as its heading; what the row says ("Only in Backup", "Moved: Photos → Backup"); a note when equality is only assumed (fast mode: "Equal by size and time only; not verified."). Then a section per side of the comparison, in order, headed by its label: Path (within the location), Size (or the kind for folders and links), Modified and Content (Not hashed, Partly hashed, Verified, with the first 12 hex digits of the digest) as label–value pairs, or "Not on Backup" for a side without the file.

**Keyboard:** Escape closes it, as does the close button.

**Accessibility:** a complementary landmark named after the row's path; the per-side facts are a description list.
