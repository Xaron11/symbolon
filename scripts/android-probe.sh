#!/usr/bin/env bash
# Pushes the agent, lists a directory on the device through it (NDJSON), then removes the binary.
# Usage: android-probe.sh <agent binary> <device directory> [serial]
set -euo pipefail

agent="$1"
dir="$2"
serial="${3:-}"
adb_args=()
[[ -n "$serial" ]] && adb_args=(-s "$serial")
remote=/data/local/tmp/symbolon-agent

# Remove the binary however the script ends, including a failed push or chmod.
trap 'adb "${adb_args[@]}" shell rm -f "$remote"' EXIT
adb "${adb_args[@]}" push "$agent" "$remote" >&2
adb "${adb_args[@]}" shell chmod 755 "$remote"
# adb joins its arguments into one device shell command line, so quote the directory.
adb "${adb_args[@]}" exec-out "$remote" walk "'${dir//\'/\'\\\'\'}'"
