set shell := ["bash", "-euo", "pipefail", "-c"]

# The one entry point for local work and CI: every workflow job runs these recipes, and the
# workflows only set up tools, caches and artifacts. Scripts longer than a few lines are in scripts/.

ui := "crates/symbolon-app/ui"
agent_target := "aarch64-unknown-linux-musl"
agent := "target/" + agent_target + "/release-agent/symbolon-agent"

# Windows covers core and cli until the Windows milestone (M8). Only library crates have doctests.
rust_packages := if os() == "windows" { "-p symbolon-core -p symbolon-cli" } else { "--workspace" }
doc_packages := if os() == "windows" { "-p symbolon-core" } else { "--workspace" }

[private]
default:
    @just --list --unsorted

# --- develop -------------------------------------------------------------------

# Debug build of every crate.
[group('develop')]
build:
    cargo build {{ rust_packages }}

# Desktop app with hot reload.
[group('develop')]
app-dev: ui-install
    cd crates/symbolon-app && cargo tauri dev

# Desktop app that agents can screenshot and drive through the Tauri MCP server (ADR-0025).
[group('develop')]
app-dev-mcp: ui-install
    cd crates/symbolon-app && cargo tauri dev --features mcp-bridge --config '{"app":{"withGlobalTauri":true}}'

# Release build of the desktop app, without packaging it.
[group('develop')]
app-build: ui-install
    cd crates/symbolon-app && cargo tauri build --no-bundle

# The interface alone in a browser, without Tauri.
[group('develop')]
ui-dev: ui-install
    pnpm --dir {{ ui }} dev

# Install the frontend's dependencies from the lockfile.
[group('develop')]
ui-install:
    pnpm --dir {{ ui }} install --frozen-lockfile --silent

# Two generated locations scanned into target/demo/catalog.db, for trying the app.
[group('develop')]
demo files="5000":
    scripts/demo.sh {{ files }}

# Rebuild the design tokens (CSS, TypeScript and the book's token page) from design/tokens/.
[group('develop')]
tokens: ui-install
    pnpm --dir {{ ui }} tokens

# Regenerate the book's keyboard page from the keymap.
[group('develop')]
keymap-doc: ui-install
    pnpm --dir {{ ui }} keymap-doc

# Rewrite the frontend's command bindings from the Tauri commands (ADR-0024).
[group('develop')]
bindings:
    cargo test -p symbolon-app --lib bindings::tests::write -- --ignored

# Write the catalog fixture for a new schema version (see docs/schema.md).
[group('develop')]
schema-fixture:
    cargo test -p symbolon-core --test schema_fixtures -- --ignored write_fixture

# Format every file in the repository.
[group('develop')]
fmt: ui-install
    treefmt

# --- check ---------------------------------------------------------------------

# Everything CI checks except the Nix build (`just nix-check`); run it before pushing.
[group('check')]
check: lint test test-ui ui-build deny deny-ui docs-check

# Formatting, hooks, Rust and frontend lints.
[group('check')]
lint: hooks lint-rust lint-ui

# Every pre-commit hook on every file: formatting, typos, Nix, shell, workflows.
[group('check')]
hooks: ui-install
    pre-commit run --all-files --show-diff-on-failure

# core must also build with its optional features off (ADR-0006).
# Clippy for every crate and feature set, rustdoc, and unused dependencies.
[group('check')]
[linux]
lint-rust:
    cargo clippy --workspace --all-targets --locked -- -D warnings
    cargo clippy -p symbolon-core --no-default-features --all-targets --locked -- -D warnings
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
    cargo shear

# Clippy for the crates Windows covers.
[group('check')]
[windows]
lint-rust:
    cargo clippy {{ rust_packages }} --all-targets --locked -- -D warnings

# Prettier, ESLint, Stylelint and svelte-check.
[group('check')]
lint-ui: ui-install
    pnpm --dir {{ ui }} lint
    pnpm --dir {{ ui }} check

# Production build of the interface.
[group('check')]
ui-build: ui-install
    pnpm --dir {{ ui }} build

# Licenses, advisories and sources of the Rust dependencies.
[group('check')]
deny:
    cargo deny check

# Licenses of the frontend's dependencies.
[group('check')]
deny-ui: ui-install
    pnpm --dir {{ ui }} license-check

# Flake checks and the Nix packages of the CLI and the app (slow).
[group('check')]
nix-check:
    nix flake check

# --- test ----------------------------------------------------------------------

# Rust tests and doctests.
[group('test')]
test:
    cargo nextest run {{ rust_packages }} --locked --no-tests=warn
    cargo test {{ doc_packages }} --locked --doc

# Leaves out the ignored tests that rewrite committed files: the bindings writer would race
# `bindings_are_current`, which reads the same file.
# The CI test run: instrumented, with the slow ignored tests (100k-file acceptance) too.
[group('test')]
[linux]
test-ci:
    cargo llvm-cov nextest {{ rust_packages }} --locked --profile ci --no-tests=warn \
        --run-ignored all -E 'not (test(=write_fixture) | test(=bindings::tests::write))'
    cargo test {{ doc_packages }} --locked --doc

