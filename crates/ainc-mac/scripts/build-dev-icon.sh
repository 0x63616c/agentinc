#!/bin/sh
set -eu
cd "$(dirname "$0")/.."

style=${1:-border}
output=${2:-assets/AppIconDev.icns}
case "$style" in border|banner) ;; *) echo 'usage: build-dev-icon.sh [border|banner] [output.icns]' >&2; exit 2 ;; esac

temporary=$(mktemp -d)
trap 'rm -rf "$temporary"' EXIT
cat > "$temporary/overlay.svg" <<'SVG'
<svg xmlns="http://www.w3.org/2000/svg" width="1254" height="1254" viewBox="0 0 1254 1254">
  <defs>
    <pattern id="hazard" width="110" height="110" patternUnits="userSpaceOnUse" patternTransform="rotate(35)">
      <rect width="110" height="110" fill="#F8C629"/>
      <rect width="52" height="110" fill="#17191A"/>
    </pattern>
  </defs>
  <rect x="64" y="64" width="1126" height="1126" rx="242" fill="none" stroke="url(#hazard)" stroke-width="78"/>
</svg>
SVG
if [ "$style" = banner ]; then
  cat > "$temporary/overlay.svg" <<'SVG'
<svg xmlns="http://www.w3.org/2000/svg" width="1254" height="1254" viewBox="0 0 1254 1254">
  <defs>
    <pattern id="hazard" width="110" height="110" patternUnits="userSpaceOnUse" patternTransform="rotate(35)">
      <rect width="110" height="110" fill="#F8C629"/>
      <rect width="52" height="110" fill="#17191A"/>
    </pattern>
    <clipPath id="icon"><rect x="40" y="40" width="1174" height="1174" rx="220"/></clipPath>
  </defs>
  <g clip-path="url(#icon)">
    <rect x="-150" y="515" width="1554" height="224" fill="url(#hazard)" transform="rotate(-25 627 627)"/>
  </g>
</svg>
SVG
fi

rsvg-convert "$temporary/overlay.svg" -o "$temporary/overlay.png"
magick assets/AppIcon.png "$temporary/overlay.png" -compose over -composite "$temporary/icon.png"
iconset="$temporary/AppIcon.iconset"
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
  magick "$temporary/icon.png" -filter Lanczos -resize "${size}x${size}" "$iconset/icon_${size}x${size}.png"
  double=$((size * 2))
  magick "$temporary/icon.png" -filter Lanczos -resize "${double}x${double}" "$iconset/icon_${size}x${size}@2x.png"
done
iconutil -c icns "$iconset" -o "$output"
printf 'Built %s (%s)\n' "$output" "$style"
