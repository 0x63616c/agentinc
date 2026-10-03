//! Reject new native component forks outside the app-owned ui module.
//! Counts per rule and file are ratcheted by `scripts/ui-core-baseline.json`.
use super::files;
use anyhow::{Result, bail};
use regex::Regex;
use std::{collections::BTreeMap, fs, path::Path};

type Baseline = BTreeMap<String, BTreeMap<String, usize>>;

/// `(name, pattern, advice)`, in the order violations are reported.
fn rules() -> [(&'static str, Regex, &'static str); 3] {
    [
        (
            "local constructor",
            Regex::new(r"\bfn\s+(?:button|field)\s*\(").unwrap(),
            "use ui::action_button or ui::text_field",
        ),
        (
            "raw button role",
            Regex::new(r"\bRole::Button\b").unwrap(),
            "use ui::action_button for focus, disabled, and keyboard behavior",
        ),
        (
            "raw RGB",
            Regex::new(r"\brgb\s*\(\s*0x[0-9a-fA-F]+").unwrap(),
            "add a color role to ui::tokens and use that role",
        ),
    ]
}

/// `(line, rule)` for every match beyond what the baseline allows.
fn violations(source: &str, allowed: &BTreeMap<String, usize>) -> Vec<(usize, &'static str)> {
    let rules = rules();
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    let mut found = Vec::new();
    for (i, line) in source.lines().enumerate() {
        for (name, pattern, _) in &rules {
            for _ in pattern.find_iter(line) {
                let count = seen.entry(name).or_default();
                *count += 1;
                if *count > allowed.get(*name).copied().unwrap_or(0) {
                    found.push((i + 1, *name));
                }
            }
        }
    }
    found
}

pub fn run(app: &Path) -> Result<()> {
    let baseline: Baseline = serde_json::from_str(&fs::read_to_string(
        app.join("scripts/ui-core-baseline.json"),
    )?)?;
    let rules = rules();
    let advice = |rule: &str| rules.iter().find(|r| r.0 == rule).map_or("", |r| r.2);
    let raw_rgb = &rules[2].1;
    let ui = app.join("src/ui");
    let mut errors = Vec::new();
    for path in files(&app.join("src"), "rs")? {
        let relative = path.strip_prefix(app)?.display().to_string();
        let source = fs::read_to_string(&path)?;
        if path.starts_with(&ui) {
            if path.file_name().is_some_and(|n| n != "tokens.rs") {
                for (i, line) in source.lines().enumerate() {
                    if raw_rgb.is_match(line) {
                        errors.push(format!(
                            "{relative}:{}: raw RGB; {}",
                            i + 1,
                            advice("raw RGB")
                        ));
                    }
                }
            }
            continue;
        }
        let allowed = baseline.get(&relative).cloned().unwrap_or_default();
        for (n, rule) in violations(&source, &allowed) {
            errors.push(format!("{relative}:{n}: {rule}; {}", advice(rule)));
        }
    }
    if !errors.is_empty() {
        bail!("New native UI forks:\n{}", errors.join("\n"));
    }
    println!("Native UI core ratchet passed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_forks_beyond_the_baseline() {
        let allowed = BTreeMap::from([("local constructor".to_string(), 1)]);
        assert_eq!(
            violations("fn button() {}\nfn button() {}", &allowed),
            vec![(2, "local constructor")]
        );
    }

    #[test]
    fn flags_raw_roles_and_colors() {
        assert_eq!(
            violations(".role(Role::Button)\nrgb(0xff00ff)", &BTreeMap::new()),
            vec![(1, "raw button role"), (2, "raw RGB")]
        );
    }
}
