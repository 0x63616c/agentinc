#!/usr/bin/env python3
"""Fail when native source or SVG assets author colors outside ui/tokens.rs.

Rust: no hex or numeric color literals, no `hsla(...)`, no named gpui colors such as
`white()`, no `Rgba { .. }` / `Hsla { .. }` literals. Use `rgb(TOKEN)`.
SVG: paints must be `none`, `currentColor` or a `url(#...)` reference, so every icon is
tinted by the app's token colors. Luminance masks (`<mask>`) are exempt: white/black there
select shape, not color.
Clippy's `disallowed-methods` (clippy.toml) bans the same gpui constructors at compile time.
"""

from pathlib import Path
import re
import sys


APP = Path(__file__).resolve().parents[1]
TOKENS = APP / "src/ui/tokens.rs"
RUST_LITERAL = re.compile(
    r"\b0x[0-9a-fA-F]{6,8}\b|#[0-9a-fA-F]{3,8}\b|"
    r"\b(?:rgb|rgba|hsl|hsla)\s*\(\s*(?:0x[0-9a-fA-F]+|[0-9])|"
    r"\bhsla?\s*\(|"
    r"\b(?:white|black|red|green|blue|yellow|opaque_grey|transparent_black|transparent_white)\s*\(|"
    r"(?:[=(,]|\breturn)\s*(?:gpui::)?(?:Rgba|Hsla)\s*\{"
)
SVG_PAINT = re.compile(
    r"""\b(?:fill|stroke|stop-color|flood-color|lighting-color|color)\s*[=:]\s*["']?\s*([^"';>\s]+)"""
)
SVG_HEX = re.compile(r"#[0-9a-fA-F]{3,8}\b")
SVG_ALLOWED = {"none", "currentColor", "inherit", "transparent"}
MASK = re.compile(r"<mask\b.*?</mask>", re.S)


def rust_violations(text: str) -> list[tuple[int, str]]:
    return [
        (number, line.strip())
        for number, line in enumerate(text.splitlines(), 1)
        if RUST_LITERAL.search(line)
    ]


def svg_violations(text: str) -> list[tuple[int, str]]:
    # Blank the masks but keep line numbers.
    text = MASK.sub(lambda m: "\n" * m.group(0).count("\n"), text)
    found = []
    for number, line in enumerate(text.splitlines(), 1):
        bad = [
            value
            for value in SVG_PAINT.findall(line)
            if value not in SVG_ALLOWED and not value.startswith("url(")
        ]
        if bad or SVG_HEX.search(line):
            found.append((number, line.strip()))
    return found


def main() -> int:
    offenders = []
    for path in sorted(APP.glob("src/**/*.rs")):
        if path != TOKENS:
            offenders += [
                f"{path.relative_to(APP)}:{n}: {line}"
                for n, line in rust_violations(path.read_text())
            ]
    for path in sorted(APP.glob("assets/**/*.svg")):
        offenders += [
            f"{path.relative_to(APP)}:{n}: {line[:120]}"
            for n, line in svg_violations(path.read_text())
        ]
    if offenders:
        print(
            "Colors belong in src/ui/tokens.rs; SVGs must paint with currentColor:\n"
            + "\n".join(offenders)
        )
        return 1
    print("Native colors are centralized in src/ui/tokens.rs")
    return 0


if __name__ == "__main__":
    sys.exit(main())
