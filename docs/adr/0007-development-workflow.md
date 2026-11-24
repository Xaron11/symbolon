# 0007. Development workflow, CI, releases and project management

- **Status:** Accepted
- **Date:** 2026-11-24

## Context

After M0 the project needs a workflow that scales past a single bootstrap session: protected `main`, PR-based changes, strict and uniform formatting and lints enforced both locally and in CI, secure and cached CI, a repeatable versioning and release process, and issue/milestone tracking. The repository is private on a plan that supports rulesets, has one maintainer, and commits are SSH-signed.

## Decision

### Branches and repository settings

- All changes go through PRs from `<type>/<short-name>` branches. **Squash merge only**; the squash commit uses the PR title (a Conventional Commit, checked by the `pr-title` workflow) and the PR body.
- **`main` ruleset:** PR required (0 approvals, single maintainer), required status checks `ci-ok` and `pr-title`, linear history, signed commits, no force-push, no deletion.
- **Tag ruleset (`v*`):** only repository admins may create, update or delete release tags.
- Settings: delete branch on merge, auto-merge allowed, wiki off. Actions: read-only default `GITHUB_TOKEN`, Actions may not create or approve PRs, actions must be pinned to full commit SHAs. Dependabot alerts and security updates enabled.

### Formatting, lints and hooks

- **treefmt** (`treefmt.toml`) formats everything: rustfmt, nixfmt, taplo, and Prettier for Markdown/YAML/JSON/TS/Svelte. Prettier is the frontend's locked version, so editors, hooks and CI agree.
- **Rust:** clippy `pedantic` plus restriction lints (`unwrap_used`, `expect_used`, `panic`, `todo`, `unimplemented`, `dbg_macro`, `undocumented_unsafe_blocks`, `allow_attributes_without_reason`), rustc lints (`unreachable_pub`, `rust_2018_idioms`, …), `missing_docs` and no printing in `core`; `cargo shear` for unused dependencies; rustdoc with `-D warnings`. This supersedes the lint paragraph of ADR-0001.
- **Frontend:** `typescript-eslint` strict-type-checked + stylistic, stricter `tsconfig` flags, `svelte-check`.
- **Hooks** via `git-hooks.nix`, installed by the dev shell:
  - `pre-commit`: treefmt, typos, statix, deadnix, actionlint, zizmor, merge-conflict and private-key detection;
  - `commit-msg`: convco (Conventional Commits);
  - `pre-push`: `just lint-rust test` and `just lint-ui test-ui`.
- Hooks need `node_modules` and the cargo registry, so they are not run inside the Nix sandbox; CI runs the same hook set with `pre-commit run --all-files`.

### CI

- `ci.yml` jobs: `hooks`, `clippy` (Linux workspace, Windows core+cli), `test` (Linux: instrumented nextest with coverage summary; Windows: nextest; doctests for library crates; `symbolon-app` joins when it has tests), `frontend`, `deny`, `nix` (`nix flake check`), `android-agent` (cross-build and `check-agent`), `docs`, and `ci-ok`, which fails unless every job succeeded and is the single required check.
- **Security:** all third-party actions pinned by SHA with a version comment, `permissions: {}` at workflow level and minimal per-job grants, `persist-credentials: false`, untrusted values passed via `env:`, no `pull_request_target`, zizmor (online audits in CI) and actionlint on every change.
- **Caching:** `Swatinem/rust-cache` per job type, `nix-community/cache-nix-action` keyed on the flake (and `Cargo.lock` for builds), the pnpm store via `setup-node`, prebuilt tools via `taiki-e/install-action`. Caches are saved only from `main`; PRs restore but never write, so they cannot evict or poison main's caches. Runs on PR branches are cancelled when superseded; runs on `main` are not.
- **Speed:** Nix jobs use lean dev shells (`.#ci` for hooks and docs, `.#android` for the agent) instead of the full developer shell, so they download and cache well under half as much. CI builds without debug info (`CARGO_PROFILE_DEV_DEBUG=0`). The agent's `target/` is cached with `actions/cache`, saved from `main` only.
- **Dependency updates:** Dependabot (cargo, npm, GitHub Actions, nix) weekly, grouped, with a 7-day cooldown. `just update-flake` is the manual fallback for `flake.lock`.

### Versioning and releases

- One SemVer version for all crates and `ui/package.json` (`[workspace.package] version`). `0.x` until the first stable release; breaking changes bump the minor version (git-cliff `[bump]`). Each milestone ends with a minor release (M0 → `v0.1.0`, M1 → `v0.2.0`, …); fixes are patch releases. The catalog schema version is independent (docs/schema.md).
- `just release [version]` on an up-to-date `main`: computes the next version from commits, bumps versions and `CHANGELOG.md`, opens the PR `chore(release): vX.Y.Z`.
- After merge, `just tag` creates an SSH-signed annotated tag on the release commit and pushes it.
- `release.yml` verifies the tag signature against `.github/allowed_signers` **read from `main`**, checks that the tag is on `main` and matches the workspace version, builds the CLI (Linux, Windows) and the Android agent, publishes `SHA256SUMS` and git-cliff notes linking the source tag. `0.x` releases are marked pre-release.

### Project management

- Issue forms (bug, feature, task); blank issues disabled. Labels: `type:*`, `area:*`, `priority:p0`–`p3`, `blocked`, `needs-decision`, `dependencies`.
- One GitHub milestone per spec milestone (M1–M8) with its acceptance criteria and target version. Detailed issues are written when a milestone starts; later milestones have a single tracking issue until then.
- A GitHub Projects board (Todo / In progress / In review / Done) tracks open work.

## Consequences

- Nothing reaches `main` without green CI and a signed, conventional, reviewed-by-PR commit; the changelog is generated, not written by hand.
- Releases need no stored secrets; only an admin with the signing key can publish.
- Not adopted (cost or plan limits): CodeQL and secret scanning (paid for private repositories), artifact attestations (Enterprise for private repositories), third-party coverage services. Revisit if the repository becomes public, where all three are free.
- Mutation testing (`cargo-mutants`) is a follow-up once `compare` exists.
