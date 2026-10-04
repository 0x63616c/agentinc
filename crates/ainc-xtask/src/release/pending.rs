//! A release keeps running until main's product version is published. Every push to main
//! asks again, so a failed run is retried by the next push instead of being lost (0.8.0 sat
//! unreleased for 15 hours when only the version-bump push could start it).
use super::output;
use anyhow::{Result, bail};
use serde_json::Value;

/// Whether no published (non-draft) release is tagged `vVERSION`.
fn pending(releases: &[Value], version: &str) -> bool {
    let tag = format!("v{version}");
    !releases
        .iter()
        .any(|release| release["tag_name"] == tag.as_str() && release["draft"] == false)
}

pub fn cli(args: &[String]) -> Result<()> {
    let [version] = args else {
        bail!("usage: cargo xtask release-pending VERSION");
    };
    let pages: Vec<Vec<Value>> = serde_json::from_str(&output(
        "gh",
        &[
            "api",
            "--paginate",
            "--slurp",
            "repos/{owner}/{repo}/releases?per_page=100",
        ],
    )?)?;
    let releases: Vec<Value> = pages.into_iter().flatten().collect();
    println!("{}", pending(&releases, version));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_version_is_pending_until_its_release_is_published() {
        let published = json!({"tag_name": "v0.7.0", "draft": false});
        let draft = json!({"tag_name": "v0.8.0", "draft": true});
        let test_draft = json!({"tag_name": "phase5-test-0123456789ab", "draft": false});
        let releases = [published, draft, test_draft];
        assert!(!pending(&releases, "0.7.0"));
        // A draft from a failed or in-flight run does not count as released.
        assert!(pending(&releases, "0.8.0"));
        assert!(pending(&releases, "0.9.0"));
    }
}
