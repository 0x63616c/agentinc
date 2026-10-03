//! Fail closed on the latest exact-commit CI workflow, not a similarly named check.
use super::output;
use anyhow::{Result, anyhow, bail};
use serde_json::Value;
use std::{env, thread, time::Duration};

fn field<'a>(run: &'a Value, name: &str) -> Result<&'a Value> {
    run.get(name)
        .ok_or_else(|| anyhow!("workflow run is missing {name}"))
}

/// Whether the latest eligible run for `commit` has completed successfully. A latest run that
/// completed any other way is an error; one still running, or none at all, is `false`.
pub fn ci_state(runs: &[Value], commit: &str) -> Result<bool> {
    let mut latest: Option<(&Value, (u64, u64))> = None;
    for run in runs {
        let event = field(run, "event")?.as_str();
        if field(run, "head_sha")?.as_str() != Some(commit)
            || !matches!(event, Some("push" | "pull_request"))
        {
            continue;
        }
        let key = (
            field(run, "run_number")?.as_u64().unwrap_or(0),
            field(run, "run_attempt")?.as_u64().unwrap_or(0),
        );
        // First maximum wins on a tie, like Python's max().
        if latest.is_none_or(|(_, best)| key > best) {
            latest = Some((run, key));
        }
    }
    let Some((latest, _)) = latest else {
        return Ok(false);
    };
    if field(latest, "status")?.as_str() != Some("completed") {
        return Ok(false);
    }
    if field(latest, "conclusion")?.as_str() != Some("success") {
        bail!("release refused: latest workspace CI failed for this exact commit");
    }
    Ok(true)
}

pub fn runs_endpoint(repo: &str, commit: &str) -> String {
    format!("repos/{repo}/actions/workflows/ci.yml/runs?head_sha={commit}&per_page=100")
}

/// Poll until the exact-commit CI run succeeds. `fetch` returns the endpoint's JSON.
pub fn wait_for_ci_with(
    repo: &str,
    commit: &str,
    mut fetch: impl FnMut(&str) -> Result<Value>,
    mut sleep: impl FnMut(),
) -> Result<()> {
    loop {
        let response = fetch(&runs_endpoint(repo, commit))?;
        let runs = field(&response, "workflow_runs")?
            .as_array()
            .ok_or_else(|| anyhow!("workflow_runs is not a list"))?;
        if ci_state(runs, commit)? {
            return Ok(());
        }
        println!("Waiting for workspace CI on the release commit");
        sleep();
    }
}

pub fn wait_for_ci(commit: &str) -> Result<()> {
    let repo = env::var("GITHUB_REPOSITORY").map_err(|_| anyhow!("GITHUB_REPOSITORY"))?;
    wait_for_ci_with(
        &repo,
        commit,
        |endpoint| Ok(serde_json::from_str(&output("gh", &["api", endpoint])?)?),
        || thread::sleep(Duration::from_secs(15)),
    )
}

pub fn cli(args: &[String]) -> Result<()> {
    let Some(commit) = args.first() else {
        bail!("usage: cargo xtask release-ci-gate COMMIT");
    };
    wait_for_ci(commit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const COMMIT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn ci_run(overrides: Value) -> Value {
        let mut run = json!({
            "head_sha": COMMIT, "event": "push", "run_number": 1, "run_attempt": 1,
            "status": "completed", "conclusion": "success",
        });
        for (key, value) in overrides.as_object().unwrap() {
            run[key] = value.clone();
        }
        run
    }

    #[test]
    fn success_must_be_for_exact_commit() {
        assert!(ci_state(&[ci_run(json!({}))], COMMIT).unwrap());
        let other = ci_run(json!({"head_sha": "b".repeat(40)}));
        assert!(!ci_state(&[other], COMMIT).unwrap());
        assert!(!ci_state(&[], COMMIT).unwrap());
    }

    #[test]
    fn pending_latest_run_does_not_accept_old_success() {
        let latest = ci_run(json!({"run_number": 2, "status": "in_progress", "conclusion": null}));
        assert!(!ci_state(&[latest, ci_run(json!({}))], COMMIT).unwrap());
    }

    #[test]
    fn latest_failed_or_cancelled_attempt_blocks() {
        for conclusion in [
            json!("failure"),
            json!("cancelled"),
            json!("skipped"),
            json!("timed_out"),
            json!(null),
        ] {
            let latest = ci_run(json!({"run_attempt": 2, "conclusion": conclusion}));
            assert!(
                ci_state(&[ci_run(json!({})), latest], COMMIT).is_err(),
                "{conclusion}"
            );
        }
    }

    #[test]
    fn unrequested_workflow_event_cannot_satisfy_gate() {
        let run = ci_run(json!({"event": "workflow_dispatch"}));
        assert!(!ci_state(&[run], COMMIT).unwrap());
    }

    #[test]
    fn queries_the_ci_workflow_not_a_check_name() {
        let mut endpoints = Vec::new();
        let mut sleeps = 0;
        wait_for_ci_with(
            "owner/repo",
            COMMIT,
            |endpoint| {
                endpoints.push(endpoint.to_string());
                Ok(json!({"workflow_runs": [ci_run(json!({}))]}))
            },
            || sleeps += 1,
        )
        .unwrap();
        assert!(
            endpoints
                .last()
                .unwrap()
                .contains(&format!("/actions/workflows/ci.yml/runs?head_sha={COMMIT}"))
        );
        assert_eq!(sleeps, 0);
    }
}
