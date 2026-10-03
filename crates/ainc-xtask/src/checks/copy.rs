//! The copy style sheet (docs/design-system.md "Copy") over the Mac app's string
//! literals. The heuristic is deliberately small: it reads the literal passed to a
//! handful of constructors, classifies it by the constructor, and checks what a
//! regex can check. Judgement calls go in `ALLOWED`, not in cleverness here.
use super::files;
use anyhow::{Result, bail};
use std::{fs, path::Path};

/// What a literal is, by the constructor it was passed to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// A button, menu or palette entry, dialog or page title: Title Case.
    Title,
    /// An `EmptyState` title: a sentence, `No {things} yet.`
    EmptyTitle,
    /// A `TextInput::field` placeholder: a sentence-case noun phrase, no period.
    Placeholder,
}

/// `(constructor, which argument holds the literal, kind)`.
const SITES: [(&str, usize, Kind); 8] = [
    ("Button::new(", 1, Kind::Title),
    ("MenuEntry::new(", 1, Kind::Title),
    ("PaletteEntry::new(", 1, Kind::Title),
    ("PageHeader::new(", 0, Kind::Title),
    ("dialog_shell(", 0, Kind::Title),
    ("MenuItem::action(", 0, Kind::Title),
    ("EmptyState::new(", 1, Kind::EmptyTitle),
    ("TextInput::field(", 0, Kind::Placeholder),
];

/// Words Title Case leaves lowercase inside a title.
const SMALL_WORDS: [&str; 15] = [
    "a", "an", "and", "as", "at", "by", "for", "from", "in", "of", "on", "or", "the", "to", "with",
];

/// CONTEXT.md's avoid-words, whole-word and case-insensitive.
const AVOID_WORDS: [&str; 20] = [
    "task",
    "tasks",
    "issue",
    "issues",
    "job",
    "jobs",
    "chat",
    "chats",
    "thread",
    "threads",
    "session",
    "sessions",
    "bot",
    "bots",
    "principal",
    "agentic",
    "firstmate",
    "integration",
    "tenant",
    "cron",
];

/// Literals the heuristic misjudges: brand names and glyph-only labels.
const ALLOWED: [&str; 2] = ["Sign in with ChatGPT", "System · SF Pro"];

/// The `index`th argument of the call starting after `open`, if it is a string literal.
fn literal_argument(source: &str, open: usize, index: usize) -> Option<String> {
    let mut depth = 0usize;
    let mut argument = 0usize;
    let mut in_string = false;
    let mut start = open;
    let bytes = source.as_bytes();
    let mut i = open;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if in_string {
            if c == '\\' {
                i += 1;
            } else if c == '"' {
                in_string = false;
            }
        } else {
            match c {
                '"' => in_string = true,
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => {
                    if depth == 0 {
                        return literal(&source[start..i]);
                    }
                    depth -= 1;
                }
                ',' if depth == 0 => {
                    if argument == index {
                        return literal(&source[start..i]);
                    }
                    argument += 1;
                    start = i + 1;
                }
                _ => {}
            }
        }
        i += 1;
    }
    None
}

/// The text of `argument` when it is exactly one plain string literal.
fn literal(argument: &str) -> Option<String> {
    let trimmed = argument.trim();
    let inner = trimmed.strip_prefix('"')?.strip_suffix('"')?;
    (!inner.contains('"')).then(|| inner.to_owned())
}

/// Why `text` breaks the rules for its `kind`, if it does.
fn violation(kind: Kind, text: &str) -> Option<String> {
    if text.is_empty() || ALLOWED.contains(&text) {
        return None;
    }
    if text.contains("...") {
        return Some("use the `…` glyph, not three dots".into());
    }
    if text.contains('\'') {
        return Some("use `’`, not a straight apostrophe".into());
    }
    if text.contains("Canceled") || text.contains("canceled") {
        return Some("British spelling: Cancelled".into());
    }
    if let Some(word) = text
        .split(|c: char| !c.is_alphanumeric())
        .find(|word| AVOID_WORDS.contains(&word.to_lowercase().as_str()))
    {
        return Some(format!("\"{word}\" is an avoid-word in CONTEXT.md"));
    }
    match kind {
        Kind::Title => title_case(text),
        Kind::EmptyTitle => (!text.ends_with('.') || !starts_upper(text))
            .then(|| "empty states read `No {things} yet.`, a sentence with its period".into()),
        Kind::Placeholder => (text.ends_with('.') || !starts_upper(text))
            .then(|| "placeholders are sentence-case noun phrases without a period".into()),
    }
}

