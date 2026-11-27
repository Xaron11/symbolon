#!/usr/bin/env bash
# Prints the release notes for a tag: its changelog section, then where the source and checksums are.
# Before the tag exists, it previews the unreleased commits under that tag's name.
# Usage: release-notes.sh <tag>
set -euo pipefail

tag="$1"
if [[ "$(git tag --points-at HEAD)" == *"$tag"* ]]; then
    git cliff --current --strip all
else
    git cliff --unreleased --tag "$tag" --strip all
fi
echo
echo "Built from source tag [$tag](${GITHUB_SERVER_URL:-https://github.com}/${GITHUB_REPOSITORY:-Xaron11/symbolon}/tree/$tag) under MIT OR Apache-2.0. Verify downloads with \`sha256sum -c SHA256SUMS\`."
