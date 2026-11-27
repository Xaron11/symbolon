#!/usr/bin/env bash
# Opens a release PR from an up-to-date main (ADR-0007). The version defaults to the one implied by
# the commits since the last tag.
# Usage: release.sh <frontend directory> [version]
set -euo pipefail

die() {
    echo "error: $*" >&2
    exit 1
}

ui="$1"
version="${2:-}"
[[ -z "$(git status --porcelain)" ]] || die "working tree is not clean"
git fetch --quiet origin main --tags
[[ "$(git rev-parse HEAD)" == "$(git rev-parse origin/main)" ]] || die "run this on main, in sync with origin/main"
version="${version:-$(git cliff --bumped-version)}"
version="${version#v}"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "not a SemVer version: $version"
git rev-parse -q --verify "refs/tags/v$version" >/dev/null && die "tag v$version already exists"

git switch -c "release/v$version"
sed -i "0,/^version = \".*\"/s//version = \"$version\"/" Cargo.toml
cargo update --workspace --quiet
(cd "$ui" && npm pkg set version="$version")
git cliff --tag "v$version" -o CHANGELOG.md
treefmt CHANGELOG.md "$ui/package.json"
git commit --quiet -am "chore(release): v$version"
git push --quiet -u origin HEAD
gh pr create --title "chore(release): v$version" \
    --body "Release v$version. After merging, run \`just tag\` on main."
