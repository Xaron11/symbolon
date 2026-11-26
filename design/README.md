# Design tokens

Platform-neutral source of Symbolon's design tokens, in the [W3C Design Tokens](https://www.designtokens.org/) (DTCG) format: `tokens/primitive/` (color ramps), `tokens/semantic/` (colors and shadows per theme, plus scales) and `tokens/component.json`. Every client builds its own outputs from these files; the desktop app runs `pnpm tokens` in `crates/symbolon-app/ui`. See the book's [Design system](../docs/design/index.md) page and [ADR-0022](../docs/adr/0022-design-system.md).
