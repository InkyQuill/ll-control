#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
# Source artwork supplied by InkyQuill in issue #34.
source=assets/branding/ll-control/ll-control-mono.svg
cp "$source" desktop/static/ll-control.svg
# Keep the supplied geometry legible on both panel appearances.
sed 's/#0F172A/#F1F5F9/g' "$source" | rsvg-convert -w 64 -h 64 -o desktop/src-tauri/icons/tray-on-dark.png
rsvg-convert -w 64 -h 64 "$source" -o desktop/src-tauri/icons/tray-on-light.png
rsvg-convert -w 256 -h 256 "$source" -o desktop/src-tauri/icons/icon.png
cp desktop/src-tauri/icons/icon.png assets/icon.png
rsvg-convert -w 32 -h 32 "$source" -o desktop/src-tauri/icons/32x32.png
