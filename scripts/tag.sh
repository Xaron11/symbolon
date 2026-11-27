#!/usr/bin/env bash
# After the release PR is merged: creates and pushes the signed tag for the current version.
set -euo pipefail

die() {
    echo "error: $*" >&2
    exit 1
}

git switch --quiet main
git pull --quiet --ff-only origin main
git fetch --quiet origin --tags
version="$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name == "symbolon-core") | .version')"
tag="v$version"
git rev-parse -q --verify "refs/tags/$tag" >/dev/null && die "tag $tag already exists"
git log -1 --format=%s | grep -Eq "^chore\(release\): $tag( \(#[0-9]+\))?$" ||
    die "HEAD is not the release commit for $tag"

git tag -s "$tag" -m "Symbolon $tag"
git -c gpg.ssh.allowedSignersFile=.github/allowed_signers tag -v "$tag"
git push origin "$tag"