# Each property-test case creates a catalog, and file creation is slow on Windows (~0.5 s per
# case). The model tests check SQL, not the platform; Linux runs all 200 cases.
# The CI test run, with fewer property-test cases.
[group('test')]
[windows]
test-ci:
    PROPTEST_CASES=32 cargo nextest run {{ rust_packages }} --locked --profile ci --no-tests=warn
    cargo test {{ doc_packages }} --locked --doc

# Frontend tests.
[group('test')]
test-ui: ui-install
    pnpm --dir {{ ui }} test

# Acceptance at scale, timed in a release build: 100k-file scan and rescan, 200k × 2 comparison.
[group('test')]
test-acceptance:
    cargo nextest run --release -p symbolon-core --test acceptance --test compare_scale --run-ignored ignored-only --no-capture

# Line coverage; `just coverage --html` writes a browsable report to target/llvm-cov/html.
[group('test')]
coverage *args:
    cargo llvm-cov nextest {{ rust_packages }} --locked {{ args }}

# Report on the last coverage run, e.g. `just coverage-report --summary-only`.
[group('test')]
coverage-report *args:
    cargo llvm-cov report {{ args }}

# Mutation testing (slow): which code changes do the tests fail to notice? Pass `--file` to narrow.
[group('test')]
mutants *args:
    cargo mutants -p symbolon-core --test-tool nextest --jobs 4 --timeout 120 --output target/mutants {{ args }}

# Benchmarks.
[group('test')]
bench:
    cargo bench --workspace

# --- docs ----------------------------------------------------------------------

# Build the book and the API reference into target/book.
[group('docs')]
docs: docs-api docs-book

# mdBook copies every non-Markdown file in its source directory into each build, so the API pages
# survive rebuilds, including `docs-serve`.
# Build the API reference into docs/api (git-ignored).
[group('docs')]
docs-api:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
    rm -rf docs/api
    cp -r target/doc docs/api

# Build the book alone.
[group('docs')]
docs-book:
    mdbook-mermaid install docs >/dev/null
    mdbook build docs

# Serve the book and the API reference with live reload; run `just docs-api` after editing doc comments.
[group('docs')]
docs-serve: docs-api
    mdbook-mermaid install docs >/dev/null
    mdbook serve docs --open

# Build everything and check every internal link and anchor.
[group('docs')]
docs-check: docs docs-links

# The 404 page links by absolute site paths (book.toml site-url), which only resolve once published.
# Check the links and anchors of the built book.
[group('docs')]
docs-links:
    lychee --offline --include-fragments --no-progress --root-dir "$PWD/target/book" --exclude-path target/book/api --exclude-path target/book/404.html 'target/book/**/*.html'

# --- android -------------------------------------------------------------------

# zig's linker ignores the `-O1` rustc passes and warns about it on every link.
# Build the static aarch64 agent that is pushed to Android phones.
[group('android')]
android-scanner:
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_RUSTFLAGS="-A linker_messages" \
        cargo zigbuild --locked -p symbolon-agent --profile release-agent --target {{ agent_target }}
    @echo {{ agent }}

# Fail unless the agent is fully static and its segments suit 4/16/64 KiB page kernels (ADR-0003).
[group('android')]
check-agent:
    scripts/check-agent.sh {{ agent }}

# Push the agent, list a directory on the device through it (NDJSON), then remove the binary.
[group('android')]
android-probe dir="/sdcard/DCIM" serial="": android-scanner
    scripts/android-probe.sh {{ agent }} {{ quote(dir) }} {{ quote(serial) }}

# --- release -------------------------------------------------------------------

# Open a release PR from an up-to-date main. Version defaults to the one implied by commits.
[group('release')]
release version="": ui-install
    scripts/release.sh {{ ui }} {{ quote(version) }}

# After the release PR is merged: create and push the signed tag for the current version.
[group('release')]
tag:
    scripts/tag.sh

# Rewrite CHANGELOG.md from the commits.
[group('release')]
changelog:
    git cliff -o CHANGELOG.md
    treefmt CHANGELOG.md

# Release archive of the CLI for a target, with the agent from `just android-scanner`, into dist/.
[group('release')]
dist-cli target name="dev":
    scripts/dist-cli.sh {{ quote(target) }} {{ quote(name) }} {{ agent }}

# Release archive of the Android agent from `just android-scanner`, into dist/.
[group('release')]
dist-agent name="dev":
    scripts/dist-agent.sh {{ quote(name) }} {{ agent }}

# Desktop packages (e.g. `appimage,deb` or `nsis`) into dist/; `with-agent="true"` bundles the agent.
[group('release')]
dist-app bundles name="dev" with-agent="false": ui-install
    scripts/dist-app.sh {{ quote(bundles) }} {{ quote(name) }} {{ with-agent }} {{ agent }}

# SHA256SUMS for everything in dist/.
[group('release')]
dist-checksums:
    cd dist && sha256sum -- * > SHA256SUMS

# Release notes for a tag: its changelog section and where to find the source.
[group('release')]
release-notes tag:
    scripts/release-notes.sh {{ quote(tag) }}

# Update flake inputs on a branch and open a PR (fallback if Dependabot's nix support is unavailable).
[group('release')]
update-flake:
    scripts/update-flake.sh
