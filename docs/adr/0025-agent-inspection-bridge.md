# 0025. A debug-only bridge for agents to see the app

- **Status:** Accepted
- **Date:** 2026-11-27

## Context

Unit and component tests run the interface in jsdom, which has no layout, so they cannot catch what people see first: cards of uneven height, labels that wrap, colors that clash, controls that do not look clickable. Coding agents working on the app could not see its window either. [tauri-plugin-mcp-bridge](https://github.com/hypothesi/mcp-server-tauri) (MIT) lets the matching MCP server take screenshots of a running Tauri app, read its DOM and accessibility tree, click and type, read console logs and watch IPC. It does this through an unauthenticated WebSocket, which by default listens on every interface.

## Decision

- `symbolon-app` gets an opt-in `mcp-bridge` Cargo feature. It is off by default, and a build with it but without debug assertions fails to compile, so release binaries never contain the plugin.
- With the feature on, the plugin listens on 127.0.0.1 only (`Config::localhost_only()`), and its `mcp-bridge:default` permission is added as a capability at startup. It is not in `capabilities/default.json`, because a capability that names a plugin which is not compiled in fails the build.
- `just app-dev-mcp` runs the dev app with the feature and with `withGlobalTauri` turned on through `--config`. The MCP server needs `withGlobalTauri` to watch IPC, and the shipped configuration stays unchanged.
- The crate is pinned exactly (`=0.13.2`). It follows Tauri's minor version, so it moves together with `tauri`. The MCP server on the developer's machine should use the same version.

## Consequences

- Agents can check screens in the real WebKitGTK window, with real scans, before claiming a UI change works.
- While `just app-dev-mcp` runs, any local process can drive the app and run JavaScript in it. Use it only on your own machine, and not while handling sensitive catalogs.
- Upgrading Tauri's minor version needs a matching plugin release. Without one, drop the feature until a release exists.
