# 0006. Feature gates in `symbolon-core`

- **Status:** Accepted
- **Date:** 2026-11-24

## Context

A future mobile app (UniFFI/JNI, Tauri mobile or `flutter_rust_bridge`, spec §6) and the on-device agent should reuse the pure logic without pulling in desktop-only parts. Open question §15.4.

## Decision

Always compiled (pure, portable): `hash`, `pathkey`, `catalog` (with `rusqlite` bundled), `compare`, `plan`, `Query`, the `Scanner` event types and the `Source` trait.

Cargo features:

| Feature       | Default | Contents                                                                                              |
| ------------- | ------- | ----------------------------------------------------------------------------------------------------- |
| `local`       | yes     | `LocalSource`, walker, local hash pipeline (`rayon`, `ignore`-style patterns).                        |
| `android-adb` | yes     | `transport::android`: `AndroidTransport`, `SystemAdb` (spawns `adb`), `AdbSource`.                    |
| `identity`    | yes     | Per-OS volume/device fingerprinting (Linux `/dev/disk/by-uuid`, statfs; Windows volume serial later). |
| `exec`        | no      | Plan executor with journal and verification (M7).                                                     |
| `report-html` | no      | HTML report renderer (M8).                                                                            |

The agent builds core with `default-features = false, features = ["local"]`. A mobile consumer would use `local` plus its own `Source`/transport. Platform-specific code inside a feature sits behind `cfg(target_os)` modules, never at the API surface.

## Consequences

- CI should build core with `--no-default-features` and with each feature alone, to catch accidental coupling (add when the features exist, M1+).
- Tests for the ADB layer use a mock `AndroidTransport` and do not need the `android-adb` feature's process spawning.
