#!/usr/bin/env bash
# Generates two locations, Laptop and Backup, and scans them into target/demo/catalog.db for trying
# the app. Every comparison category appears, with several extensions and sizes.
# Usage: demo.sh [number of files]
set -euo pipefail

files="${1:-5000}"
root="$PWD/target/demo"
rm -rf "$root"
mkdir -p "$root"/{Laptop,Backup}/d{0..49}

exts=(jpg png txt md pdf mp4 rs json)
# One file in 13 carries 0 to 36 KiB of padding, so the size chart has more than one bar.
kib="$(printf '%01024d' 0)"
pads=()
for ((k = 0; k < 7; k++)); do
    pad=""
    for ((j = 0; j < k * k; j++)); do pad+="$kib"; done
    pads+=("$pad")
done

for ((i = 0; i < files; i++)); do
    d="d$((i % 50))"
    ext="${exts[i % 8]}"
    name="f$i.$ext"
    body="file $i"
    ((i % 13 == 0)) && body+="${pads[i % 7]}"
    laptop="$root/Laptop/$d/$name"
    backup="$root/Backup/$d/$name"
    case $((i % 20)) in
        0) backup="" ;;
        1) laptop="" ;;
        2) printf '%s' "$body changed" >"$backup" && backup="" ;;
        3) backup="$root/Backup/d$(((i + 1) % 50))/$name" ;;
        4) backup="$root/Backup/$d/renamed-$name" ;;
        5) printf '%s' "$body" >"$root/Laptop/$d/f$i copy.$ext" ;;
    esac
    if [[ -n "$laptop" ]]; then printf '%s' "$body" >"$laptop"; fi
    if [[ -n "$backup" ]]; then printf '%s' "$body" >"$backup"; fi
done
head -c 3000000 /dev/zero >"$root/Laptop/d0/video.mp4"
cp "$root/Laptop/d0/video.mp4" "$root/Backup/d0/video.mp4"

cargo build --quiet -p symbolon-cli
symbolon() { target/debug/symbolon --catalog "$root/catalog.db" --quiet "$@"; }
symbolon location add Laptop "$root/Laptop" >/dev/null
symbolon location add Backup "$root/Backup" >/dev/null
symbolon scan Laptop Backup >/dev/null
echo "SYMBOLON_CATALOG=$root/catalog.db just app-dev"
