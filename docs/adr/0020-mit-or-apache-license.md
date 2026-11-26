# 0020. MIT OR Apache-2.0 instead of GPL-3.0-or-later

- **Status:** Accepted
- **Date:** 2026-11-26

## Context

The spec (§3) licensed Symbolon under `GPL-3.0-or-later`, with no DCO and no CLA, to be revisited before the first outside contribution (#19). Versions up to v0.7.0 were released under it.

Most of Symbolon's code is written by AI agents under the maintainer's direction. How far copyright covers such code is unsettled, which makes a strong copyleft both hard to rely on and an odd promise to make. A permissive license avoids asserting more control over the code than may exist.

Every commit so far is the maintainer's (no outside contributions, no co-authors), so the maintainer alone can relicense. Every Rust and npm dependency is already under a permissive license or MPL-2.0 (`deny.toml`, `license-check`), so nothing forces copyleft on the binaries.

## Decision

1. From the first release after v0.7.0, Symbolon is licensed under `MIT OR Apache-2.0`, at the user's option, the default of the Rust ecosystem. `LICENSE-MIT` and `LICENSE-APACHE` replace `LICENSE`; `Cargo.toml`, `ui/package.json` and the Nix package say so.
2. Contributions are accepted under the same dual license (inbound = outbound), stated in `CONTRIBUTING.md` and the README with the usual Rust wording. There is no DCO and no CLA; this settles #19.
3. The dependency policy keeps its allowlist (permissive licenses and MPL-2.0); its reason is now that the binaries must be distributable under `MIT OR Apache-2.0`. GPL dependencies stay rejected.
4. Release archives ship both license files, and release notes name the license and link to the source tag.

## Consequences

- Anyone can use, modify and redistribute Symbolon, including `symbolon-core` as a library in closed-source software, as long as they keep the license and copyright notices. Apache-2.0 adds an explicit patent grant for those who choose it.
- Releases up to v0.7.0 remain available under `GPL-3.0-or-later` to anyone who has them; that cannot be withdrawn.
- A permissive license does not settle whether AI-written code reproduces third-party code too closely; that risk is the same under any license.
- Revisit if outside contributions with other terms are proposed, or if the project wants a contributor agreement after all.
