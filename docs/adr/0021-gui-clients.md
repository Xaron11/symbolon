# 0021. One Svelte codebase for every GUI client

- **Status:** Accepted
- **Date:** 2026-11-26

## Context

M6 builds the desktop GUI with Tauri 2 and SvelteKit (spec §9). Other clients are likely later:

- an Android app, either as a remote UI for a Symbolon running on a computer or as a front end for the on-phone agent ([ADR-0017](0017-android-over-adb.md));
- a web UI served from a computer or a home server.

The spec (§6) left the mobile route open: UniFFI with a native Kotlin UI, Tauri 2 mobile, or Flutter. Each route other than Tauri would mean a second UI codebase and a second design implementation.

Tauri 2 builds the same SvelteKit frontend for Linux, Windows, macOS and Android. A browser cannot run `symbolon-core` itself (no arbitrary filesystem access, no SQLite with threads), but it can show the same UI if a local Rust process serves the data.

## Decision

1. Every Symbolon GUI client is the same SvelteKit codebase in `crates/symbolon-app/ui/`:
   - desktop (Linux first, then Windows and macOS) and Android are built with Tauri 2;
   - the web UI is the same build served by a local Rust server, added when needed.
2. Screens never call Tauri directly. They talk to the backend through one frontend API layer with one implementation per transport: Tauri commands and channels now, HTTP and WebSocket for the web UI later. Data components take data sources that this layer implements.
3. The interface is designed once, platform-neutral, in the design system ([ADR-0022](0022-design-system.md)): tokens, component specs and the keymap. Layouts are responsive (desktop, tablet, phone) and touch-ready, but M6 builds and tests the Linux desktop only.
4. `symbolon-core` stays free of UI and transport concerns ([ADR-0006](0006-core-feature-gates.md)).

## Consequences

- One UI to build, test and keep consistent across platforms.
- The API layer is a real boundary: anything the UI needs must exist as a command or event in the bridge, which keeps business logic out of the frontend.
- Android depends on Tauri's mobile support, which is younger than its desktop support; reading all of a phone's storage from the phone also needs Android's "All files access" permission, a product question for when the Android client is planned.
- A native Kotlin or Flutter client remains possible later from the same core and design system, but is no longer the plan.
- Revisit if Tauri mobile proves unworkable for the Android client, or if a client needs capabilities a webview cannot provide.
