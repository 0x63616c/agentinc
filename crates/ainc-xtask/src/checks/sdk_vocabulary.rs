//! The SDK speaks agent vocabulary. Fail when a product name appears under
//! `crates/turnkeel/src`: `agentinc`, `ainc`, `ticket(s)` or `evee`, as whole words, in any
//! case. The one exemption is `engine/legacy.rs`, which holds the durable names retained
//! histories were recorded under and exists only so they keep replaying.
use super::files;
use anyhow::{Result, bail};
use regex::Regex;
use std::{fs, path::Path};

const EXEMPT: &str = "engine/legacy.rs";

fn violations(text: &str) -> Vec<(usize, String)> {
    let word = Regex::new(r"(?i)\b(agentinc|ainc|tickets?|evee)\b").unwrap();
    text.lines()
        .enumerate()
        .filter(|(_, line)| word.is_match(line))
        .map(|(i, line)| (i + 1, line.trim().to_string()))
        .collect()
}

pub fn run(root: &Path) -> Result<()> {
    let sdk = root.join("crates/turnkeel/src");
    let mut failures = Vec::new();
    for path in files(&sdk, "rs")? {
        if path.ends_with(EXEMPT) {
            continue;
        }
        for (line, text) in violations(&fs::read_to_string(&path)?) {
            failures.push(format!("{}:{line}: {text}", path.display()));
        }
    }
    if !failures.is_empty() {
        bail!(
            "product vocabulary inside the SDK (wrap it in agent vocabulary: Run, Session, Tool, Model):\n{}",
            failures.join("\n")
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_words_in_any_case() {
        assert_eq!(violations("let name = \"ticket-42\";").len(), 1);
        assert_eq!(violations("// AgentInc owns product state").len(), 1);
        assert_eq!(violations("let ainc_id = 1;").len(), 0);
        assert_eq!(violations("let tickets = 1;").len(), 1);
        assert_eq!(violations("Runtime, Run, Session, Tool, Model").len(), 0);
    }
}
