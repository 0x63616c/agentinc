#!/usr/bin/env python3
"""Place a real GPUI Pilot app render on a vivid gradient.

The layout follows the open-source Tokokino screenshot composer's background
and padding approach (https://github.com/ShivaBhattacharjee/Tokokino). The
macOS controls and corner mask come from a repository native-window capture;
no extra title bar is added above the actual app pixels.
Requires Pillow: python3 scripts/compose-readme-screenshots.py RAW.png OUTPUT.png
"""
import argparse
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

CANVAS = (3200, 2070)
MAX_BYTES = 1_000_000
STOPS = ((0.0, (255, 214, 92)), (0.46, (255, 104, 130)), (1.0, (181, 71, 233)))
NATIVE_REFERENCE = (Path(__file__).resolve().parents[1] /
                    "crates/ainc-mac/docs/verification/native-capture-library.png")


def background():
    image = Image.new("RGB", CANVAS)
    draw = ImageDraw.Draw(image)
    for y in range(CANVAS[1]):
        position = y / (CANVAS[1] - 1)
        for (start, first), (end, last) in zip(STOPS, STOPS[1:]):
            if position <= end:
                fraction = (position - start) / (end - start)
                color = tuple(round(a + (b - a) * fraction) for a, b in zip(first, last))
                draw.line((0, y, CANVAS[0], y), fill=color)
                break
    return image


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("raw", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    app = Image.open(args.raw).convert("RGBA")
    if app.size != (2720, 1656):
        raise ValueError(f"Expected a 2720×1656 retina Pilot render, got {app.size}")
    if app.width > CANVAS[0] - 200 or app.height > CANVAS[1] - 200:
        raise ValueError(f"Capture lacks padding on {CANVAS}: {app.size}")
    native = Image.open(NATIVE_REFERENCE).convert("RGBA")
    if native.width != app.width:
        raise ValueError("Native reference width differs from Pilot render")
    # The app and reference use the same #0c0c0c top bar. Copy only the actual
    # macOS traffic-light region at the configured 18 pt window offset.
    controls = (26, 30, 163, 71)
    if app.getpixel((20, 20)) != native.getpixel((20, 20)):
        raise ValueError("Native control background no longer matches app header")
    app.paste(native.crop(controls), controls[:2])
    # Reuse the alpha of a real native window at the same retina width, with
    # its exact top/bottom corner contours. The middle rows remain opaque.
    mask = Image.new("L", app.size, 255)
    alpha = native.getchannel("A")
    edge = 80
    mask.paste(alpha.crop((0, 0, app.width, edge)), (0, 0))
    mask.paste(alpha.crop((0, native.height - edge, app.width, native.height)),
               (0, app.height - edge))
    app.putalpha(mask)
    image = background().convert("RGBA")
    placement = ((CANVAS[0] - app.width) // 2, (CANVAS[1] - app.height) // 2)
    shadow = Image.new("RGBA", CANVAS)
    shadow_mask = Image.new("L", CANVAS)
    shadow_mask.paste(mask, (placement[0], placement[1] + 34))
    shadow.putalpha(shadow_mask.filter(ImageFilter.GaussianBlur(52)).point(
        lambda value: round(value * 0.48)))
    image.alpha_composite(shadow)
    image.alpha_composite(app, placement)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    image.convert("RGB").save(args.output, format="PNG", optimize=True, compress_level=9)
    size = args.output.stat().st_size
    if size >= MAX_BYTES:
        args.output.unlink()
        raise RuntimeError(f"Hero PNG is {size:,} bytes (limit {MAX_BYTES:,})")
    print(f"{args.output}: {size:,} bytes")


if __name__ == "__main__":
    main()
