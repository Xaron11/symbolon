# Symbolon

> *symbolon* (Greek σύμβολον): a token broken in two, each party keeping a half, later matched to prove identity.

Symbolon answers one question: **do these locations match, and if not, how exactly do they differ?**

It scans two or more locations (directories, mounted disks, pendrives, Android phones over ADB), hashes file contents into a local catalog, and reports for any set of them which files are identical, missing, changed, moved or renamed, and duplicated.

**Status:** pre-alpha. Milestone M0 (scaffolding) only; nothing useful runs yet. See [`docs/start.md`](docs/start.md) for the full specification and [`docs/adr/`](docs/adr/) for design decisions.

## Layout

| Path | Purpose |
|---|---|
| `crates/symbolon-core` | All logic: scanning, catalog, comparison. No UI, no async runtime requirement. |
| `crates/symbolon-cli` | `symbolon` command-line client. |
| `crates/symbolon-app` | Tauri 2 desktop app; SvelteKit frontend in `ui/`. |
| `crates/symbolon-agent` | Static scanner binary pushed to Android devices over ADB. |

## Development

Requires [Nix](https://nixos.org) with flakes. Everything else, including the pinned Rust toolchain, comes from the dev shell.

```sh
direnv allow          # or: nix develop
just                  # list recipes
just test lint deny   # what CI runs
just app-dev          # desktop app with hot reload
just android-scanner  # cross-compile the Android agent (aarch64, static musl)
nix build             # CLI package
```

## Privacy

A catalog contains full file paths and content hashes of your data. Treat catalogs and exported `.symbolon` files as sensitive. See [`SECURITY.md`](SECURITY.md).

## License

[GPL-3.0-or-later](LICENSE). This includes `symbolon-core` when used as a library.
