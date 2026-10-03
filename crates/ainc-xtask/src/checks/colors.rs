//! Fail when native source or SVG assets author colors outside `ui/tokens.rs`.
//!
//! Rust: no hex or numeric color literals, no `hsla(...)`, no named gpui colors such as
//! `white()`, no `Rgba { .. }` / `Hsla { .. }` literals. Use `rgb(TOKEN)`.
//! SVG: paints must be `none`, `currentColor` or a `url(#...)` reference, so every icon is
//! tinted by the app's token colors. Luminance masks (`<mask>`) are exempt: white/black there
//! select shape, not color.
//! Clippy's `disallowed-methods` (clippy.toml) bans the same gpui constructors at compile time.
use super::files;
use anyhow::{Result, bail};
use regex::Regex;
use std::{fs, path::Path};

const SVG_ALLOWED: [&str; 4] = ["none", "currentColor", "inherit", "transparent"];

fn rust_literal() -> Regex {
    Regex::new(concat!(
        r"\b0x[0-9a-fA-F]{6,8}\b|#[0-9a-fA-F]{3,8}\b|",
        r"\b(?:rgb|rgba|hsl|hsla)\s*\(\s*(?:0x[0-9a-fA-F]+|[0-9])|",
        r"\bhsla?\s*\(|",
        r"\b(?:white|black|red|green|blue|yellow|opaque_grey|transparent_black|transparent_white)\s*\(|",
        r"(?:[=(,]|\breturn)\s*(?:gpui::)?(?:Rgba|Hsla)\s*\{"
    ))
    .unwrap()
}

fn rust_violations(text: &str) -> Vec<(usize, String)> {
    let literal = rust_literal();
    text.lines()
        .enumerate()
        .filter(|(_, line)| literal.is_match(line))
        .map(|(i, line)| (i + 1, line.trim().to_string()))
        .collect()
}

fn svg_violations(text: &str) -> Vec<(usize, String)> {
    let paint = Regex::new(
        r#"\b(?:fill|stroke|stop-color|flood-color|lighting-color|color)\s*[=:]\s*["']?\s*([^"';>\s]+)"#,
    )
    .unwrap();
    let hex = Regex::new(r"#[0-9a-fA-F]{3,8}\b").unwrap();
    let mask = Regex::new(r"(?s)<mask\b.*?</mask>").unwrap();
    // Blank the masks but keep line numbers.
    let text = mask.replace_all(text, |m: &regex::Captures| {
        "\n".repeat(m[0].matches('\n').count())
    });
    text.lines()
        .enumerate()
        .filter(|(_, line)| {
            let bad = paint.captures_iter(line).any(|c| {
                let value = &c[1];
                !SVG_ALLOWED.contains(&value) && !value.starts_with("url(")
            });
            bad || hex.is_match(line)
        })
        .map(|(i, line)| (i + 1, line.trim().chars().take(120).collect()))
        .collect()
}

pub fn run(app: &Path) -> Result<()> {
    let tokens = app.join("src/ui/tokens.rs");
    let mut offenders = Vec::new();
    let relative = |path: &Path| path.strip_prefix(app).unwrap_or(path).display().to_string();
    for path in files(&app.join("src"), "rs")? {
        if path != tokens {
            for (n, line) in rust_violations(&fs::read_to_string(&path)?) {
                offenders.push(format!("{}:{n}: {line}", relative(&path)));
            }
        }
    }
    for path in files(&app.join("assets"), "svg")? {
        for (n, line) in svg_violations(&fs::read_to_string(&path)?) {
            offenders.push(format!("{}:{n}: {line}", relative(&path)));
        }
    }
    if !offenders.is_empty() {
        bail!(
            "Colors belong in src/ui/tokens.rs; SVGs must paint with currentColor:\n{}",
            offenders.join("\n")
        );
    }
    println!("Native colors are centralized in src/ui/tokens.rs");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bans_invented_colors() {
        for line in [
            "div().bg(rgb(0xffffff))",
            "div().bg(rgba(0x00000080))",
            "let c = hsla(0., 0., 1., 1.);",
            "div().text_color(gpui::white())",
            "let c = Rgba { r: 1., g: 1., b: 1., a: 1. };",
            r##"let c = "#fff";"##,
        ] {
            assert!(!rust_violations(line).is_empty(), "{line}");
        }
    }

    #[test]
    fn allows_tokens() {
        for line in [
            "div().bg(rgb(SURFACE))",
            "let c = rgba(SCRIM);",
            "fn blend(from: u32, to: u32, t: f32) -> Rgba {",
            "rgb((channel(16) << 16) | (channel(8) << 8) | channel(0))",
        ] {
            assert!(rust_violations(line).is_empty(), "{line}");
        }
    }

    #[test]
    fn bans_painted_svg_colors() {
        for text in [
            r#"<path fill="white"/>"#,
            r##"<path stroke="#fff"/>"##,
            r#"<g style="fill: red"/>"#,
        ] {
            assert!(!svg_violations(text).is_empty(), "{text}");
        }
    }

    #[test]
    fn allows_current_color_and_masks() {
        assert!(svg_violations(r#"<path fill="none" stroke="currentColor"/>"#).is_empty());
        assert!(
            svg_violations(
                r##"<mask id="m"><rect fill="white"/><path stroke="black"/></mask><circle fill="currentColor" mask="url(#m)"/>"##
            )
            .is_empty()
        );
    }
}
