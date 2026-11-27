#!/usr/bin/env bash
# Fails unless the agent is fully static and its segments suit 4/16/64 KiB page kernels (ADR-0003).
# Usage: check-agent.sh <agent binary>
set -euo pipefail

f="$1"
if readelf -lW "$f" | grep -q INTERP; then
    echo "error: $f is dynamically linked" >&2
    exit 1
fi
readelf -lW "$f" | awk '$1 == "LOAD" { print $NF }' | while read -r align; do
    ((align >= 0x10000)) || {
        echo "error: LOAD alignment $align < 64 KiB" >&2
        exit 1
    }
done
echo "ok: static, LOAD alignment >= 64 KiB"
