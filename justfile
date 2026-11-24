set shell := ["bash", "-euo", "pipefail", "-c"]

ui := "crates/symbolon-app/ui"
agent_target := "aarch64-unknown-linux-musl"

default:
    @just --list

# --- build -------------------------------------------------------------------

build:
    cargo build --workspace

# Static aarch64 binary pushed to Android over adb.
android-scanner:
    cargo zigbuild -p symbolon-agent --profile release-agent --target {{ agent_target }}
    @echo target/{{ agent_target }}/release-agent/symbolon-agent

# Fail unless the agent is fully static and its segments suit 4/16/64 KiB page kernels (ADR-0003).
check-agent:
    #!/usr/bin/env bash
    set -euo pipefail
    f=target/{{ agent_target }}/release-agent/symbolon-agent
    if readelf -lW "$f" | grep -q INTERP; then echo "error: $f is dynamically linked" >&2; exit 1; fi
    readelf -lW "$f" | awk '$1 == "LOAD" { print $NF }' | while read -r align; do
        (( align >= 0x10000 )) || { echo "error: LOAD alignment $align < 64 KiB" >&2; exit 1; }
    done
    echo "ok: static, LOAD alignment >= 64 KiB"

app-dev: ui-install
    cd crates/symbolon-app && cargo tauri dev

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

# Line coverage report; `just coverage --html` for a browsable one in target/llvm-cov/html.
coverage *args:
    cargo llvm-cov nextest --workspace --locked {{ args }}

# Write the catalog fixture for a new schema version (see docs/schema.md).
schema-fixture:
    cargo test -p symbolon-core --test schema_fixtures -- --ignored write_fixture

bench:
    cargo bench --workspace

# PoC spike (M0): push the agent, hash a directory on the device, then remove the binary.
android-probe dir="/sdcard/DCIM" serial="": android-scanner
    #!/usr/bin/env bash
    set -euo pipefail
    adb_args=()
    [[ -n "{{ serial }}" ]] && adb_args=(-s "{{ serial }}")
    remote=/data/local/tmp/symbolon-agent
    adb "${adb_args[@]}" push target/{{ agent_target }}/release-agent/symbolon-agent "$remote" >&2
    adb "${adb_args[@]}" shell chmod 755 "$remote"
    trap 'adb "${adb_args[@]}" shell rm -f "$remote"' EXIT
    adb "${adb_args[@]}" exec-out "$remote" "{{ dir }}"

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

docs-links:
    lychee --offline --include-fragments --no-progress --root-dir "$PWD/target/book" --exclude-path target/book/api 'target/book/**/*.html'

# --- release (see docs/adr/0007-development-workflow.md) ---------------------

changelog:
    git cliff -o CHANGELOG.md
    treefmt CHANGELOG.md

# Update flake inputs on a branch and open a PR (fallback if Dependabot's nix support is unavailable).
update-flake:
    #!/usr/bin/env bash
    set -euo pipefail
    [[ -z "$(git status --porcelain)" ]] || { echo "error: working tree is not clean" >&2; exit 1; }
    git switch -c "build/flake-update-$(date +%F)" origin/main
    nix flake update --commit-lock-file --option commit-lockfile-summary "build(nix): update flake inputs"
    git push -u origin HEAD
    gh pr create --fill

# Open a release PR from an up-to-date main. Version defaults to the one implied by commits.
release version="": ui-install
    #!/usr/bin/env bash
    set -euo pipefail
    die() { echo "error: $*" >&2; exit 1; }
    [[ -z "$(git status --porcelain)" ]] || die "working tree is not clean"
    git fetch --quiet origin main --tags
    [[ "$(git rev-parse HEAD)" == "$(git rev-parse origin/main)" ]] || die "run this on main, in sync with origin/main"
    version="{{ version }}"
    version="${version:-$(git cliff --bumped-version)}"
    version="${version#v}"
    [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "not a SemVer version: $version"
    git rev-parse -q --verify "refs/tags/v$version" >/dev/null && die "tag v$version already exists"
    git switch -c "release/v$version"
    sed -i "0,/^version = \".*\"/s//version = \"$version\"/" Cargo.toml
    cargo update --workspace --quiet
    (cd {{ ui }} && npm pkg set version="$version")
    git cliff --tag "v$version" -o CHANGELOG.md
    treefmt CHANGELOG.md {{ ui }}/package.json
    git commit --quiet -am "chore(release): v$version"
    git push --quiet -u origin HEAD
    gh pr create --title "chore(release): v$version" \
        --body "Release v$version. After merging, run \`just tag\` on main."

# After the release PR is merged: create and push the signed tag for the current version.
tag:
    #!/usr/bin/env bash
    set -euo pipefail
    die() { echo "error: $*" >&2; exit 1; }
    git switch --quiet main
    git pull --quiet --ff-only origin main
    git fetch --quiet origin --tags
    version="$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name == "symbolon-core") | .version')"
    tag="v$version"
    git rev-parse -q --verify "refs/tags/$tag" >/dev/null && die "tag $tag already exists"
    git log -1 --format=%s | grep -Eq "^chore\(release\): $tag( \(#[0-9]+\))?$" \
        || die "HEAD is not the release commit for $tag"
    git tag -s "$tag" -m "Symbolon $tag"
    git -c gpg.ssh.allowedSignersFile=.github/allowed_signers tag -v "$tag"
    git push origin "$tag"
