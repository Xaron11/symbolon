# Command-line usage

`symbolon` compares directories, disks and pendrives from the shell. This chapter walks through a typical session, then covers output formats, exit codes and scripting. Every command has `--help`, and `man symbolon` works where the man pages are installed. The conventions here are decided in [ADR-0011](adr/0011-cli-conventions.md).

## Installing

- **Release archives** (Linux x86-64, Windows x86-64) on the [releases page](https://github.com/Xaron11/symbolon/releases) contain the binary, man pages (`man/`), completion scripts (`completions/`), and the Android agent `symbolon-agent-aarch64-linux-musl`, which Symbolon pushes to phones to scan them.
- **Nix:** `nix run github:Xaron11/symbolon -- --help`, or add the flake's `packages.<system>.symbolon` to your system. The package installs man pages and bash, zsh and fish completions. It does not include the Android agent yet: download it from the releases page and point `SYMBOLON_AGENT` at it, or phones are read with the SHA-256 fallback.
- **From source:** `cargo build --release -p symbolon-cli` in the dev shell; the binary is `target/release/symbolon`. `just android-scanner` builds the agent (`target/aarch64-unknown-linux-musl/release-agent/symbolon-agent`); set `SYMBOLON_AGENT` to its path.

Shell completions for other setups: `symbolon completions bash|zsh|fish|elvish|powershell` prints the script, for example `symbolon completions zsh > ~/.zfunc/_symbolon`.

## The catalog

Everything `symbolon` learns goes into one SQLite database, the catalog: saved locations, their files, sizes, modification times and hashes. By default it lives at:

| Platform | Path                                                |
| -------- | --------------------------------------------------- |
| Linux    | `~/.local/share/symbolon/catalog.db`                |
| Windows  | `%LOCALAPPDATA%\symbolon\catalog.db`                |
| macOS    | `~/Library/Application Support/symbolon/catalog.db` |

`--catalog <PATH>` or the `SYMBOLON_CATALOG` environment variable selects another one. The catalog contains full paths and hashes of your files, so treat it, and the export files below, as private data. On Linux and macOS Symbolon creates them readable only by you (mode `0600`, and `0700` for directories it creates). Files and directories that already exist keep their permissions.

## A session

Suppose a laptop folder is backed up to a USB disk, and you want to know whether the backup is current.

### Save the locations

A location is a directory with a label. Every other command refers to it by label.

```console
$ symbolon location add laptop ~/Documents
LABEL   ROOT                POLICY  ALGO    FILES  SIZE  HASHED  LAST SCAN  DEVICE
laptop  /home/me/Documents  lazy    blake3  0      0 B   0       never      fs:9ee6adca-89bf-4eaf-9e85-4f0a008d8b53:/me/Documents
$ symbolon location add backup /mnt/usb/Documents --policy eager
```

The **hash policy** decides when file contents are read. `eager` hashes every file during a scan; `lazy` (the default) only records names, sizes and times, and hashes later, only the files a comparison cannot decide otherwise. Lazy is much faster for large locations; eager makes later comparisons instant. `--algo` picks the hash (`blake3` by default); only locations with the same algorithm can be compared. `--ignore <pattern>` skips paths in `.gitignore` syntax.

On Linux, Symbolon also records which **device** a location is on: the filesystem's UUID and the path inside it (`DEVICE`). This lets it recognize a pendrive mounted somewhere else next time, and keeps it from scanning an empty mount point or another device as the location ([ADR-0016](adr/0016-device-fingerprints.md)). Filesystems without a UUID, such as tmpfs and network shares, have no device ID; for those the root path is trusted as before.

`symbolon location set <label>` changes a saved location:

- `--label <new>` renames it.
- `--root <path>` points it at another directory, for example where its device is mounted now, and records the device found there.
- `--policy lazy|eager` changes when files are hashed from the next scan on.

### Scan

```console
$ symbolon scan laptop backup
LABEL   STATUS     FILES  SIZE  HASHED    REUSED  WARNINGS  TIME
laptop  completed  5      65 B  0 (0 B)   0       0         0.0 s
backup  completed  5      60 B  5 (60 B)  0       0         0.0 s
```

A progress line shows on the terminal while it runs. Rescans are incremental: files whose size and modification time did not change keep their hashes (`REUSED`) and are not read again. Ctrl-C stops a scan cleanly and keeps what was recorded. Files that cannot be read (permissions, files vanishing mid-scan) are counted as warnings; `--verbose` lists them. `scan --all` scans every saved location, and `--full` hashes everything even in lazy locations.

`symbolon location list` shows every location with its last scan.

Before reading a location, `scan` checks that its device is where it was recorded:

- **Moved device.** When the device is mounted somewhere else now, for example a pendrive plugged into another port or computer, `scan` follows it to the new place and says so: `photos: its device is now at /run/media/me/USB (was /media/usb); following it`. `diff` does the same.
- **Several matches.** When the device matches several mounted directories, `scan` asks which one to use. Without a terminal it stops with exit code 4 and lists them; pick one with `symbolon location set photos --root <path>`.
- **Another device at the root.** When another device, or an empty mount point, is at the location's root and its own device is not plugged in, scanning would record every file as gone. `scan` asks first. Without a terminal it stops with exit code 4, unless you pass `--yes` to scan what is there as the location.
- **`scan --all`.** It skips locations whose device is not connected and scans the rest.

### Compare

```console
$ symbolon diff laptop backup
CATEGORY   ROWS  FILES  SIZE
identical  1     2      26 B
different  1     2      18 B
only_in    2     2      21 B
moved      1     2      24 B
renamed    1     2      36 B

CATEGORY   PATH            laptop                   backup
only_in    2019.txt        -                        8 B
different  draft.txt       9 B                      9 B
only_in    notes-copy.txt  13 B                     -
moved      old/beach.jpg   photos/beach.jpg (12 B)  12 B
renamed    todo-list.txt   todo.txt (18 B)          18 B
```

The first table sums up every category; the second lists each difference with one column per location: the file's size, or its path there when it differs (moves and renames), or `-` where it is missing. Identical files are left out of the list; `--all` shows them too.

| Category     | Meaning                                                                   |
| ------------ | ------------------------------------------------------------------------- |
| `identical`  | Same path, same content everywhere.                                       |
| `different`  | Same path, different size or content.                                     |
| `only_in`    | Present in some locations only.                                           |
| `moved`      | Same content and file name in another folder.                             |
| `renamed`    | Same content under another name.                                          |
| `collision`  | Several files of one location map to one path (case or Unicode spelling). |
| `unverified` | Same path and size, not hashed, so equality is unknown.                   |

[Concepts](concepts.md) explains how these are decided. Before listing, `diff` hashes what it needs in lazy locations, with progress; `--no-hash` skips that and leaves such files `unverified`. A location that is not connected is reported, and its files stay unverified.

Useful options:

- `laptop:Photos` compares a folder of a location; any number of locations can be compared at once.
- `--only different,only-in`, `--ext jpg`, `--min-size`, `--max-size` and `--under <folder>` narrow the list; the summary always covers everything. `--summary` prints only the totals.
- `--case-insensitive` and `--nfc` match paths regardless of letter case or Unicode spelling, for example between Linux and Windows or macOS disks.
- `--fast 2s` treats files of equal size whose times differ by at most 2 seconds as equal without reading them, marked `(assumed)`: fast, but not proof.

### Find duplicates

```console
$ symbolon dupes laptop
GROUP  SIDE    SIZE  PATH
1      laptop  13 B  notes-copy.txt
1      laptop  13 B  notes.txt

laptop: 1 group, 2 files, 13 B could be freed
```

Groups are ordered by the space that keeping one copy would free. Lazy locations are hashed only where two files share a size. `--min-size` skips small files.

### Export and compare elsewhere

A location can be taken to another computer as a single `.symbolon` file, to compare against it there without the disk:

```sh
symbolon export backup -o backup.symbolon
# on the other computer:
symbolon import backup.symbolon
symbolon diff laptop backup
```

An imported location compares like any other but cannot be scanned there. Importing again updates it if the file holds a newer scan. When an import is ambiguous, `import` stops with exit code 4 and says which choice resolves it. That happens, for example, when the label belongs to a location scanned on this computer or to another device. The choices are `--as <label>` to import under a new label, `--replace` to replace the existing location's data, `--into <label>` to update the location that is the same device, or `--force` to accept an older scan. Replacing or updating a location scanned on this computer turns it into an imported location, which is no longer scanned there. [ADR-0005](adr/0005-export-format-and-import-conflicts.md) and [ADR-0014](adr/0014-imports-and-local-locations.md) have the full rules.

## Output for scripts

Results go to stdout; progress, warnings and errors go to stderr, so piping is safe. `--quiet` turns progress and warnings off.

`--format json` (or `--json`) and `--format csv` are stable for scripts: field names only change in a minor release with a changelog entry, and only by adding fields. Sizes are bytes, times are RFC 3339 in UTC, digests are lowercase hex, and paths are written with `/`, with bytes that are not valid UTF-8 escaped as `%XX`. The table format is for people and may change; it shows control characters in file names (which could otherwise drive the terminal) as `%XX`, while JSON and CSV keep them exact.

```console
$ symbolon diff laptop backup --json | jq -r '.rows[] | select(.category == "only_in") | .key'
2019.txt
notes-copy.txt
$ symbolon diff laptop backup --format csv --only different | cut -d, -f4,5
side,path
laptop,draft.txt
backup,draft.txt
$ symbolon dupes laptop --json | jq '.totals[].redundant_bytes'
13
```

`diff --json` returns `{sides, summary, rows}`; each row has a `category`, a `key` (its path) and `files`, one per location with `side`, `path`, `size` and `digest`. CSV has one line per file, with a `row` number grouping them.

## Exit codes

| Code | Meaning                                                                                                                          |
| ---- | -------------------------------------------------------------------------------------------------------------------------------- |
| 0    | Success. For `diff`: everything compared is identical.                                                                           |
| 1    | `diff` found differences (anything not identical, including unverified files).                                                   |
| 2    | Invalid command line.                                                                                                            |
| 3    | Error: unknown label, unreadable catalog, mixed hash algorithms, I/O failure.                                                    |
| 4    | Needs a choice: an import conflict, a location already being scanned, a missing `--yes`, a device that is ambiguous or replaced. |
| 130  | Stopped with Ctrl-C after cleaning up.                                                                                           |

So `symbolon diff laptop backup --quiet --summary >/dev/null && echo "backup is current"` works as a check in scripts.
