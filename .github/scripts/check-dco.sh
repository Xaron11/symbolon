#!/usr/bin/env bash
# Fails if a non-merge commit in BASE..HEAD lacks a DCO sign-off by its author (ADR-0020).
# Bots are exempt. Prints the offending commits.
# Usage: check-dco.sh BASE [HEAD]
set -euo pipefail

base=${1:?usage: check-dco.sh BASE [HEAD]}
head=${2:-HEAD}

missing=()
while read -r sha; do
    email=$(git log -1 --format=%ae "$sha")
    [[ "$email" == *'[bot]@users.noreply.github.com' ]] && continue
    if ! git log -1 --format='%(trailers:key=Signed-off-by,valueonly)' "$sha" | grep -qiF "<$email>"; then
        missing+=("$(git log -1 --format='%h %s' "$sha")")
    fi
done < <(git rev-list --no-merges "$base..$head")

if ((${#missing[@]})); then
    echo "These commits have no 'Signed-off-by:' with their author's email:"
    printf '  %s\n' "${missing[@]}"
    echo "Sign off with 'git commit -s'; fix a branch with 'git rebase --signoff $base' (see CONTRIBUTING.md)."
    exit 1
fi
echo "Every commit is signed off."
