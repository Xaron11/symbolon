# 0011. CLI conventions

- **Status:** Accepted
- **Date:** 2026-11-24

## Context

Spec §10 sketches the `symbolon` commands and asks for stable exit codes, `--json` for scripting, progress on stderr, results on stdout, shell completions and man pages. It leaves open where the catalog lives, which formats exist and what they promise, and which exit code means what. Every command in M3 depends on these answers, and scripts will depend on them once released.

## Decision

**Catalog location.** One catalog per user, at `<data dir>/symbolon/catalog.db`, where the data directory is the platform's local data directory (`dirs::data_local_dir`):

| Platform | Default catalog path                                      |
| -------- | --------------------------------------------------------- |
| Linux    | `$XDG_DATA_HOME/symbolon/catalog.db` (`~/.local/share/…`) |
| Windows  | `%LOCALAPPDATA%\symbolon\catalog.db`                      |
| macOS    | `~/Library/Application Support/symbolon/catalog.db`       |

`--catalog <path>` overrides it, then the `SYMBOLON_CATALOG` environment variable. The directory is created on first use. The GUI (M6) uses the same default, so both see the same locations.

**Streams.** Results go to stdout and nothing else does. Progress, warnings and errors go to stderr. Progress is drawn only when stderr is a terminal, and `--quiet` turns it and the warnings off; errors are always printed.

**Formats.** `--format table|json|csv`, with `--json` as a shorthand for `--format json`:

- `table` (the default) is for people. Its layout may change in any release.
- `json` and `csv` are for scripts. Field names and meanings only change in a minor release with a changelog entry, and only by adding fields until 1.0 says otherwise. Sizes are bytes, times are RFC 3339 UTC, digests are lowercase hex with their algorithm alongside, paths are path keys (ADR-0004: valid UTF-8, invalid bytes escaped as `%XX`).
- Each command's JSON output is one document, written incrementally so a million rows never sit in memory. CSV has a header row.
- HTML reports belong to M8.

**Exit codes.**

| Code | Meaning                                                                                          |
| ---- | ------------------------------------------------------------------------------------------------ |
| 0    | Success. For `diff`: every compared file is identical.                                           |
| 1    | `diff` found differences (any category other than identical), like `diff(1)`.                    |
| 2    | Usage error: unknown option, missing argument (clap's code).                                     |
| 3    | Runtime error: I/O, a corrupt or too-new catalog, an unknown label.                              |
| 4    | Refused until the user chooses: an import conflict (ADR-0005), a location already being scanned. |
| 130  | Interrupted by Ctrl-C after cleaning up.                                                         |

**Ctrl-C.** The first Ctrl-C cancels the running scan or hashing through core's cancellation and waits for it to stop, which records a `cancelled` scan and keeps every hash already stored. A second Ctrl-C exits at once; the catalog stays consistent because every write is a transaction, and the scan is marked `interrupted` on the next open.

**Confirmations.** A destructive command (`location remove`) asks on a terminal and requires `--yes` otherwise; it never reads stdin when stdin is not a terminal.

## Consequences

- Scripts can rely on `--json`/`--format csv` output and on the exit codes; `diff` composes with `&&` and `if` like `diff(1)`, at the cost that a script treating any non-zero code as failure must allow 1.
- The table format can keep improving without breaking anyone.
- A per-user catalog shared by the CLI and GUI makes concurrent access real, which is why one scan per location is enforced (#35).
- Revisit if a command needs a result that is neither success nor difference (for example `dupes` exiting 1 when duplicates exist); that would be a new code or a flag, not a change to these.
