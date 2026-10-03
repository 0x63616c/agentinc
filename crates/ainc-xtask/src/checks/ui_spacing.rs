//! Warn when migrated native UI files gain raw numeric spacing calls.
//! Counts per `method:value` are ratcheted by `scripts/ui-spacing-baseline.json`.
use anyhow::Result;
use regex::Regex;
use std::{collections::BTreeMap, fs, path::Path};

type Baseline = BTreeMap<String, BTreeMap<String, usize>>;

fn preferred(method: &str) -> &'static str {
    match method {
        "gap" => "CONTROL_GAP, FIELD_LABEL_GAP, or FORM_STACK_GAP",
        "px" | "pl" | "pr" => "CONTROL_INSET_X, FIELD_INSET_X, or a named optical inset",
        _ => "PAGE_X, RIGHT_PANE_CONTENT_INSET, or a named inset token",
    }
}

/// `(line, method, value)` for every literal beyond what the baseline allows.
fn new_literals(source: &str, allowed: &BTreeMap<String, usize>) -> Vec<(usize, String, String)> {
    let spacing = Regex::new(
        r"\.(gap|p|px|py|pl|pr|pt|pb|m|mx|my|ml|mr|mt|mb)\(\s*px\(\s*(-?\d+(?:\.\d*)?)\s*\)\s*\)",
    )
    .unwrap();
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    let mut found = Vec::new();
    for (i, line) in source.lines().enumerate() {
        for m in spacing.captures_iter(line) {
            let key = format!("{}:{}", &m[1], &m[2]);
            let count = seen.entry(key.clone()).or_default();
            *count += 1;
            if *count > allowed.get(&key).copied().unwrap_or(0) {
                found.push((i + 1, m[1].to_string(), m[2].to_string()));
            }
        }
    }
    found
}

pub fn run(app: &Path) -> Result<()> {
    let baseline: Baseline = serde_json::from_str(&fs::read_to_string(
        app.join("scripts/ui-spacing-baseline.json"),
    )?)?;
    let mut warnings = 0;
    for (name, allowed) in &baseline {
        for (n, method, value) in new_literals(&fs::read_to_string(app.join(name))?, allowed) {
            warnings += 1;
            println!(
                "::warning file=crates/ainc-mac/{name},line={n}::Raw {method} spacing {value}px; \
                 prefer {} from ui/tokens.rs. Name an optical exception there if needed.",
                preferred(&method)
            );
        }
    }
    println!("UI spacing ratchet: {warnings} new raw literal warning(s)");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_literals_beyond_the_baseline() {
        let allowed = BTreeMap::from([("gap:8.".to_string(), 1)]);
        assert_eq!(
            new_literals(".gap(px(8.)).gap(px(8.))", &allowed),
            vec![(1, "gap".to_string(), "8.".to_string())]
        );
    }

    #[test]
    fn allows_tokens() {
        assert!(new_literals(".gap(px(CONTROL_GAP))", &BTreeMap::new()).is_empty());
    }
}
