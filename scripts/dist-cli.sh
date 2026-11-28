#!/usr/bin/env bash
# Builds the CLI for a target and packs it as a release ships it: with the Android agent, licenses,
# README, man pages and shell completions, into dist/symbolon-<name>-<target>.tar.gz (.zip for
# Windows).
# Usage: dist-cli.sh <target> <name> <agent binary>
set -euo pipefail

target="$1"
name="$2"
agent="$3"
exe=symbolon
[[ "$target" == *windows* ]] && exe=symbolon.exe

# For the host itself, build without --target so the release build is shared with
# `cargo build --release` and the app (target/release) instead of compiled a second time.
if [[ "$target" == "$(rustc -vV | sed -n 's/^host: //p')" ]]; then
    cargo build --release --locked -p symbolon-cli
    bin="target/release/$exe"
else
    cargo build --release --locked -p symbolon-cli --target "$target"
    bin="target/$target/release/$exe"
fi
pkg="symbolon-$name-$target"
dir="dist/$pkg"
rm -rf "$dir"
mkdir -p "$dir/completions"
cp "$bin" LICENSE-MIT LICENSE-APACHE README.md "$dir/"
# CI artifacts lose the executable bit; it matters only on the phone, where it is set.
cp "$agent" "$dir/symbolon-agent-aarch64-linux-musl"
chmod 755 "$dir/symbolon-agent-aarch64-linux-musl"
"$bin" man --out "$dir/man"
for shell in bash zsh fish elvish powershell; do
    "$bin" completions "$shell" >"$dir/completions/symbolon.$shell"
done
echo "Corresponding source: ${GITHUB_SERVER_URL:-https://github.com}/${GITHUB_REPOSITORY:-Xaron11/symbolon}/tree/$name" >"$dir/SOURCE.txt"

cd dist
if [[ "$target" == *windows* ]]; then
    7z a "$pkg.zip" "$pkg" >/dev/null
else
    tar czf "$pkg.tar.gz" "$pkg"
fi
rm -rf "$pkg"
ls "$pkg".*
