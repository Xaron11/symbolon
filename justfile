set shell := ["bash", "-euo", "pipefail", "-c"]

ui := "crates/symbolon-app/ui"
agent_target := "aarch64-unknown-linux-musl"

default:
    @just --list

build:
    cargo build --workspace

test:
    cargo nextest run --workspace --no-tests=warn
    cargo test --workspace --doc

lint: ui-install
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    pnpm --dir {{ui}} lint
    pnpm --dir {{ui}} check

fmt:
    cargo fmt --all
    pnpm --dir {{ui}} format

deny: ui-install
    cargo deny check
    pnpm --dir {{ui}} license-check

bench:
    cargo bench --workspace

ui-install:
    pnpm --dir {{ui}} install --frozen-lockfile --silent

ui-dev: ui-install
    pnpm --dir {{ui}} dev

app-dev: ui-install
    cd crates/symbolon-app && cargo tauri dev

app-build: ui-install
    cd crates/symbolon-app && cargo tauri build --no-bundle

changelog:
    git cliff -o CHANGELOG.md

# Static aarch64 binary pushed to Android over adb.
android-scanner:
    cargo zigbuild -p symbolon-agent --profile release-agent --target {{agent_target}}
    @echo target/{{agent_target}}/release-agent/symbolon-agent

# PoC spike (M0): push the agent, hash a directory on the device, then remove the binary.
android-probe dir="/sdcard/DCIM" serial="": android-scanner
    #!/usr/bin/env bash
    set -euo pipefail
    adb_args=()
    [[ -n "{{serial}}" ]] && adb_args=(-s "{{serial}}")
    remote=/data/local/tmp/symbolon-agent
    adb "${adb_args[@]}" push target/{{agent_target}}/release-agent/symbolon-agent "$remote" >&2
    adb "${adb_args[@]}" shell chmod 755 "$remote"
    trap 'adb "${adb_args[@]}" shell rm -f "$remote"' EXIT
    adb "${adb_args[@]}" exec-out "$remote" "{{dir}}"
