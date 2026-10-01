import importlib.util
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location("check_colors", Path(__file__).with_name("check-colors.py"))
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)


class RustColors(unittest.TestCase):
    def test_bans_invented_colors(self):
        for line in [
            "div().bg(rgb(0xffffff))",
            "div().bg(rgba(0x00000080))",
            "let c = hsla(0., 0., 1., 1.);",
            "div().text_color(gpui::white())",
            "let c = Rgba { r: 1., g: 1., b: 1., a: 1. };",
            'let c = "#fff";',
        ]:
            self.assertTrue(check.rust_violations(line), line)

    def test_allows_tokens(self):
        for line in [
            "div().bg(rgb(SURFACE))",
            "let c = rgba(SCRIM);",
            "fn blend(from: u32, to: u32, t: f32) -> Rgba {",
            "rgb((channel(16) << 16) | (channel(8) << 8) | channel(0))",
        ]:
            self.assertFalse(check.rust_violations(line), line)


class SvgColors(unittest.TestCase):
    def test_bans_painted_colors(self):
        for text in ['<path fill="white"/>', '<path stroke="#fff"/>', '<g style="fill: red"/>']:
            self.assertTrue(check.svg_violations(text), text)

    def test_allows_current_color_and_masks(self):
        self.assertFalse(check.svg_violations('<path fill="none" stroke="currentColor"/>'))
        self.assertFalse(
            check.svg_violations(
                '<mask id="m"><rect fill="white"/><path stroke="black"/></mask><circle fill="currentColor" mask="url(#m)"/>'
            )
        )


if __name__ == "__main__":
    unittest.main()
