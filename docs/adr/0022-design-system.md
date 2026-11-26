# 0022. Design system: tokens, components and keyboard

- **Status:** Accepted
- **Date:** 2026-11-26

## Context

The GUI is for power users of a file comparison tool: dense, keyboard-first, data-heavy, comfortable in dark and light environments (spec §9). It must look the same on every OS and carry over to future clients ([ADR-0021](0021-gui-clients.md)). The spec asks that no heavy component framework be added without an ADR.

## Decision

1. **Visual direction:** minimal, flat, structured by 1px borders, in the style of developer tools. Dark and light themes are equal; the OS decides by default and the user can override it. One blue accent for focus, selection and primary actions. Comparison categories have fixed semantic colors, always paired with an icon and a label.
2. **Tokens:** W3C Design Tokens (DTCG) JSON in `design/tokens/`, in three layers: primitive color ramps (never used directly), semantic tokens (one file per theme plus theme-independent scales) and component tokens. Style Dictionary (development dependency) builds them into CSS custom properties prefixed `--sy-`, a typed TS module for code that cannot read CSS, and the book's [token reference](../design/tokens.md). Other platforms add generator targets, not new sources.
3. **Accessibility in the tokens:** tests require text contrast of at least 4.5:1 and borders, focus, accent and category graphics of at least 3:1 in both themes, and category colors that stay apart under simulated protanopia and deuteranopia (CIEDE2000 ≥ 20 between categories, ≥ 10 under each simulation). Category colors mark icons and tints only; labels use the text colors. A category keeps its hue in both themes: identical emerald, different red, only in amber, moved and renamed violet, collision pink, unverified gray.
4. **Components:** our own Svelte 5 components styled only with tokens; [Bits UI](https://bits-ui.com) (MIT, headless) provides behaviour for dialogs, menus, selects, comboboxes, tabs, tooltips, popovers and the command palette. Lint rules reject raw colors anywhere in components (hex, named, `rgb()`-style functions, also in script strings) and raw lengths or durations for spacing, gaps, borders, outlines, radii, type and motion; arithmetic on tokens (`calc(var(--sy-…) * 2)`) is allowed, and layout sizes such as widths are not restricted. Icons come from Lucide (ISC).
5. **Fonts:** IBM Plex Sans for the interface and JetBrains Mono for paths, hashes and numbers (tabular figures), bundled from Fontsource (OFL-1.1, added to the npm license allowlist), never loaded from the network.
6. **Data components:** tables and trees are virtualized and read rows through an offset-based data source (`count()` and `rows(offset, limit, sort)`), not cursors, so a list can jump anywhere (scrollbar drag, end of list) without loading earlier pages. The app–core bridge implements it over the comparison and catalog queries.
7. **Compare summary:** a composition bar (one 100% stacked bar with a legend table) replaces the pie or doughnut of spec §9: there are seven categories and small ones would be unreadable slices. Bar charts by extension and size use Chart.js.
8. **Keyboard:** one keymap table drives handling, the shortcut help and the book's keyboard page. Standard keys and vi-style keys (`j`/`k`/`h`/`l`, `gg`/`G`, `Ctrl+D`/`Ctrl+U`, `g` sequences) are always on together; letter keys never act while text is being entered.
9. **Documentation:** each component's platform-neutral spec (anatomy, states, keyboard, accessibility) is a page in the book under `docs/design/components/`, and the component's source file links to it.

## Consequences

- The look is ours and consistent across platforms; accessibility of complex widgets comes from Bits UI rather than our own focus and ARIA code.
- Changing a color, size or theme means editing token JSON; components do not change.
- The bridge must support offset paging on large result sets, which the indexed temporary comparison tables allow.
- Two runtime UI dependencies (Bits UI, Lucide) and fonts add to the bundle; revisit if bundle size or Bits UI's maintenance becomes a problem.
