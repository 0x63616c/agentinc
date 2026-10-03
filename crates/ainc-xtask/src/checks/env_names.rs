//! Fail on environment variable names outside their family. Product variables are `AINC_*`;
//! `TURNKEEL_*` belongs to the SDK alone, so it appears only under `crates/turnkeel/`.
use anyhow::{Result, bail};
use regex::Regex;
use std::path::Path;

/// Retired `AGENTINC_*` names still allowed while their lane renames them; none today.
const FOLLOW_UPS: [&str; 0] = [];

/// This file names the retired prefix in its own patterns.
const SELF: &str = "crates/ainc-xtask/src/checks/env_names.rs";

/// Every violation in one file under `crates/`, as `path:line: reason`.
pub(crate) fn violations(path: &str, text: &str) -> Vec<String> {
    let mut found = Vec::new();
    if !path.starts_with("crates/") || path == SELF {
        return found;
    }
    let name = Regex::new(r"\b(AGENTINC_|TURNKEEL_)[A-Z0-9_]*").unwrap();
    for (index, line) in text.lines().enumerate() {
        for hit in name.find_iter(line) {
            let hit = hit.as_str();
            let allowed = if hit.starts_with("AGENTINC_") {
                FOLLOW_UPS.contains(&hit)
            } else {
                path.starts_with("crates/turnkeel/")
            };
            if !allowed {
                let reason = if hit.starts_with("AGENTINC_") {
                    "product variables are `AINC_*`"
                } else {
                    "`TURNKEEL_*` is the SDK's; the product uses `AINC_*`"
                };
                found.push(format!("{path}:{}: `{hit}`: {reason}", index + 1));
            }
        }
    }
    found
}

pub fn run(root: &Path) -> Result<()> {
    let mut found = Vec::new();
    for (path, text) in super::tracked_text_files(root)? {
        found.extend(violations(&path, &text));
    }
    if !found.is_empty() {
        bail!(
            "environment variable name violations:\n{}",
            found.join("\n")
        );
    }
    println!("Environment variable names stay in their families");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::violations;

    #[test]
    fn product_variables_use_the_ainc_prefix() {
        assert_eq!(
            violations(
                "crates/ainc-mac/src/main.rs",
                "var(\"AGENTINC_SESSION_PATH\")"
            )
            .len(),
            1
        );
        assert!(violations("crates/ainc-mac/src/main.rs", "var(\"AINC_SESSION_PATH\")").is_empty());
        assert!(
            violations(
                "crates/ainc-daemon/src/codex.rs",
                "var(\"AINC_CODEX_HOME\")"
            )
            .is_empty()
        );
    }

    #[test]
    fn turnkeel_variables_stay_in_the_sdk() {
        assert!(violations("crates/turnkeel/tests/recovery.rs", "TURNKEEL_EFFECT_GATE").is_empty());
        assert_eq!(
            violations("crates/ainc-daemon/src/lib.rs", "TURNKEEL_EFFECT_GATE").len(),
            1
        );
    }

    #[test]
    fn only_crates_are_checked() {
        assert!(violations("docs/plan.md", "AGENTINC_SESSION_PATH").is_empty());
    }
}
