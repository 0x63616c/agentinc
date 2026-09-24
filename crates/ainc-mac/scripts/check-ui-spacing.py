#!/usr/bin/env python3
"""Warn when migrated native UI files gain raw numeric spacing calls."""

from collections import Counter
import json
from pathlib import Path
import re
import sys


APP = Path(__file__).resolve().parents[1]
BASELINE = APP / "scripts/ui-spacing-baseline.json"
SPACING = re.compile(
    r"\.(gap|p|px|py|pl|pr|pt|pb|m|mx|my|ml|mr|mt|mb)"
    r"\(\s*px\(\s*(-?\d+(?:\.\d*)?)\s*\)\s*\)"
)


def preferred(method: str) -> str:
    if method == "gap":
        return "CONTROL_GAP, FIELD_LABEL_GAP, or FORM_STACK_GAP"
    if method in {"px", "pl", "pr"}:
        return "CONTROL_INSET_X, FIELD_INSET_X, or a named optical inset"
    return "PAGE_X, RIGHT_PANE_CONTENT_INSET, or a named inset token"


def new_literals(source: str, allowed: dict[str, int]):
    seen = Counter()
    for number, line in enumerate(source.splitlines(), 1):
        for match in SPACING.finditer(line):
            method, value = match.groups()
            key = f"{method}:{value}"
            seen[key] += 1
            if seen[key] > allowed.get(key, 0):
                yield number, method, value


def main() -> int:
    if sys.argv[1:] == ["--self-test"]:
        assert list(new_literals(".gap(px(8.)).gap(px(8.))", {"gap:8.": 1})) == [
            (1, "gap", "8.")
        ]
        assert not list(new_literals(".gap(px(CONTROL_GAP))", {}))
        return 0
    baseline = json.loads(BASELINE.read_text())
    warnings = 0
    for name, allowed in baseline.items():
        for number, method, value in new_literals((APP / name).read_text(), allowed):
            warnings += 1
            print(
                f"::warning file=crates/ainc-mac/{name},line={number}::"
                f"Raw {method} spacing {value}px; prefer {preferred(method)} "
                "from style.rs. Name an optical exception there if needed."
            )
    print(f"UI spacing ratchet: {warnings} new raw literal warning(s)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
