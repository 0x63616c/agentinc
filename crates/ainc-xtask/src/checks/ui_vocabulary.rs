//! One vocabulary for time, work state and shortcuts: formatters live in
//! `ui/time.rs`, wire state strings in `ui/work_state.rs`, shortcut glyphs
//! have no separators, and the README shortcut table is the one in
//! `ui/shortcuts.rs`.
use super::files;
use anyhow::{Context, Result, bail};
use regex::Regex;
use std::{fs, path::Path};

const README_START: &str = "<!-- shortcuts -->";
const README_END: &str = "<!-- /shortcuts -->";

/// `(pattern, the one file allowed to match it, advice)`.
fn rules() -> [(Regex, Option<&'static str>, &'static str); 3] {
    [
        (
            Regex::new(r#"\.format\("%"#).unwrap(),
            Some("src/ui/time.rs"),
            "format times through ui::time",
        ),
        (
            Regex::new(r#""(queued|running)""#).unwrap(),
            Some("src/ui/work_state.rs"),
            "read work states through ui::WorkState",
        ),
        (
            Regex::new("⌘ \\+").unwrap(),
            None,
            "shortcuts are glyph-only (⌘K), from ui::shortcuts",
        ),
    ]
}

/// `(line, advice)` for every stray formatter, state string or shortcut notation.
fn violations(relative: &str, source: &str) -> Vec<(usize, &'static str)> {
    let mut found = Vec::new();
    for (pattern, home, advice) in rules() {
        if home == Some(relative) {
            continue;
        }
        for (i, line) in source.lines().enumerate() {
            if pattern.is_match(line) {
                found.push((i + 1, advice));
            }
        }
    }
    found
}

/// The README table built from every `Shortcut { .. }` literal in `ui/shortcuts.rs`.
fn shortcut_table(shortcuts: &str) -> String {
    let entry = Regex::new(
        r#"(?s)Shortcut \{\s*action: "([^"]*)",\s*keystroke: "[^"]*",\s*glyph: "([^"]*)",?\s*\}"#,
    )
    .unwrap();
    let mut table = String::from("| Shortcut | Action |\n| --- | --- |\n");
    for capture in entry.captures_iter(shortcuts) {
        table.push_str(&format!("| {} | {} |\n", &capture[2], &capture[1]));
    }
    table
}

/// The text between the README's shortcut markers.
fn readme_table(readme: &str) -> Result<&str> {
    let start = readme
        .find(README_START)
        .context("README has no shortcuts marker")?;
    let rest = &readme[start + README_START.len()..];
    let end = rest
        .find(README_END)
        .context("README shortcuts marker is unclosed")?;
    Ok(rest[..end].trim_matches('\n'))
}

pub fn run(app: &Path) -> Result<()> {
    let mut errors = Vec::new();
    for path in files(&app.join("src"), "rs")? {
        let relative = path.strip_prefix(app)?.display().to_string();
        for (line, advice) in violations(&relative, &fs::read_to_string(&path)?) {
            errors.push(format!("{relative}:{line}: {advice}"));
        }
    }
    let expected = shortcut_table(&fs::read_to_string(app.join("src/ui/shortcuts.rs"))?);
    let readme = fs::read_to_string(app.join("README.md"))?;
    if readme_table(&readme)? != expected.trim_end() {
        errors.push(format!(
            "README.md: shortcut table differs from src/ui/shortcuts.rs; expected\n{expected}"
        ));
    }
    if !errors.is_empty() {
        bail!("Presentation vocabulary drift:\n{}", errors.join("\n"));
    }
    println!("Presentation vocabulary check passed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_strays_outside_their_home() {
        let source = "t.format(\"%b\")\nx == \"queued\"\nkbd(\"⌘ + K\")";
        assert_eq!(
            violations("src/evee.rs", source)
                .iter()
                .map(|(line, _)| *line)
                .collect::<Vec<_>>(),
            [1, 2, 3]
        );
        assert_eq!(violations("src/ui/time.rs", "t.format(\"%b\")"), []);
        assert_eq!(violations("src/ui/work_state.rs", "\"running\""), []);
    }

    #[test]
    fn readme_table_comes_from_the_shortcuts_source() {
        let source = "pub const A: Shortcut = Shortcut {\n    action: \"Go to…\",\n    keystroke: \"cmd-k\",\n    glyph: \"⌘K\",\n};";
        assert_eq!(
            shortcut_table(source),
            "| Shortcut | Action |\n| --- | --- |\n| ⌘K | Go to… |\n"
        );
        let readme = "intro\n<!-- shortcuts -->\n| a |\n<!-- /shortcuts -->\n";
        assert_eq!(readme_table(readme).unwrap(), "| a |");
    }
}
