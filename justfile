set shell := ["bash", "-euo", "pipefail", "-c"]

ui := "crates/symbolon-app/ui"
agent_target := "aarch64-unknown-linux-musl"

default:
    @just --list

# --- build -------------------------------------------------------------------

build:
    cargo build --workspace

# Static aarch64 binary pushed to Android over adb.
# zig's linker ignores the `-O1` rustc passes and warns about it on every link.
android-scanner:
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_RUSTFLAGS="-A linker_messages" \
        cargo zigbuild --locked -p symbolon-agent --profile release-agent --target {{ agent_target }}
    @echo target/{{ agent_target }}/release-agent/symbolon-agent

# Fail unless the agent is fully static and its segments suit 4/16/64 KiB page kernels (ADR-0003).
check-agent:
    scripts/check-agent.sh target/{{ agent_target }}/release-agent/symbolon-agent

app-dev: ui-install
    cd crates/symbolon-app && cargo tauri dev

# Desktop app that agents can screenshot and drive through the Tauri MCP server (ADR-0025).
app-dev-mcp: ui-install
    cd crates/symbolon-app && cargo tauri dev --features mcp-bridge --config '{"app":{"withGlobalTauri":true}}'

# Two generated locations scanned into target/demo/catalog.db, for trying the app.
demo files="5000":
    scripts/demo.sh {{ files }}

app-build: ui-install
    cd crates/symbolon-app && cargo tauri build --no-bundle

ui-install:
    pnpm --dir {{ ui }} install --frozen-lockfile --silent

ui-dev: ui-install
    pnpm --dir {{ ui }} dev

# --- format and lint ---------------------------------------------------------

# Format every file in the repository.
fmt: ui-install
    treefmt

# Fail if anything is not formatted.
fmt-check: ui-install
    treefmt --ci

# Everything CI lints: formatting, hooks, Rust, frontend, workflows.
lint: ui-install
    pre-commit run --all-files
    just lint-rust lint-ui

lint-rust:
    cargo clippy --workspace --all-targets --locked -- -D warnings
    # core must also build with its optional features off (ADR-0006), as CI checks.
    cargo clippy -p symbolon-core --no-default-features --all-targets --locked -- -D warnings
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked --quiet
    cargo shear

lint-ui: ui-install
    pnpm --dir {{ ui }} lint
    pnpm --dir {{ ui }} check

deny: ui-install
    cargo deny check
    pnpm --dir {{ ui }} license-check

# --- test --------------------------------------------------------------------

test:
    cargo nextest run --workspace --locked --no-tests=warn
    cargo test --workspace --locked --doc

# Acceptance at scale, timed in a release build: 100k-file scan and rescan, 200k × 2 comparison.
test-acceptance:
    cargo nextest run --release -p symbolon-core --test acceptance --test compare_scale --run-ignored ignored-only --no-capture

test-ui: ui-install
    pnpm --dir {{ ui }} test

# Rewrite the frontend's command bindings from the Tauri commands (ADR-0024).
bindings:
    cargo test -p symbolon-app --lib bindings::tests::write -- --ignored

# Line coverage report; `just coverage --html` for a browsable one in target/llvm-cov/html.
coverage *args:
    cargo llvm-cov nextest --workspace --locked {{ args }}

# Write the catalog fixture for a new schema version (see docs/schema.md).
schema-fixture:
    cargo test -p symbolon-core --test schema_fixtures -- --ignored write_fixture

# Mutation testing (slow): which code changes do the tests fail to notice? Pass `--file` to narrow.
mutants *args:
    cargo mutants -p symbolon-core --test-tool nextest --jobs 4 --timeout 120 --output target/mutants {{ args }}

bench:
    cargo bench --workspace

# Push the agent, list a directory on the device through it (NDJSON), then remove the binary.
android-probe dir="/sdcard/DCIM" serial="": android-scanner
    scripts/android-probe.sh target/{{ agent_target }}/release-agent/symbolon-agent {{ quote(dir) }} {{ quote(serial) }}

# --- documentation -----------------------------------------------------------

# Build the book and the API reference into target/book.
docs: docs-api docs-book

# Build the API reference into docs/api (git-ignored). mdBook copies every non-Markdown file in
# its source directory into each build, so the API pages survive rebuilds, including `docs-serve`.
docs-api:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
    rm -rf docs/api
    cp -r target/doc docs/api

docs-book:
    mdbook-mermaid install docs >/dev/null
    mdbook build docs

# Serve the book and the API reference with live reload. After changing doc comments, run
# `just docs-api` in another terminal; the page reloads.
docs-serve: docs-api
    mdbook-mermaid install docs >/dev/null
    mdbook serve docs --open

# Build everything and check every internal link and anchor.
docs-check: docs docs-links

# The 404 page links by absolute site paths (book.toml site-url), which only resolve once published.
docs-links:
    lychee --offline --include-fragments --no-progress --root-dir "$PWD/target/book" --exclude-path target/book/api --exclude-path target/book/404.html 'target/book/**/*.html'

# --- release -----------------------------------------------------------------

changelog:
    git cliff -o CHANGELOG.md
    treefmt CHANGELOG.md

# Update flake inputs on a branch and open a PR (fallback if Dependabot's nix support is unavailable).
update-flake:
    scripts/update-flake.sh

# Open a release PR from an up-to-date main. Version defaults to the one implied by commits.
release version="": ui-install
    scripts/release.sh {{ ui }} {{ quote(version) }}

# After the release PR is merged: create and push the signed tag for the current version.
tag:
    scripts/tag.sh
