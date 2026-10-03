//! Fail on the product's retired names. The Mac package is `ainc-mac` and its binary
//! `AgentInc`; the product is spelled AgentInc in prose and UI strings; `Agentinc OS` survives
//! only as the name of the legacy data directory; and the `agent-inc-*` request headers are
//! named once, as `ainc_identity::{CLIENT_HEADER, SERVER_HEADER}`.
use anyhow::{Result, bail};
use std::path::Path;

/// Files the check does not read: history, generated output and this file's own patterns.
const SKIPPED: [&str; 4] = [
    "Cargo.lock",
    "docs/cohesion-plan.md",
    "crates/ainc-mac/docs/verification/",
    "crates/ainc-xtask/src/checks/naming.rs",
];

/// The bundle keeps an `agentinc-os` symlink for the installed 0.1.0 updater.
const LEGACY_EXECUTABLE_OWNER: &str = "crates/ainc-xtask/src/release/prepare.rs";

/// Where `Agentinc OS` may appear as a bare name: the glossary's "formerly named" line and the
/// code that resolves the legacy data directory.
const LEGACY_NAME_OWNERS: [&str; 2] = ["CONTEXT.md", "crates/ainc-identity/src/lib.rs"];

/// Header literals still to be replaced; none today.
const HEADER_LITERAL_FOLLOW_UPS: [&str; 0] = [];

fn skipped(path: &str) -> bool {
    SKIPPED
        .iter()
        .any(|skip| path == *skip || path.starts_with(skip))
}

/// Every violation in one file, as `path:line: reason`.
pub(crate) fn violations(path: &str, text: &str) -> Vec<String> {
    let mut found = Vec::new();
    if skipped(path) {
        return found;
    }
    for (index, line) in text.lines().enumerate() {
        let mut report = |reason: &str| found.push(format!("{path}:{}: {reason}", index + 1));
        if line.contains("agentinc-os") && path != LEGACY_EXECUTABLE_OWNER {
            report("`agentinc-os` is gone; the package is `ainc-mac`, the binary `AgentInc`");
        }
        for (start, _) in line.match_indices("Agentinc") {
            let rest = &line[start..];
            if rest.starts_with("Agentinc OS") {
                let legacy_path = rest.starts_with("Agentinc OS/")
                    || line[..start].ends_with("Support/")
                    || LEGACY_NAME_OWNERS.contains(&path);
                if !legacy_path {
                    report("`Agentinc OS` names only the legacy data directory; write AgentInc");
                }
            } else {
                report("the product is spelled AgentInc");
            }
        }
        if line.contains("agent-inc-")
            && !path.starts_with("crates/ainc-identity/")
            && !HEADER_LITERAL_FOLLOW_UPS.contains(&path)
        {
            report("use ainc_identity::{CLIENT_HEADER, SERVER_HEADER} instead of the literal");
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
        bail!("naming violations:\n{}", found.join("\n"));
    }
    println!("Product names are consistent");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::violations;

    #[test]
    fn retired_package_name_is_reported_except_for_the_legacy_symlink() {
        assert_eq!(
            violations("justfile", "cargo build -p agentinc-os").len(),
            1
        );
        assert!(
            violations(
                "crates/ainc-xtask/src/release/prepare.rs",
                "const LEGACY_EXECUTABLE: &str = \"agentinc-os\";"
            )
            .is_empty()
        );
    }

    #[test]
    fn legacy_data_directory_is_the_only_agentinc_os() {
        assert!(
            violations(
                "docs/x.md",
                "stored in `~/Library/Application Support/Agentinc OS/daemon`"
            )
            .is_empty()
        );
        assert!(violations("docs/x.md", "the `…/Agentinc OS/api-url` file").is_empty());
        assert!(violations("CONTEXT.md", "Formerly named Agentinc OS.").is_empty());
        assert_eq!(violations("docs/x.md", "Agentinc OS now uses Zed").len(), 1);
        assert_eq!(violations("src/a.rs", "title: \"Agentinc QA\"").len(), 1);
        assert!(violations("src/a.rs", "title: \"AgentInc QA\"").is_empty());
    }

    #[test]
    fn header_literals_live_in_identity_only() {
        assert_eq!(
            violations(
                "crates/ainc-cli/src/main.rs",
                ".header(\"agent-inc-client\", v)"
            )
            .len(),
            1
        );
        assert!(
            violations(
                "crates/ainc-identity/src/lib.rs",
                "pub const CLIENT_HEADER: &str = \"agent-inc-client\";"
            )
            .is_empty()
        );
        assert_eq!(
            violations("crates/ainc-daemon/src/lib.rs", "\"agent-inc-server\",").len(),
            1
        );
    }

    #[test]
    fn history_and_the_plan_are_skipped() {
        assert!(violations("docs/cohesion-plan.md", "package `agentinc-os`").is_empty());
        assert!(
            violations("crates/ainc-mac/docs/verification/STATUS.md", "Agentinc QA").is_empty()
        );
    }
}
