#!/usr/bin/env python3
"""Fail when native source or SVG assets author colors outside ui/tokens.rs."""

from pathlib import Path
import re
import sys


APP = Path(__file__).resolve().parents[1]
TOKENS = APP / "src/ui/tokens.rs"
LITERAL = re.compile(
    r"\b0x[0-9a-fA-F]{6,8}\b|#[0-9a-fA-F]{3,8}\b|"
    r"\b(?:rgb|rgba|hsl|hsla)\s*\(\s*(?:0x[0-9a-fA-F]+|[0-9])"
)


def main() -> int:
    offenders = []
    for path in sorted((*APP.glob("src/**/*.rs"), *APP.glob("assets/**/*.svg"))):
        if path == TOKENS:
            continue
        for number, line in enumerate(path.read_text().splitlines(), 1):
            if LITERAL.search(line):
                offenders.append(f"{path.relative_to(APP)}:{number}: {line.strip()}")
    if offenders:
        print("Color literals belong in src/ui/tokens.rs:\n" + "\n".join(offenders))
        return 1
    print("Native colors are centralized in src/ui/tokens.rs")
    return 0


if __name__ == "__main__":
    sys.exit(main())
