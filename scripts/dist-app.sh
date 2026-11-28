#!/usr/bin/env bash
# Builds the desktop app's packages (ADR-0026) into dist/: the AppImage and .deb on Linux, the NSIS
# installer on Windows. With the agent, they carry it as a resource (tauri.release.conf.json).
# Usage: dist-app.sh <bundles> <name> <with agent: true|false> <agent binary>
set -euo pipefail

bundles="$1"
name="$2"
with_agent="$3"
agent="$4"

config=()
if [[ "$with_agent" == true ]]; then
    # CI artifacts lose the executable bit, and the package installs the file as it finds it.
    chmod 755 "$agent"
    config=(--config tauri.release.conf.json)
fi
(cd crates/symbolon-app && cargo tauri build --ci --bundles "$bundles" "${config[@]}")

bundle=target/release/bundle
mkdir -p dist
if [[ "$bundles" == *nsis* ]]; then
    cp "$bundle"/nsis/*-setup.exe "dist/symbolon-app-$name-x86_64-setup.exe"
fi
if [[ "$bundles" == *appimage* ]]; then
    cp "$bundle"/appimage/*.AppImage "dist/symbolon-app-$name-x86_64.AppImage"
fi
if [[ "$bundles" == *deb* ]]; then
    deb="dist/symbolon-app-$name-x86_64.deb"
    cp "$bundle"/deb/*.deb "$deb"
    # The app looks for the agent here; a release package without it scans phones slowly.
    if [[ "$with_agent" == true ]]; then
        dpkg-deb --contents "$deb" | grep -q 'usr/lib/Symbolon/symbolon-agent-aarch64-linux-musl'
    fi
fi
ls dist/symbolon-app-"$name"-*
