# 0001. Stack and repository layout

- **Status:** Accepted
- **Date:** 2026-11-24

## Context

`docs/start.md` §3 and §4 fix the stack (Rust, Tauri 2 + SvelteKit, SQLite via `rusqlite` bundled, BLAKE3) and a three-crate workspace. M0 had to turn this into a buildable skeleton and settle the details the spec leaves open.

## Decision

**Workspace.** Cargo workspace (resolver 3, edition 2024) with four crates:

| Crate            | Kind                                           | Depends on           |
| ---------------- | ---------------------------------------------- | -------------------- |
| `symbolon-core`  | lib                                            | (none of the others) |
| `symbolon-cli`   | bin `symbolon`                                 | core                 |
| `symbolon-app`   | Tauri 2 bin + lib (`run()`), frontend in `ui/` | core                 |
| `symbolon-agent` | bin, static aarch64 binary pushed to Android   | core (from M5)       |

`symbolon-agent` is an addition to the spec's layout. The on-device scanner is a separate binary with a different build target, profile (`release-agent`: fat LTO, `panic = "abort"`, stripped) and global allocator (`mimalloc`, see §6 of the spec). Keeping it out of the CLI keeps the pushed binary small and free of CLI-only dependencies. It must stay a thin wrapper over `symbolon-core`.

`default-members` excludes `symbolon-app`, so `cargo build` and `cargo test` without `--workspace` do not need webkitgtk. CI and `just` use `--workspace` on Linux.

**Toolchain.** `rust-toolchain.toml` pins an exact stable release (1.100.0 at bootstrap) and the `aarch64-unknown-linux-musl` target. `rust-version` in the workspace matches it. Bump both together.

**Lints.** Superseded by ADR-0007 (workspace lints, hooks, CI).

**Frontend.** Svelte 5 (runes) + SvelteKit 3, scaffolded with the official `sv` CLI, TypeScript strict, ESLint + Prettier. `@sveltejs/adapter-static` with `fallback: "index.html"`, `ssr = false`, `prerender = false`: the app is a pure SPA served by Tauri, so dynamic routes need no prerender entries. No component framework. Dark mode follows `prefers-color-scheme`.

**Tauri CLI.** The devShell provides `cargo-tauri` from nixpkgs instead of the `@tauri-apps/cli` npm package. The npm package loads a prebuilt native module that links host libraries (`liblzma.so.5`) not present inside the Nix shell and not present at all on NixOS. `tauri.conf.json` lives in `crates/symbolon-app` and runs `pnpm` in `ui/` via `beforeDevCommand`/`beforeBuildCommand` with `cwd: "ui"`.

**Identifiers.** Tauri bundle identifier `dev.xaron11.symbolon`.

**Changelog.** Generated from Conventional Commits with `git-cliff` (`cliff.toml`, Keep a Changelog layout, `just changelog`).

## Consequences

- A debug `cargo build -p symbolon-app` uses the dev server URL and does not need the built frontend; release builds go through `cargo tauri build`, which builds the frontend first.
- The gtk-rs 0.18 stack under Tauri's Linux runtime pulls `proc-macro-error` (RUSTSEC-2024-0370, unmaintained). It is ignored in `deny.toml` with that reason; revisit when Tauri moves to GTK 4 or newer gtk-rs.
- The crate name `symbolon` was reported available on crates.io; re-check before the first publish (all crates have `publish = false` for now).
