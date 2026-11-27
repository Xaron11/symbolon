# Design system

Symbolon's interface is built from one platform-neutral design system: design tokens, component and screen specs, and a keymap. The desktop app implements it in Svelte; future Android and web clients implement the same specs ([ADR-0021](../adr/0021-gui-clients.md), [ADR-0022](../adr/0022-design-system.md)).

## Principles

- **Dense and calm.** The interface is for people comparing large file sets: compact rows, monospace numbers, borders instead of shadows, color only where it carries meaning.
- **Tokens only.** Every color, size, radius, font and duration comes from a [design token](tokens.md). A component never contains a raw hex value or pixel length; changing the look means changing tokens.
- **Color is never the only signal.** Each comparison category has a color, an icon and a label. Category colors mark icons and tints; labels always use the text colors. Contrast is at least 4.5:1 for text and 3:1 for borders, focus and category graphics, in both themes, and category colors stay apart under simulated color blindness. Tests check all of this.
- **Keyboard-first.** Everything works without a pointer, focus is always visible, and the same keys work everywhere, including vi-style navigation in lists.
- **Large data stays smooth.** Lists and trees render only what is visible and load rows on demand.
- **Respect the system.** The theme follows the OS unless the user picks one; reduced motion turns animations off.

## Tokens

Tokens live in `design/tokens/` as [DTCG](https://www.designtokens.org/) JSON:

| Layer     | File                                                                                                                            | Used by components |
| --------- | ------------------------------------------------------------------------------------------------------------------------------- | ------------------ |
| Primitive | `primitive/color.json`: color ramps                                                                                             | Never              |
| Semantic  | `semantic/light.json`, `semantic/dark.json`: colors and shadows by meaning; `semantic/scale.json`: spacing, radii, type, motion | Yes                |
| Component | `component.json`: sizes of specific parts                                                                                       | Yes                |

`pnpm tokens` in `crates/symbolon-app/ui` builds them into CSS variables (`--sy-…`), a typed TS module and the [token reference](tokens.md). The app's `dev`, `build`, `check`, `lint` and `test` scripts run it first.

## Themes

Light and dark are equal themes. By default the app follows the OS (`prefers-color-scheme`); choosing a theme sets `data-theme` on the root element and is remembered, and a remembered theme is applied before the first paint ([ThemeSwitch](components/theme-switch.md)).

## Gallery

Development builds show every component in both themes at `/gallery`; see [Working on the UI](../development/ui.md).
