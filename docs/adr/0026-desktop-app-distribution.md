# 0026. Shipping the desktop app

- **Status:** Accepted
- **Date:** 2026-11-27

## Context

Releases shipped the CLI and the Android agent only ([ADR-0007](0007-development-workflow.md), [ADR-0017](0017-android-over-adb.md)). The desktop app (M6) is ready for people who do not use a terminal. It needs packages that install a menu entry, find the system's webview, and carry the Android agent so phones are scanned at full speed. The developer's own machines run NixOS and Arch, where an AppImage needs `appimage-run` (NixOS) and a `.deb` does not install.

## Decision

- Each release adds three files, built by `.github/workflows/app-bundles.yml` from the verified tag commit, with the same checksums and build attestations as the CLI:
  - `symbolon-app-<tag>-x86_64.AppImage`: one file for any Linux distribution, built on the oldest supported Ubuntu (22.04) so it runs on older systems;
  - `symbolon-app-<tag>-amd64.deb`: Debian and Ubuntu, using the system's WebKitGTK;
  - `symbolon-app-<tag>-x86_64-setup.exe`: an NSIS installer for Windows that installs for the current user and fetches WebView2 when it is missing.
- The packages carry the agent as a Tauri resource (`tauri.release.conf.json`, added only for releases so development builds do not need it). The app looks for the agent in `SYMBOLON_AGENT`, then next to its executable, then in its resource directory (`/usr/lib/Symbolon` on Linux; next to the executable on Windows).
- The flake has a `symbolon-app` package (nixpkgs' Tauri and pnpm hooks, the pinned toolchain), built by `nix flake check` in CI whenever its inputs change. Like the CLI package, it does not include the agent yet.
- The same workflow runs on pull requests that change the packaging, without the agent, so a broken bundle shows before a release.
- The workflow uses Tauri's prebuilt `cargo-tauri` release binary, the dev shell's version, checked against a SHA-256 pinned in the workflow. Compiling it took 6 to 10 minutes per job, and release builds use no caches. Tauri publishes no checksums or attestations for these binaries, so the first hash was trusted on download (2026-11-27).
- Not shipped: `.rpm`, MSI, and macOS, which needs paid signing and notarization to open without warnings. The Windows installer is unsigned, and there is no auto-update.

## Consequences

- People on common Linux distributions and Windows can install the app without Rust or Node; NixOS and Nix users get it from the flake.
- Windows SmartScreen warns about an unknown publisher until the installer is code-signed.
- Updating the Tauri CLI means changing its version in `app-bundles.yml` and both hashes along with the dev shell's.
- The Nix package pins the frontend's dependencies by hash: changing `pnpm-lock.yaml` means updating `pnpmDeps.hash` in `flake.nix` (CI's nix job fails with the new hash until then).
- On some NVIDIA setups WebKitGTK shows a blank window; the documented workaround is `WEBKIT_DISABLE_DMABUF_RENDERER=1`. Setting it in the app would need `unsafe` (`std::env::set_var`), so it stays a documented switch unless reports show it is needed widely.
