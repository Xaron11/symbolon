#!/usr/bin/env bash
# Updates the flake inputs on a new branch and opens a PR (fallback if Dependabot's nix support is
# unavailable).
set -euo pipefail

[[ -z "$(git status --porcelain)" ]] || {
    echo "error: working tree is not clean" >&2
    exit 1
}
git switch -c "build/flake-update-$(date +%F)" origin/main
nix flake update --commit-lock-file --option commit-lockfile-summary "build(nix): update flake inputs"
git push -u origin HEAD
gh pr create --fill
