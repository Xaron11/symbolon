# Scan page

One recorded scan (`/scans/<id>`), opened from Scans, a location's scan history or its Last scan.

**Anatomy:** a [breadcrumb](../components/breadcrumb.md) ("Scans › Photos, 27 Nov 2026, 10:23"); the location's label as the heading with the outcome as a [badge](../components/badge.md) and, for the location's newest ended scan, "Latest scan"; "Started 27 Nov 2026, 10:23, took 3 min 20 s"; **Open Photos** (a link styled as a button) and **Scan again**. Below, four [stat tiles](../components/stat-tile.md): Files, Size, Took, Warnings. A failed scan shows its error in an alert; an older one says "A newer scan of Photos exists.". Then its warnings as a [table](../components/data-table.md) (path, kind, message), or "This scan recorded no warnings.".

**Behaviour:** Scan again starts a scan of the location and opens Scans; a location already being scanned opens Scans too. Scan again is disabled while the location is being scanned.

**States:** loading (spinner); an unknown id or one that is not a number says "This scan no longer exists." with a link back to Scans.
