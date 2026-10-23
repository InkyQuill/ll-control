#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
for theme in dark light; do
  rsvg-convert -w 64 -h 64 \
    "assets/branding/ll-control/vector-dial/ll-control-clean-on-$theme.svg" \
    -o "desktop/src-tauri/icons/tray-on-$theme.png"
done
# The launcher keeps the approved charcoal body, independent of panel appearance.
cp assets/branding/ll-control/vector-dial/ll-control-clean-on-light.svg desktop/static/ll-control.svg
rsvg-convert -w 256 -h 256 desktop/static/ll-control.svg -o desktop/src-tauri/icons/icon.png
cp desktop/src-tauri/icons/icon.png assets/icon.png
rsvg-convert -w 32 -h 32 desktop/static/ll-control.svg -o desktop/src-tauri/icons/32x32.png
