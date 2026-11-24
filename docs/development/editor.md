# Editor and LSP setup

The repository does not ship editor-specific configuration. Instead, the dev shell provides every language server at a version that matches the toolchain and CI, and each server reads its project settings from tool-native files in the repository. Any LSP-capable editor works; Neovim is used as the example below.

## Getting the servers on `PATH`

The servers live in the dev shell, so the editor must see its environment:

- start the editor from a shell inside the project (with direnv active, or after `nix develop`), or
- use a direnv integration for your editor that loads the environment per project.

Do not install these servers with a separate manager (Mason, VS Code extensions bundling servers, global npm installs): versions would drift from the ones CI uses.

## Servers

| Files              | Server (binary)                                                                            | Project configuration it reads                                                       |
| ------------------ | ------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------ |
| Rust               | `rust-analyzer` (from the pinned toolchain)                                                | `rust-analyzer.toml`, workspace lints in `Cargo.toml`, `rustfmt.toml`, `clippy.toml` |
| Nix                | `nixd`                                                                                     | editor settings (see below)                                                          |
| TOML               | `taplo lsp stdio`                                                                          | `taplo.toml`; schemas (e.g. `Cargo.toml`) from SchemaStore                           |
| Svelte             | `svelteserver --stdio`                                                                     | `tsconfig.json`, `vite.config.ts`                                                    |
| TypeScript, JS     | `vtsls --stdio`                                                                            | `tsconfig.json`                                                                      |
| ESLint diagnostics | `vscode-eslint-language-server --stdio`                                                    | `eslint.config.js`                                                                   |
| JSON, CSS, HTML    | `vscode-json-language-server`, `vscode-css-language-server`, `vscode-html-language-server` | `$schema` keys in JSON files                                                         |
| YAML               | `yaml-language-server --stdio`                                                             | SchemaStore matches workflows, `dependabot.yml`, `action.yml`                        |
| Markdown           | `marksman server` (links, headings); `rumdl server` (lint)                                 | `.rumdl.toml`                                                                        |
| Spelling (all)     | `typos-lsp`                                                                                | `_typos.toml`                                                                        |
| `justfile`         | `just-lsp`                                                                                 | —                                                                                    |

The frontend servers need `node_modules`; run `just ui-install` once.

## rust-analyzer

`rust-analyzer.toml` in the repository root sets what rust-analyzer accepts from a project file: import style (one `use` per module, grouped, `crate::` paths) and style diagnostics. These apply in every editor.

Some settings are only read from the editor or from your user-level config, not from a project file (rust-analyzer looks up its check and target-directory settings without a source root, so the project file is skipped for them). Set these yourself:

| Setting           | Value      | Why                                                                                                    |
| ----------------- | ---------- | ------------------------------------------------------------------------------------------------------ |
| `check.command`   | `"clippy"` | See the same lints CI enforces, on save.                                                               |
| `cargo.targetDir` | `true`     | Build into `target/rust-analyzer`, so the editor never blocks `cargo` in a terminal on the build lock. |
| `cargo.features`  | `"all"`    | Check feature-gated code too.                                                                          |

Either in your editor's rust-analyzer settings, or for all projects in `~/.config/rust-analyzer/rust-analyzer.toml`:

```toml
[check]
command = "clippy"

[cargo]
targetDir = true
```

## nixd

nixd gets its settings from the editor. Useful values for this repository:

| Setting              | Value                                                          |
| -------------------- | -------------------------------------------------------------- |
| `nixpkgs.expr`       | `import (builtins.getFlake (toString ./.)).inputs.nixpkgs { }` |
| `formatting.command` | `[ "nixfmt" ]`                                                 |

## Formatting

`treefmt <file>` formats any file in the repository exactly as the pre-commit hook and CI do. Point your editor's format-on-save at it, or at the per-language tools it wraps (rustfmt, nixfmt, taplo, and the frontend's Prettier in `crates/symbolon-app/ui/node_modules/.bin/prettier`).

## Example: Neovim 0.11+

Server names below are the configurations shipped by [nvim-lspconfig](https://github.com/neovim/nvim-lspconfig); `rumdl` is defined inline. Put this in your own config, not in the repository:

```lua
vim.lsp.config("rust_analyzer", {
  settings = {
    ["rust-analyzer"] = {
      check = { command = "clippy" },
      cargo = { targetDir = true, features = "all" },
    },
  },
})

vim.lsp.config("nixd", {
  settings = {
    nixd = {
      nixpkgs = { expr = "import (builtins.getFlake (toString ./.)).inputs.nixpkgs { }" },
      formatting = { command = { "nixfmt" } },
    },
  },
})

vim.lsp.config("vtsls", {
  settings = { vtsls = { autoUseWorkspaceTsdk = true } },
})

vim.lsp.config("rumdl", {
  cmd = { "rumdl", "server" },
  filetypes = { "markdown" },
  root_markers = { ".rumdl.toml", ".git" },
})

vim.lsp.enable({
  "rust_analyzer", "nixd", "taplo", "svelte", "vtsls", "eslint",
  "jsonls", "cssls", "html", "yamlls", "marksman", "rumdl", "typos_lsp", "just",
})
```

For format-on-save with one tool for everything, a formatter plugin such as conform.nvim can run `treefmt` on the current file.
