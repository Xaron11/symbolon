# 0002. Nix flake, toolchain and Android cross-compilation

- **Status:** Accepted
- **Date:** 2026-11-24

## Context

The spec (§3, §6, §12, open question §15.1) asks for a flake with a devShell and a CLI package, and a static `aarch64-unknown-linux-musl` scanner binary built without the Android NDK. The agent has C dependencies: `mimalloc` (C library) and `blake3`, which compiles its NEON implementation from C on aarch64 by default. Cross-compiling therefore needs an aarch64 musl C compiler, not just Rust's self-contained linker. The development machines run NixOS and Arch (with Nix), and CI runs on GitHub-hosted Ubuntu and Windows.

## Decision

- **Toolchain source:** `oxalica/rust-overlay`. The flake reads `rust-toolchain.toml` (`builtins.fromTOML`) and builds the same channel, components and targets, adding `rust-src` and `rust-analyzer` for the devShell. One file pins the toolchain for Nix, rustup and CI.
- **Package:** `crane` builds `packages.default` (`symbolon`, the CLI) with `-p symbolon-cli --locked`, with dependency artifacts cached separately (built with `cargo build` only, no test or bench targets). Tests are not rerun inside the Nix build; CI runs them via nextest.
- **Cross-compilation:** `cargo-zigbuild` with `zig` as C compiler and linker for `aarch64-unknown-linux-musl` (`just build-agent`). Zig ships musl headers and libc for every target, works the same inside and outside Nix, and avoids building a cross GCC from `pkgsCross`. The Android NDK is not needed.
- **Page size:** the agent must run on devices with 16 KiB pages (newer Pixels, Android 15+ 16 KB mode). The zig/lld output has `LOAD` segments aligned to 64 KiB (`readelf -lW`), which satisfies 4, 16 and 64 KiB page sizes. Verify this when the toolchain changes.
- **Tauri system libraries** (webkitgtk 4.1, gtk3, libsoup 3, librsvg, …) and `gsettings` schemas are in the devShell. On Arch with X11 the Nix-built webkitgtk renders without extra GL setup (verified at bootstrap).
- **CI** installs the toolchain with `rustup toolchain install` (reads `rust-toolchain.toml`) and system libraries with apt; a separate job runs `nix flake check`, `nix build` and the agent cross-build inside the devShell.

## Consequences

- `nix develop` gives the same Rust version that CI uses through rustup.
- If zig's musl or linker causes issues (for example with a future C dependency), the fallback is `pkgsCross.aarch64-multiplatform-musl` or the NDK (`aarch64-linux-android`); record that switch in a new ADR.
- Whether the musl binary actually runs on devices is covered by ADR-0003.
