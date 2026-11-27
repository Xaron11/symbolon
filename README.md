# Symbolon

> _symbolon_ (Greek σύμβολον): a token broken in two, each party keeping a half, later matched to prove identity.

Symbolon answers one question: **do these locations match, and if not, how exactly do they differ?**

It scans two or more locations (directories, mounted disks, pendrives, Android phones over ADB), hashes file contents into a local catalog, and reports for any set of them which files are identical, missing, changed, moved or renamed, and duplicated.

**Status:** pre-alpha. The `symbolon` command line scans local directories and disks into a catalog, compares two or more of them (identical, different, only-in, moved, renamed and duplicate files, hashing only what decides a result), and exports a location to compare it on another computer. It also scans Android phones over ADB, and the [desktop app](docs/desktop-app.md) does all of this with screens (Linux AppImage and `.deb`, Windows installer, Nix package). See [`docs/start.md`](docs/start.md) for the full specification and [`docs/adr/`](docs/adr/) for design decisions.

## Quick start

```sh
symbolon location add laptop ~/Documents
symbolon location add backup /mnt/usb/Documents
symbolon scan laptop backup
symbolon diff laptop backup        # exit code 1 if anything differs
symbolon dupes laptop
```

The [command-line chapter](docs/cli.md) covers installing, output formats for scripts (`--json`, `--format csv`) and exit codes.

The documentation, with concepts, the desktop app, the architecture and the API reference, is published at <https://xaron11.github.io/symbolon/>.

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
just fmt check        # format, then everything CI checks
just app-dev          # desktop app with hot reload
just android-scanner  # cross-compile the Android agent (aarch64, static musl)
nix build             # CLI package
```

Documentation: the book in [`docs/`](docs/introduction.md) (`just docs-serve`), with concepts, architecture, editor/LSP setup and the API reference. Workflow, code rules and conventions: [`AGENTS.md`](AGENTS.md). Testing: [`docs/testing.md`](docs/testing.md). Releases and CI: [ADR-0007](docs/adr/0007-development-workflow.md).

## Privacy

A catalog contains full file paths and content hashes of your data. Treat catalogs and exported `.symbolon` files as sensitive. See [`SECURITY.md`](SECURITY.md).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT), at your option. Releases up to v0.7.0 were published under GPL-3.0-or-later.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Symbolon by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
