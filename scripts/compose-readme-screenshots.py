#!/usr/bin/env python3
"""Place a native AgentInc window capture on a vivid gradient.

The layout follows the open-source Tokokino screenshot composer's background
and padding approach (https://github.com/ShivaBhattacharjee/Tokokino).
Only the transparent space around the actual WindowServer capture is filled.
Requires Pillow: python3 scripts/compose-readme-screenshots.py RAW.png OUTPUT.png
"""
import argparse
from pathlib import Path

from PIL import Image, ImageDraw

CANVAS = (3200, 2070)
MAX_BYTES = 1_000_000
STOPS = ((0.0, (255, 214, 92)), (0.46, (255, 104, 130)), (1.0, (181, 71, 233)))


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
    if app.width < 2600 or app.height < 1600:
        raise ValueError(f"Expected a retina native window capture, got {app.size}")
    if app.getchannel("A").getextrema()[0] == 255:
        raise ValueError("Capture has no transparent native corners or shadow")
    if app.width > CANVAS[0] - 200 or app.height > CANVAS[1] - 200:
        raise ValueError(f"Capture lacks padding on {CANVAS}: {app.size}")
    image = background()
    image.paste(app, ((CANVAS[0] - app.width) // 2, (CANVAS[1] - app.height) // 2), app)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    image.save(args.output, format="PNG", optimize=True, compress_level=9)
    size = args.output.stat().st_size
    if size >= MAX_BYTES:
        args.output.unlink()
        raise RuntimeError(f"Hero PNG is {size:,} bytes (limit {MAX_BYTES:,})")
    print(f"{args.output}: {size:,} bytes")


if __name__ == "__main__":
    main()
