# Desktop app

The desktop app does everything the CLI does, with screens for locations, scans and comparisons. It uses the same catalog as the CLI, so locations added in one appear in the other.

## Installing

Downloads are on the [releases page](https://github.com/Xaron11/symbolon/releases), next to `SHA256SUMS` (check them with `sha256sum -c SHA256SUMS`):

| System         | File                                      | How                                                                                                                               |
| -------------- | ----------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| Any Linux      | `symbolon-app-<version>-x86_64.AppImage`  | `chmod +x` it and run it. It needs no installation.                                                                               |
| Debian, Ubuntu | `symbolon-app-<version>-amd64.deb`        | `sudo apt install ./symbolon-app-<version>-amd64.deb`; Symbolon appears in the applications menu.                                 |
| Windows        | `symbolon-app-<version>-x86_64-setup.exe` | Run it; it installs for your user only. Windows may warn that the publisher is unknown, because the installer is not code-signed. |
| NixOS, Nix     | flake package `symbolon-app`              | `nix run github:Xaron11/symbolon#symbolon-app`, or add `packages.<system>.symbolon-app` to your system.                           |

The AppImage, `.deb` and Windows installer include the Android agent, so phones are scanned at full speed ([Concepts](concepts.md)). The Nix package does not yet: set `SYMBOLON_AGENT` to the agent from a release archive, or phones are read with the SHA-256 fallback.

The decisions behind these formats are in [ADR-0026](adr/0026-desktop-app-distribution.md).

## The catalog

The app opens the same catalog as the CLI: `SYMBOLON_CATALOG` if it is set, otherwise the default path listed in [Command-line usage](cli.md#the-catalog).

## Troubleshooting

- **The window stays blank or white (Linux, often with NVIDIA graphics):** start it with `WEBKIT_DISABLE_DMABUF_RENDERER=1`, for example `WEBKIT_DISABLE_DMABUF_RENDERER=1 ./symbolon-app-<version>-x86_64.AppImage`.
- **An AppImage does not start on NixOS:** use the Nix package, or `appimage-run` from nixpkgs.
