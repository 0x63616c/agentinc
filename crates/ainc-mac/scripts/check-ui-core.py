#!/usr/bin/env python3
"""Reject new native component forks outside the app-owned ui module."""

from collections import Counter
import json
from pathlib import Path
import re
import sys

APP = Path(__file__).resolve().parents[1]
BASELINE = APP / "scripts/ui-core-baseline.json"
RULES = {
    "local constructor": (
        re.compile(r"\bfn\s+(?:button|field)\s*\("),
        "use ui::action_button or ui::text_field",
    ),
    "raw button role": (
        re.compile(r"\bRole::Button\b"),
        "use ui::action_button for focus, disabled, and keyboard behavior",
    ),
    "raw RGB": (
        re.compile(r"\brgb\s*\(\s*0x[0-9a-fA-F]+"),
        "add a color role to ui::tokens and use that role",
    ),
}


def violations(source: str, allowed: dict[str, int]):
    seen = Counter()
    for number, line in enumerate(source.splitlines(), 1):
        for rule, (pattern, _) in RULES.items():
            for _ in pattern.finditer(line):
                seen[rule] += 1
                if seen[rule] > allowed.get(rule, 0):
                    yield number, rule


def main() -> int:
    if sys.argv[1:] == ["--self-test"]:
        assert list(violations("fn button() {}\nfn button() {}", {"local constructor": 1})) == [
            (2, "local constructor")
        ]
        assert list(violations(".role(Role::Button)\nrgb(0xff00ff)", {})) == [
            (1, "raw button role"),
            (2, "raw RGB"),
        ]
        return 0
    baseline = json.loads(BASELINE.read_text())
    errors = []
    for path in sorted((APP / "src").rglob("*.rs")):
        if path.is_relative_to(APP / "src/ui"):
            if path.name != "tokens.rs":
                for number, line in enumerate(path.read_text().splitlines(), 1):
                    if RULES["raw RGB"][0].search(line):
                        errors.append(
                            f"{path.relative_to(APP)}:{number}: raw RGB; {RULES['raw RGB'][1]}"
                        )
            continue
        relative = str(path.relative_to(APP))
        for number, rule in violations(path.read_text(), baseline.get(relative, {})):
            errors.append(f"{relative}:{number}: {rule}; {RULES[rule][1]}")
    if errors:
        print("New native UI forks:\n" + "\n".join(errors))
        return 1
    print("Native UI core ratchet passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
