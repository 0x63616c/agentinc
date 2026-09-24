#!/bin/sh
set -eu
cd "$(dirname "$0")/.."

# Keep the original Evee artwork and black canvas; center its visible bounds.
magick assets/evee.png -resize 135% -gravity center \
  -crop 1254x1254-24-12 +repage assets/AppIcon.png

iconset=$(mktemp -d)/AppIcon.iconset
mkdir -p "$iconset"
trap 'rm -rf "$(dirname "$iconset")"' EXIT
for size in 16 32 128 256 512; do
  magick assets/AppIcon.png -resize "${size}x${size}" "$iconset/icon_${size}x${size}.png"
  double=$((size * 2))
  magick assets/AppIcon.png -resize "${double}x${double}" "$iconset/icon_${size}x${size}@2x.png"
done
iconutil -c icns "$iconset" -o assets/AppIcon.icns