fn starts_upper(text: &str) -> bool {
    text.chars().next().is_some_and(char::is_uppercase)
}

/// Title Case: the first word and every word longer than three letters start uppercase;
/// shorter words may stay lowercase only when they are small words. Accessible labels
/// with a `·` separator are not titles and are skipped.
fn title_case(text: &str) -> Option<String> {
    if text.contains('·') {
        return None;
    }
    for (index, word) in text.split_whitespace().enumerate() {
        let word = word.trim_matches(|c: char| !c.is_alphanumeric());
        let Some(first) = word.chars().next() else {
            continue;
        };
        if !first.is_alphabetic() || first.is_uppercase() {
            continue;
        }
        let small = SMALL_WORDS.contains(&word);
        if index == 0 || !small {
            return Some(format!("Title Case: capitalize \"{word}\""));
        }
    }
    None
}

/// Every violation in one file, as `(line, message)`.
fn violations(source: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for (site, index, kind) in SITES {
        for (offset, _) in source.match_indices(site) {
            let open = offset + site.len();
            if let Some(text) = literal_argument(source, open, index)
                && let Some(why) = violation(kind, &text)
            {
                let line = source[..offset].matches('\n').count() + 1;
                found.push((line, format!("\"{text}\": {why}")));
            }
        }
    }
    found.sort();
    found
}

pub fn run(app: &Path) -> Result<()> {
    let mut errors = Vec::new();
    for path in files(&app.join("src"), "rs")? {
        let relative = path.strip_prefix(app)?.display().to_string();
        for (line, message) in violations(&fs::read_to_string(&path)?) {
            errors.push(format!("{relative}:{line}: {message}"));
        }
    }
    if !errors.is_empty() {
        bail!(
            "Copy style sheet violations (docs/design-system.md):\n{}",
            errors.join("\n")
        );
    }
    println!("Copy style sheet check passed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals_are_read_from_the_right_argument() {
        let source = r#"Button::new(("copy", id as u64), "Copy").icon("x")"#;
        assert_eq!(violations(source), []);
        assert_eq!(
            literal_argument(source, "Button::new(".len(), 1).as_deref(),
            Some("Copy")
        );
        assert_eq!(literal_argument("PageHeader::new(title)", 16, 0), None);
    }

    #[test]
    fn planted_violations_are_named() {
        let messages = |source: &str| {
            violations(source)
                .into_iter()
                .map(|(_, m)| m)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            messages(r#"Button::new("a", "Add label")"#),
            [r#""Add label": Title Case: capitalize "label""#]
        );
        assert_eq!(
            messages(r#"MenuEntry::new("a", "Downloading...")"#),
            [r#""Downloading...": use the `…` glyph, not three dots"#]
        );
        assert_eq!(
            messages(r#"PageHeader::new("You're up to date")"#),
            [r#""You're up to date": use `’`, not a straight apostrophe"#]
        );
        assert_eq!(
            messages(r#"dialog_shell("Download Canceled", body, footer)"#),
            [r#""Download Canceled": British spelling: Cancelled"#]
        );
        assert_eq!(
            messages(r#"PaletteEntry::new("a", "New Task")"#),
            [r#""New Task": "Task" is an avoid-word in CONTEXT.md"#]
        );
        assert_eq!(
            messages(r#"EmptyState::new("spark", "No Tickets yet")"#),
            [
                r#""No Tickets yet": empty states read `No {things} yet.`, a sentence with its period"#
            ]
        );
        assert_eq!(
            messages(r#"TextInput::field("What needs doing?.", false, cx)"#),
            [
                r#""What needs doing?.": placeholders are sentence-case noun phrases without a period"#
            ]
        );
    }

    #[test]
    fn the_style_sheet_passes() {
        for source in [
            r#"Button::new("a", "Sign in with ChatGPT")"#,
            r#"Button::new("a", "Go to…")"#,
            r#"Button::new("a", "Check for Updates")"#,
            r#"Button::new("a", "Update ready · Install")"#,
            r#"Button::new("a", "Delete")"#,
            r#"EmptyState::new("spark", "No matching Tickets.")"#,
            r#"TextInput::field("Search Tickets", false, cx)"#,
            r#"Button::new("a", format!("Remove {label}"))"#,
        ] {
            assert_eq!(violations(source), [], "{source}");
        }
    }
}
