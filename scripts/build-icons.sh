#!/usr/bin/env bash
# Regenerate raster app icons from the SVG source of truth.
# Run: nix develop .#test -c bash scripts/build-icons.sh
set -euo pipefail
cd "$(dirname "$0")/.."

magick -background none -density 384 RSVG:assets/paint-10.svg \
  -resize 256x256 -strip -define png:exclude-chunks=date,time \
  -define png:compression-level=9 PNG32:assets/paint-10.png

# Render the macOS icon source directly from the vector, without upscaling.
magick -background none -density 384 RSVG:assets/paint-10.svg \
  -resize 1024x1024 -strip -define png:exclude-chunks=date,time \
  -define png:compression-level=9 PNG32:assets/paint-10-1024.png

magick assets/paint-10.png -define icon:auto-resize=256,128,64,48,32,24,16 \
  -strip assets/paint-10.ico

echo 'Regenerated the 256px and 1024px PNG icons and Windows ICO from assets/paint-10.svg.'
