# 0008. Documentation and editor tooling

- **Status:** Accepted
- **Date:** 2026-11-24

## Context

The project needs documentation that stays correct as code lands (API docs, concepts, architecture, how-tos) and first-class language-server support in any editor, with Neovim as the main one. The repository is private; GitHub Pages on the current plan would publish a public site.

## Decision

### Documentation

- **API:** rustdoc for every crate. `missing_docs` across the workspace, rustdoc lints (`broken_intra_doc_links`, `private_intra_doc_links`, `invalid_rust_codeblocks`, …) and clippy's doc lints (`missing_errors_doc`, `missing_panics_doc`, `doc_markdown`). CI builds rustdoc with `-D warnings`; examples are doctests.
- **Frontend:** TSDoc on exported code in `ui/src/lib` and `ui/scripts`, enforced by `eslint-plugin-jsdoc` (`require-jsdoc` for public items; parameter and return tags not required, types carry that).
- **Book:** mdBook with `docs/` as its source, so existing documents (spec, ADRs, schema, testing) are chapters without duplication. Mermaid diagrams via `mdbook-mermaid` (the JS assets are generated at build time and git-ignored). The rustdoc output is copied into the book under `api/`.
- **Checks:** lychee checks every internal link and anchor in the built HTML (offline); rumdl lints Markdown structure in pre-commit (MD013 line length disabled, Prettier owns wrapping).
- **Publishing:** CI builds the site and uploads it as an artifact; nothing is published while the repository is private. Enable GitHub Pages when it becomes public.
- **Process:** docs change in the same PR as the code (AGENTS.md, PR template).

### Editor tooling

- No editor-specific files in the repository. The dev shell provides the language servers (rust-analyzer from the pinned toolchain, nixd, taplo, svelte-language-server, vtsls, vscode-langservers-extracted, yaml-language-server, marksman, rumdl, typos-lsp, just-lsp); each reads tool-native project config.
- `rust-analyzer.toml` holds the settings rust-analyzer honours from a project file: per-crate ("local") settings such as import granularity and style diagnostics. Verified against rust-analyzer's source and a live session: workspace-scoped settings such as `check.command` and `cargo.targetDir` are looked up without a source root for flycheck, so a project file does not affect them. Those are documented in `docs/development/editor.md` for the editor or `~/.config/rust-analyzer/rust-analyzer.toml`.

## Consequences

- Documentation errors fail CI like code errors; stale examples cannot survive because they are compiled.
- Contributors get identical language-server behaviour regardless of editor, at the cost of configuring a few rust-analyzer and nixd options in their own editor.
- If rust-analyzer starts honouring workspace settings from project files, move `check.command = "clippy"` and `cargo.targetDir = true` into `rust-analyzer.toml`.
