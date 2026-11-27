# Working on the UI

The desktop app's interface lives in `crates/symbolon-app/ui` (SvelteKit, Svelte 5, TypeScript). It implements the [design system](../design/index.md); read that first.

## Running it

| Task                              | Command (in `crates/symbolon-app/ui` unless noted)    |
| --------------------------------- | ----------------------------------------------------- |
| Desktop app with hot reload       | `just app-dev` (repository root)                      |
| Same, drivable by agents          | `just app-dev-mcp` (repository root; see below)       |
| Interface alone in a browser      | `pnpm dev`                                            |
| Rebuild tokens after editing JSON | `pnpm tokens` (`dev`, `build`, `lint`, `test` run it) |
| Regenerate the keyboard page      | `pnpm keymap-doc`                                     |
| Regenerate the command bindings   | `just bindings` (repository root)                     |
| Lints and type checks             | `just lint-ui` (repository root)                      |
| Tests                             | `just test-ui` (repository root)                      |

ESLint caches its results in `node_modules/.cache/eslint/`, so a lint after a small change takes seconds instead of half a minute. A cached file is not linted again when only a type it imports changes; CI lints without a cache and catches that. Delete the directory to lint everything locally.

## Letting an agent see the app

`just app-dev-mcp` starts the dev app with the [MCP bridge](../adr/0025-agent-inspection-bridge.md). An agent with the Tauri MCP server (`@hypothesi/tauri-mcp-server`, the same version as the `tauri-plugin-mcp-bridge` crate) can then take screenshots, read the accessibility tree, click, type and read console logs in the real window. For Claude Code, register the server with `claude mcp add tauri -- npx -y @hypothesi/tauri-mcp-server@0.13.2`, then restart it. The bridge listens on 127.0.0.1 only, but any local process can drive the app while it runs.

## The gallery

Development builds serve `/gallery`: every component in both themes side by side, including a 100,000-row table and a lazy tree. Use it to check a change in both themes and with long values before opening a PR. Release builds answer 404 there.

## Talking to the backend

Screens use the API layer in `src/lib/api/` ([ADR-0024](../adr/0024-app-core-bridge.md)): `backend()` returns the `Backend` an ancestor provided, and `comparisonRows`, `comparisonTree` and `catalogTree` turn it into sources for tables and trees. Import it as `#lib/api/index.ts`; ESLint rejects imports of `@tauri-apps/*` and of the generated `bindings.ts` anywhere else.

To add a command:

1. Write the `AppState` (or registry) method in `crates/symbolon-app/src/` with its tests, using plain Rust types and the `Sink` trait for streams.
2. Add a one-line `#[tauri::command] #[specta::specta]` wrapper in `src/commands/` and list it in `bindings::builder()`. Mark 64-bit fields of exported types `#[specta(type = specta_typescript::Number)]`, or use `Num<T>` for arguments and results.
3. Run `just bindings`, then add the method to `Backend`, its Tauri implementation and the memory backend.

Component and screen tests render against `createMemoryBackend(seed)`, which records calls and plays the backend's part with `emitScan`, `emitVerify` and `emitBrowse`.

## Adding a component

1. Write its page in `docs/design/components/` (anatomy, variants, sizes as tokens, states, keyboard, accessibility) and list it in `docs/SUMMARY.md`.
2. Add any sizes it needs to `design/tokens/component.json`; never put raw colors or lengths in a component (Stylelint and ESLint reject them).
3. Create `src/lib/components/<Name>/` with `<Name>.svelte` (first line: `<!-- Spec: docs/design/components/<page>.md -->`), `index.ts` and `<Name>.test.ts`. Build interactive parts on Bits UI; icons come from Lucide.
4. Test behaviour through roles and keys (Testing Library), and run `expectNoAxeViolations` on it.
5. Add it to the gallery.

Screens and dialogs follow the same pattern with their pages in `docs/design/screens/` and their code in `src/lib/<area>/`.

Keyboard shortcuts go in `src/lib/keyboard/keymap.ts` only; the dispatcher, shortcut help, command palette and [keyboard page](../design/keyboard.md) all read it.
