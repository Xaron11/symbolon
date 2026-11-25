# 0017. Android over ADB

- **Status:** Accepted
- **Date:** 2026-11-25

## Context

Spec §6 sets the goal: hash on the phone and transfer only the index. ADR-0003 settles the agent itself, a static `aarch64-unknown-linux-musl` binary run through `adb exec-out` with two hashing threads, and the rules for a `sha256sum` fallback. Several things are still open:

- how core talks to `adb`;
- what the agent and the host say to each other;
- how hashing moves to the phone, given that core's scan and comparison pipelines read file contents through `Source::open` on the host;
- how a phone location is stored, given that schema v2 only allows `root_kind = 'local'`;
- how a phone is recognized;
- where the host finds the agent binary to push.

## Decision

**Transport.** The `AndroidTransport` trait (feature `android-adb`, ADR-0006) has four operations: list devices, push a file, run a command through `exec-out` with its stdout streamed, and run a shell command that reports its exit status. `SystemAdb` spawns the system `adb`, taken from `SYMBOLON_ADB` or `PATH`.

- **Device states.** Devices come from `adb devices -l`. Each state gets a clear message: `device`, `unauthorized` ("accept the USB debugging prompt on the phone"), `offline`, `no permissions` (udev rules), `recovery` and others.
- **Authorization.** Symbolon never authorizes a device itself.

**Agent protocol.** The agent is built on core (`default-features = false`, `local`) and pushed to `/data/local/tmp/symbolon-agent-<version>`. Every run prints a `hello` line first, carrying the protocol version, the agent version, the architecture and the number of CPUs; the host refuses a protocol version it does not know. Output is one JSON object per line. Paths are written as path keys, `%`-escaped and lossless (ADR-0004), relative to the root.

- `walk <root> [--ignore <pattern>]…` streams `entry` and `warning` lines, then `done`.
- `hash <root> --algo <a> --threads <n> --requests <file>` reads requests (path, size, mtime, full or partial) from a pushed file. It streams one `digest` or `warning` line per request, then `done`. It uses core's own hashing code, so digests, partial digests and the post-hash checks are identical to local scans.

**Hashing on the phone.** A `Source` can return a remote hasher (`Source::remote_hasher`). When it does, the scan and comparison hashing pipelines send their jobs to it instead of opening files on the host. An `AdbSource` walks and hashes through the agent and never reads file contents on the host. The pushed agent and its request files are removed when the source is dropped, whether it succeeded, failed or was interrupted.

**Fallback.** The fallback is used when the agent cannot be pushed or run: no agent binary on the host, a `noexec` `/data/local/tmp`, an architecture other than `aarch64`, or SELinux. The source switches to toybox commands and says why.

- **Walking:** `find` with `stat -c`. Times are whole seconds. A name containing a newline is a warning, not an entry.
- **Hashing:** SHA-256 only, full content only, one `cat -- "$f" | sha256sum` per file under `set -o pipefail` (ADR-0003). A failed read is a warning, never a digest.
- **Other algorithms:** in a location that is not SHA-256, hashing requests become warnings that name the problem. Phones that cannot run the agent should use `--algo sha256`.

**Locations.** A phone location's root is written `adb:<serial>:<absolute path>`, for example `adb:39151FDJH000LN:/sdcard/DCIM`. With `adb::<path>`, the device is the only one connected.

- **Schema v3** adds `adb_locations (location_id, serial)`. This is additive, like `imported_locations`: such a location keeps `root_kind = 'local'`, holds the device path in `root_raw` with `posix` encoding, and reads as `LocationRoot::Android { serial, path }`.
- **Fingerprint:** `adb:<ro.serialno>:<path>`. The hardware serial stays the same when the connection serial changes (wireless ADB uses `ip:port` or mDNS names).
- **Locating** follows ADR-0016:
  - **Here:** the device with that hardware serial is connected under the stored serial.
  - **Moved:** it is connected under another serial, and the stored serial is updated.
  - **Missing:** it is not connected, or not authorized; the message names the state.

**The agent binary on the host.** The host looks in three places, in order:

1. `SYMBOLON_AGENT`;
2. `symbolon-agent-aarch64-linux-musl` next to the `symbolon` executable;
3. `../libexec/symbolon/` relative to it (Nix).

Release archives ship the agent next to the CLI. Without an agent, the host uses the fallback and says so.

**Testing without a phone.** A mock transport runs the host-built agent, or the fallback's shell commands, on a local directory. This exercises the whole protocol in CI. Real-device checks are recorded in ADR-0003 and `docs/benchmarks.md`.

## Consequences

- One hashing implementation serves local scans and phones, and phone and pendrive copies of a photo compare directly when both use BLAKE3.
- Locations scanned through the fallback can only be compared with SHA-256 locations (spec §5.3). The CLI suggests `--algo sha256` when it falls back.
- Each scan or comparison round pushes the agent once, about 1 MB, and removes it afterwards.
- Schema v3 needs a migration and a fixture test (`docs/schema.md`).
