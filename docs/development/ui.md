# Working on the UI

The desktop app's interface lives in `crates/symbolon-app/ui` (SvelteKit, Svelte 5, TypeScript). It implements the [design system](../design/index.md); read that first.

## Running it

| Task                              | Command (in `crates/symbolon-app/ui` unless noted)    |
| --------------------------------- | ----------------------------------------------------- |
| Desktop app with hot reload       | `just app-dev` (repository root)                      |
| Interface alone in a browser      | `pnpm dev`                                            |
| Rebuild tokens after editing JSON | `pnpm tokens` (`dev`, `build`, `lint`, `test` run it) |
| Regenerate the keyboard page      | `pnpm keymap-doc`                                     |
| Lints and type checks             | `just lint-ui` (repository root)                      |
| Tests                             | `just test-ui` (repository root)                      |

## The gallery

Development builds serve `/gallery`: every component in both themes side by side, including a 100,000-row table and a lazy tree. Use it to check a change in both themes and with long values before opening a PR. Release builds answer 404 there.

## Adding a component

1. Write its page in `docs/design/components/` (anatomy, variants, sizes as tokens, states, keyboard, accessibility) and list it in `docs/SUMMARY.md`.
2. Add any sizes it needs to `design/tokens/component.json`; never put raw colors or lengths in a component (Stylelint and ESLint reject them).
3. Create `src/lib/components/<Name>/` with `<Name>.svelte` (first line: `<!-- Spec: docs/design/components/<page>.md -->`), `index.ts` and `<Name>.test.ts`. Build interactive parts on Bits UI; icons come from Lucide.
4. Test behaviour through roles and keys (Testing Library), and run `expectNoAxeViolations` on it.
5. Add it to the gallery.

Keyboard shortcuts go in `src/lib/keyboard/keymap.ts` only; the dispatcher, shortcut help, command palette and [keyboard page](../design/keyboard.md) all read it.
