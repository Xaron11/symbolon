# 0012. One scan per location: an OS file lock

- **Status:** Accepted
- **Date:** 2026-11-24

## Context

The CLI and the GUI share one catalog (ADR-0011), so two scans of the same location can start at once: two terminals, or a terminal and the GUI. Without coordination the second scan marks the first `interrupted`, both do the work, and their counters are wrong (#35). Scans of different locations must still run in parallel, and a crashed scan must never block the next one.

Options considered:

- **A `scans` row with a heartbeat:** the running scan updates a timestamp, and a scan whose heartbeat is older than a timeout counts as dead. It needs a timeout that is both short (a crash blocks scanning until it expires) and long (a stalled disk must not look dead), and extra writes during every scan.
- **An OS file lock:** held by the scanning process, released by the OS when the file is closed or the process dies. No timeout and no writes; works across processes and within one.

## Decision

`Scanner::start` takes an exclusive, non-blocking lock (`File::try_lock`: `flock` on Unix, `LockFileEx` on Windows) on `<catalog>-locks/<location id>.lock` before starting the scan thread, and the thread holds it until the scan ends. If the lock is taken, `start` fails with `ScanError::AlreadyScanning`, carrying the label and, when recorded, the running scan's start time. Lock files are never deleted, so no process can lock a file another has just unlinked.

Holding the lock also means a `running` scan row found by `begin` always belongs to a dead scan, so marking it `interrupted` is safe.

## Consequences

- A second scan is refused immediately; the CLI exits with code 4 (ADR-0011) and the GUI can offer to wait or cancel.
- Comparisons hashing on demand do not take the lock: their writes are guarded by size and modification time (ADR-0010), so they stay correct alongside a scan.
- Locks on network filesystems may be unreliable; catalogs are expected on local disks. Revisit if catalogs on shared storage become a use case.
