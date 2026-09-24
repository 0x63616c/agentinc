#!/usr/bin/env python3
"""Place GPUI Pilot captures in a retina Mac-style frame and redact local identity.

Usage: python3 scripts/compose-readme-screenshots.py .local/readme-raw docs/assets/readme
Requires Pillow. Raw Pilot captures must be 2720×1656 PNGs from the 1360×828 app
window at scale 2. The script replaces the app's static workspace title and the
macOS account photo/name, then writes optimized PNGs below 1 MB each.
"""
import argparse
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont

NAMES = ("hero", "tickets", "automations", "evee", "settings")
FONT = "/System/Library/Fonts/SFNS.ttf"
ICON = Path(__file__).resolve().parents[1] / "crates/ainc-mac/assets/AppIcon.icns"
CANVAS = (3200, 2070)
WINDOW = (220, 142, 2980, 1892)
CONTENT = (240, 222)
MAX_BYTES = 1_000_000


def font(size):
    return ImageFont.truetype(FONT, size)


def redact_local_identity(image):
    """Replace the two local identity areas with obvious synthetic labels."""
    draw = ImageDraw.Draw(image)
    sidebar = image.getpixel((200, 130))
    # Workspace title (the current app has a hard-coded personal title).
    draw.rectangle((28, 127, 424, 191), fill=sidebar)
    draw.rounded_rectangle((38, 137, 88, 187), radius=11, fill="#242424",
                           outline="#444444", width=2)
    draw.text((63, 162), "D", font=font(27), fill="#e9e9e9", anchor="mm")
    draw.text((105, 164), "Demo Workspace", font=font(27),
              fill="#e9e9e9", anchor="lm")
    # The real macOS account name and photo must never enter committed assets.
    profile_fill = image.getpixel((300, 1585))
    draw.rounded_rectangle((24, 1540, 408, 1644), radius=15, fill=profile_fill)
    draw.ellipse((46, 1569, 100, 1623), fill="#353535")
    draw.text((73, 1596), "D", font=font(27), fill="#f1f1f1", anchor="mm")
    draw.text((111, 1597), "Demo Profile", font=font(27),
              fill="#e9e9e9", anchor="lm")
    return image


def wallpaper():
    base = Image.new("RGBA", CANVAS, "#222b38")
    glow = Image.new("RGBA", CANVAS, (0, 0, 0, 0))
    draw = ImageDraw.Draw(glow)
    draw.ellipse((-650, -800, 2140, 1280), fill=(137, 151, 166, 110))
    draw.ellipse((1050, 650, 4000, 2920), fill=(75, 94, 123, 125))
    draw.ellipse((-900, 1190, 1500, 3000), fill=(109, 104, 126, 85))
    base.alpha_composite(glow.filter(ImageFilter.GaussianBlur(220)))
    return base


def compose(source):
    app = Image.open(source).convert("RGBA")
    if app.size != (2720, 1656):
        raise ValueError(f"{source}: expected a retina 2720×1656 Pilot capture")
    redact_local_identity(app)
    result = wallpaper()

    shadow = Image.new("RGBA", CANVAS, (0, 0, 0, 0))
    ImageDraw.Draw(shadow).rounded_rectangle(
        (WINDOW[0] + 5, WINDOW[1] + 28, WINDOW[2] + 5, WINDOW[3] + 28),
        radius=36, fill=(0, 0, 0, 175),
    )
    result.alpha_composite(shadow.filter(ImageFilter.GaussianBlur(70)))

    frame = Image.new("RGBA", CANVAS, (0, 0, 0, 0))
    frame_draw = ImageDraw.Draw(frame)
    frame_draw.rounded_rectangle(WINDOW, radius=36, fill="#151719",
                                 outline="#5a6068", width=3)
    # Traffic-light placement evokes a Mac window without changing the app pixels.
    for x, color in ((274, "#757c82"), (318, "#757c82"), (362, "#757c82")):
        frame_draw.ellipse((x, 169, x + 22, 191), fill=color)
    icon = Image.open(ICON).convert("RGBA").resize((42, 42), Image.Resampling.LANCZOS)
    frame.alpha_composite(icon, (1518, 159))
    frame_draw.text((1572, 181), "AgentInc", font=font(25),
                    fill="#aeb4ba", anchor="lm")
    frame.alpha_composite(app, CONTENT)
    # Clip the assembled frame to the outer rounded window.
    mask = Image.new("L", CANVAS, 0)
    ImageDraw.Draw(mask).rounded_rectangle(WINDOW, radius=36, fill=255)
    frame.putalpha(mask)
    result.alpha_composite(frame)
    return result.convert("RGB")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("raw", type=Path, help="directory of raw GPUI Pilot PNGs")
    parser.add_argument("output", type=Path, help="directory for README PNGs")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    for name in NAMES:
        image = compose(args.raw / f"{name}.png")
        destination = args.output / f"{name}.png"
        image.save(destination, format="PNG", optimize=True, compress_level=9)
        size = destination.stat().st_size
        if size >= MAX_BYTES:
            destination.unlink()
            raise RuntimeError(f"{name}: {size} bytes exceeds 1 MB")
        print(f"{destination}: {size:,} bytes")


if __name__ == "__main__":
    main()
