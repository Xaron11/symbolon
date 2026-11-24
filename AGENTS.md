# AGENTS.md

Instructions for AI coding agents and humans working in this repository.

## What this is

Symbolon compares two or more file locations (directories, disks, pendrives, Android phones over ADB) and reports identical, missing, changed, moved/renamed and duplicated files. The specification is [`docs/start.md`](docs/start.md); decisions that refine or deviate from it are ADRs in [`docs/adr/`](docs/adr/). Read the spec section and the ADRs touching the area before changing it.

## Layout and dependency rule

| Crate                   | Role                                                                          |
| ----------------------- | ----------------------------------------------------------------------------- |
| `crates/symbolon-core`  | All logic. No UI, no Tauri, no async runtime. Public API: `Scanner`, `Query`. |
| `crates/symbolon-cli`   | `symbolon` binary (clap). Thin client of core.                                |
| `crates/symbolon-app`   | Tauri 2 backend (thin command wrappers); SvelteKit frontend in `ui/`.         |
| `crates/symbolon-agent` | Static aarch64 musl scanner pushed to Android. Thin wrapper over core.        |

`cli`, `app` and `agent` depend on `core`; `core` depends on none of them. Business logic never lives in the frontend or the Tauri commands. Platform-specific code stays behind features/`cfg` modules (ADR-0006).

## Environment and commands

Everything comes from the Nix dev shell (`direnv allow` or `nix develop`), which also installs the git hooks. Run `just` to list recipes. The ones you need:

| Task                            | Command                                          |
| ------------------------------- | ------------------------------------------------ |
| Format everything               | `just fmt`                                       |
| All lints (what CI runs)        | `just lint`                                      |
| Rust lints only / frontend only | `just lint-rust` / `just lint-ui`                |
| Tests                           | `just test` and `just test-ui`                   |
| Coverage                        | `just coverage` (`--html` for a report)          |
| Licenses and advisories         | `just deny`                                      |
| Desktop app with hot reload     | `just app-dev`                                   |
| Android agent                   | `just android-scanner check-agent`               |
| Docs (book + API, link check)   | `just docs-check` (`just docs-serve` to preview) |

Before you say a change is done: `just fmt lint test` (plus `just test-ui` for frontend changes, `just docs-check` for docs changes) must pass. The pre-push hook runs the same checks.

Language servers for every file type come from the dev shell; see `docs/development/editor.md`. The book in `docs/` (mdBook) explains concepts and architecture; start there when the code alone does not tell you why.

## Code rules

- **Rust:** edition 2024, toolchain pinned in `rust-toolchain.toml`. Clippy `pedantic` plus restriction lints are configured in the workspace `Cargo.toml`; CI denies all warnings.
- **No panicking shortcuts** outside tests: no `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!`. Return typed errors (`thiserror` in libraries, `anyhow` in binaries).
- **No `unsafe`.** `core` forbids it; elsewhere it needs an ADR.
- **Suppress lints with `#[expect(lint, reason = "...")]`**, never a bare `#[allow]`.
- **`core` never prints**; it reports through events and return values. Every public item in `core` has a doc comment.
- **Comments explain why, not what.** No restating the code, no AI-style narration. English only.
- **Paths:** never lossily convert a path. Raw bytes are preserved; matching uses `path_key` (ADR-0004).
- **Hashes:** never treat a partial hash as proof of identity; never compare hashes of different algorithms (spec §5).
- **Frontend:** TypeScript strict (with `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`), `typescript-eslint` strict-type-checked, Svelte 5 runes. Large lists are paginated by Rust and virtualized in the UI.
- **PoC code** is labelled `PoC` in a comment and tracked by an issue.

## Documentation

Docs change in the same PR as the code. Details: `docs/development/documentation.md`.

- Every public Rust item has rustdoc: a one-sentence summary, then `# Errors` / `# Panics` / `# Examples` as they apply. Examples are doctests. Use intra-doc links; broken links fail CI.
- Exported TypeScript in `ui/src/lib` has TSDoc (enforced by ESLint).
- Concepts, architecture and how-tos go in the book (`docs/`, listed in `docs/SUMMARY.md`); decisions go in ADRs; do not duplicate, link.
- Mark not-yet-built things as _(planned)_ with their milestone and remove the mark when they land.

## Tests

See [`docs/testing.md`](docs/testing.md). Every behavior change comes with tests; bug fixes start with a failing test. Catalog schema changes need a migration and a fixture test (`docs/schema.md`).

## Workflow

- Never commit to `main`; it is protected. Branch as `<type>/<short-name>` (for example `feat/tiered-hashing`) and open a PR.
- Commits and PR titles are [Conventional Commits](https://www.conventionalcommits.org): `feat(core): ...`, `fix(cli): ...`. Scopes: `core`, `cli`, `app`, `ui`, `agent`, `android`, `catalog`, `compare`, `nix`, `ci`, `deps`, `release`, `adr`. PRs are squash-merged, so the PR title becomes the commit on `main`.
- Commits must be signed. No co-author or tool attribution lines in commits or PR bodies.
- Fill in the PR template: before and after (concrete behavior or state), testing, docs, schema and security impact. Link the issue (`Closes #N`).
- Decisions the spec does not settle: pick the simplest option that keeps the architecture flexible, write an ADR (copy `docs/adr/0000-template.md`), mention it in the PR.
- Releases: `just release` opens the release PR, `just tag` creates the signed tag after merge (ADR-0007). Do not create tags or releases any other way.

## Security

- Never commit secrets, personal paths or real device serials; use fixtures.
- Catalogs and `.symbolon` exports contain paths and hashes of private data: treat test data from real devices as sensitive.
- Workflows: pin actions to full commit SHAs, start from `permissions: {}`, use `persist-credentials: false`, pass untrusted values through `env:` (zizmor enforces this).
- Android: never auto-authorize ADB; remove pushed binaries after use.
