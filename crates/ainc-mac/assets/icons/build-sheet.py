#!/usr/bin/env python3
"""Validate the SVG set and rebuild its self-contained review sheet. Stdlib only."""
from html import escape
from pathlib import Path
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parent
GROUPS = {
    "Spaces": "today tasks agents home calendar library apps evee".split(),
    "Shell": "plus close sidebar-toggle evee-panel search settings profile bell".split(),
    "Actions": "check checkbox arrow-right arrow-up arrow-up-right chevron-left chevron-right return".split(),
    "Home & media": "moon coffee lamp temperature play pause next-track volume".split(),
    "Work & collections": "branch refresh music book folder cloud spark more".split(),
}
NAMES = [name for names in GROUPS.values() for name in names]
assert len(NAMES) == len(set(NAMES)), "Duplicate icon in sheet"
assert set(NAMES) == {p.stem for p in ROOT.glob("*.svg")}, "Sheet must cover every SVG"
ICONS = {}
for name in NAMES:
    source = (ROOT / f"{name}.svg").read_text()
    svg = ET.fromstring(source)
    for key, value in {
        "viewBox": "0 0 24 24", "width": "24", "height": "24",
        "fill": "none", "stroke": "currentColor", "stroke-width": "1.5",
        "stroke-linecap": "round", "stroke-linejoin": "round",
    }.items():
        assert svg.get(key) == value, f"{name}: incorrect {key}"
    for element in svg.iter():
        assert element.tag.split("}")[-1] in {"svg", "g", "path", "rect", "circle", "ellipse"}, name
        assert not any(key.startswith("on") or "href" in key for key in element.attrib), name
    ICONS[name] = source.replace("<svg ", '<svg aria-hidden="true" ')

sections = "".join(
    f'<section><h2>{escape(title)}</h2><div class="grid">' + "".join(
        f'<figure data-icon="{name}">{ICONS[name]}<figcaption>{name}</figcaption></figure>'
        for name in names
    ) + '</div></section>' for title, names in GROUPS.items()
)
small = "".join(f'<span>{ICONS[name]}{label}</span>' for name, label in [
    ("today", "Today"), ("tasks", "Tasks"), ("home", "Home"), ("library", "Library"),
])
html = '''<!doctype html>
<html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Control icons · First edition</title>
<style>
*{box-sizing:border-box}html{color-scheme:dark;background:#040404}body{margin:0;color:#ededed;font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;-webkit-font-smoothing:antialiased}
main{width:1440px;padding:48px 64px 40px}header{display:flex;justify-content:space-between;align-items:end;padding-bottom:30px;border-bottom:1px solid #272727}
h1{font-size:36px;font-weight:500;letter-spacing:-1.3px;margin:0 0 10px}p{font-size:13px;color:#a0a0a0;margin:0;line-height:1.6}.edition{text-align:right;font-size:12px;line-height:1.8;color:#a0a0a0}
section{padding-top:22px;border-bottom:1px solid #202020}h2{margin:0;font-size:13px;font-weight:500;color:#a0a0a0}.grid{display:grid;grid-template-columns:repeat(8,1fr);gap:12px}figure{margin:0;height:126px;display:flex;align-items:center;justify-content:center;flex-direction:column;gap:20px}figure svg{width:32px;height:32px;color:#ededed}figure[data-icon=evee] svg{color:#d5ded8}figcaption{font-size:12px;color:#a0a0a0}
footer{padding-top:28px;display:grid;grid-template-columns:1fr 1fr;gap:18px}.sample{padding:20px 24px;border:1px solid #272727;border-radius:14px;background:#0c0c0c}.sample.reference{background:#101010;border-color:#333}.sample p{font-size:11px;color:#888;margin-bottom:18px}.samples{display:flex;gap:26px;align-items:center}.samples span{display:flex;align-items:center;gap:9px;font-size:12px;color:#a0a0a0}.samples svg{width:17px;height:17px;flex:none}.sizes{display:flex;align-items:center;gap:28px;color:#a0a0a0}.sizes span{display:flex;align-items:center;gap:10px;font-size:12px}.sizes svg{width:var(--size);height:var(--size)}.caption{display:flex;justify-content:space-between;padding-top:22px;font-size:11px;color:#888}
</style><main><header><div><h1>Control icons</h1><p>Rounded outlines. Quiet, familiar shapes. One shared vocabulary.</p></div><div class="edition">First edition · 40 glyphs<br>24 × 24 grid / 1.5 stroke / currentColor</div></header>
''' + sections + '''<footer><div class="sample"><p>Native shell · #0c0c0c · 17 px</p><div class="samples">''' + small + '''</div></div><div class="sample reference"><p>Reference workspace · #101010 · optical size check</p><div class="sizes">''' + "".join(
    f'<span style="--size:{size}px">{ICONS[name]}{size} px</span>'
    for size, name in [(14, "search"), (17, "calendar"), (20, "settings"), (24, "evee")]
) + '''</div></div></footer><div class="caption"><span>All glyphs above at 32 px on the native workspace · #040404</span><span>Source SVGs + extension rules in STYLE.md</span></div></main></html>
'''
(ROOT / "contact-sheet.html").write_text(html)
print(f"Validated {len(NAMES)} SVGs; rebuilt assets/icons/contact-sheet.html")
