# Getting started

## Prerequisites

- [Nix](https://nixos.org/download) with flakes enabled (`experimental-features = nix-command flakes`). It works on NixOS and on other Linux distributions.
- Optional but recommended: [direnv](https://direnv.net) with [nix-direnv](https://github.com/nix-community/nix-direnv), so the environment loads when you enter the directory.

Everything else comes from the dev shell defined in `flake.nix`: the pinned Rust toolchain (`rust-toolchain.toml`), Node and pnpm, cargo tools, formatters, linters, language servers, docs tools, `adb` and the Tauri system libraries.

## First run

```sh
git clone git@github.com:Xaron11/symbolon.git
cd symbolon
direnv allow      # or: nix develop
just              # list all recipes
just test         # Rust tests
just test-ui      # frontend tests (installs node_modules on first run)
```

Entering the shell installs the git hooks (pre-commit, commit-msg, pre-push); see [ADR-0007](../adr/0007-development-workflow.md).

## Everyday commands

| Command                           | What it does                                                                            |
| --------------------------------- | --------------------------------------------------------------------------------------- |
| `just fmt`                        | Format every file (Rust, Nix, TOML, shell, justfile, Markdown, YAML, JSON, TS, Svelte). |
| `just check`                      | Everything CI checks: lints, tests, frontend build, licenses, docs.                     |
| `just lint`                       | Hooks, clippy, rustdoc, unused deps, frontend lints.                                    |
| `just lint-rust` / `just lint-ui` | Only the Rust lints / only the frontend lints.                                          |
| `just test` / `just test-ui`      | Rust tests (nextest + doctests) / frontend tests (vitest).                              |
| `just coverage --html`            | Coverage report in `target/llvm-cov/html`.                                              |
| `just docs` / `just docs-serve`   | Build this book and the API reference / serve both with live reload.                    |
| `just docs-check`                 | Build the book and the API reference and check every link and anchor.                   |
| `just deny` / `just deny-ui`      | License and advisory checks for Rust / npm dependencies.                                |
| `just app-dev`                    | Run the desktop app with hot reload.                                                    |
| `just build-agent check-agent`    | Cross-compile the Android agent and check it is static.                                 |
| `just nix-check`                  | Flake checks and the Nix packages of the CLI and the app (slow).                        |
| `nix build`                       | Build the CLI package.                                                                  |

## Making a change

1. Pick or open an issue; branch from `main` as `<type>/<short-name>`.
2. Make the change with tests and docs (see [Testing](../testing.md) and [Writing documentation](documentation.md)).
3. Commit with a Conventional Commit message; hooks format and lint staged files.
4. Push (the pre-push hook runs the Rust and frontend checks for the parts the push touches) and open a PR. CI must be green; the PR is squash-merged with its title as the commit message.

The full working agreement (code rules, commit types, security) is `AGENTS.md` at the repository root.

## Troubleshooting

- **The app window is blank or crashes on a non-NixOS distribution.** The Nix-built WebKitGTK needs a working GL stack. Try `WEBKIT_DISABLE_DMABUF_RENDERER=1 just app-dev`.
- **`adb devices` lists nothing.** Check that the phone shows up in `lsusb` (if not: the cable is charge-only, or USB mode is "charging only"), that your user may access USB devices (udev rules, `plugdev`/`adbusers` group), and that you accepted the debugging prompt on the unlocked phone.
- **Hooks did not run.** Enter the dev shell once; it (re)installs them. `pre-commit run --all-files` runs them by hand.
- **A formatter disagrees with your editor.** `treefmt <file>` is the reference; configure your editor to call it (see [Editor and LSP setup](editor.md)).
