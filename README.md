# Symbolon

> _symbolon_ (Greek σύμβολον): a token broken in two, each party keeping a half, later matched to prove identity.

Symbolon answers one question: **do these locations match, and if not, how exactly do they differ?**

It scans two or more locations (directories, mounted disks, pendrives, Android phones over ADB), hashes file contents into a local catalog, and reports for any set of them which files are identical, missing, changed, moved or renamed, and duplicated.

**Status:** pre-alpha. The core library scans local locations into the catalog (M1) and compares two or more of them (M2): identical, different, only-in, moved, renamed and duplicate files, hashing lazily only what decides a result. The command line (M3) comes next; there is no user-facing tool yet. See [`docs/start.md`](docs/start.md) for the full specification and [`docs/adr/`](docs/adr/) for design decisions.

## Layout

| Path                    | Purpose                                                                        |
| ----------------------- | ------------------------------------------------------------------------------ |
| `crates/symbolon-core`  | All logic: scanning, catalog, comparison. No UI, no async runtime requirement. |
| `crates/symbolon-cli`   | `symbolon` command-line client.                                                |
| `crates/symbolon-app`   | Tauri 2 desktop app; SvelteKit frontend in `ui/`.                              |
| `crates/symbolon-agent` | Static scanner binary pushed to Android devices over ADB.                      |

## Development

Requires [Nix](https://nixos.org) with flakes. Everything else, including the pinned Rust toolchain and the git hooks, comes from the dev shell.

```sh
direnv allow          # or: nix develop
just                  # list recipes
just fmt lint test    # format, lint and test everything (as CI does)
just app-dev          # desktop app with hot reload
just android-scanner  # cross-compile the Android agent (aarch64, static musl)
nix build             # CLI package
```

Documentation: the book in [`docs/`](docs/introduction.md) (`just docs-serve`), with concepts, architecture, editor/LSP setup and the API reference. Workflow, code rules and conventions: [`AGENTS.md`](AGENTS.md). Testing: [`docs/testing.md`](docs/testing.md). Releases and CI: [ADR-0007](docs/adr/0007-development-workflow.md).

## Privacy

A catalog contains full file paths and content hashes of your data. Treat catalogs and exported `.symbolon` files as sensitive. See [`SECURITY.md`](SECURITY.md).

## License

[GPL-3.0-or-later](LICENSE). This includes `symbolon-core` when used as a library.
