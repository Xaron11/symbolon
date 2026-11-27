#!/usr/bin/env bash
# Packs the Android agent with its licenses into dist/symbolon-agent-<name>-aarch64-linux-musl.tar.gz.
# Usage: dist-agent.sh <name> <agent binary>
set -euo pipefail

name="$1"
agent="$2"
pkg="symbolon-agent-$name-aarch64-linux-musl"
dir="dist/$pkg"
rm -rf "$dir"
mkdir -p "$dir"
cp "$agent" LICENSE-MIT LICENSE-APACHE "$dir/"
echo "Corresponding source: ${GITHUB_SERVER_URL:-https://github.com}/${GITHUB_REPOSITORY:-Xaron11/symbolon}/tree/$name" >"$dir/SOURCE.txt"

cd dist
tar czf "$pkg.tar.gz" "$pkg"
rm -rf "$pkg"
ls "$pkg.tar.gz"
