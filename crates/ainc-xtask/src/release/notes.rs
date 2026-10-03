//! Commit-based notes and published, version-labelled history for the updater.
use super::distribute::Shell;
use ainc_release::notes::{ReleaseNotes, changelog};
use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::{fs, path::Path};

pub struct Published {
    pub tag: String,
    pub notes: ReleaseNotes,
}

/// Fetch every page, excluding drafts, prereleases and unrelated non-product tags.
pub fn published(shell: &dyn Shell, repo: &str, version: &str) -> Result<Vec<Published>> {
    let current: ReleaseNotes = ReleaseNotes {
        version: version.parse()?,
        notes: String::new(),
    };
    let output = shell.output(&[
        "gh".into(),
        "api".into(),
        format!("repos/{repo}/releases?per_page=100"),
        "--paginate".into(),
        "--slurp".into(),
    ])?;
    let pages: Vec<Vec<Value>> =
        serde_json::from_str(&output).context("release pages are not lists")?;
    let mut releases = Vec::new();
    for release in pages.into_iter().flatten() {
        if release["draft"].as_bool() != Some(false)
            || release["prerelease"].as_bool() != Some(false)
        {
            continue;
        }
        let Some(tag) = release["tag_name"].as_str() else {
            continue;
        };
        let Some(version) = tag.strip_prefix('v').and_then(|v| v.parse().ok()) else {
            continue;
        };
        let notes = ReleaseNotes {
            version,
            notes: release["body"].as_str().unwrap_or("").into(),
        };
        if notes.version.pre.is_empty() && notes.version < current.version {
            releases.push(Published {
                tag: tag.into(),
                notes,
            });
        }
    }
    releases.sort_by(|a, b| b.notes.version.cmp(&a.notes.version));
    releases.dedup_by(|a, b| a.notes.version == b.notes.version);
    Ok(releases)
}

fn escape_markdown(text: &str) -> String {
    let mut escaped = String::new();
    for c in text.chars() {
        if "\\`*_{}[]<>()#!|".contains(c) {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

/// The exact shipped commit, not HEAD, and the preceding *published* product tag.
/// A handwritten file is an explicit override, never a silent PR-only fallback.
pub fn generate(
    shell: &dyn Shell,
    base: &Path,
    repo: &str,
    version: &str,
    commit: &str,
    previous: Option<&str>,
) -> Result<String> {
    let file = base.join(format!("docs/releases/{version}.md"));
    if file.exists() {
        let notes = fs::read_to_string(file)?;
        ensure!(
            !notes.trim().is_empty(),
            "handwritten release notes are empty"
        );
        return Ok(notes);
    }
    let range = if let Some(previous) = previous {
        shell.run(
            &[
                "git".into(),
                "merge-base".into(),
                "--is-ancestor".into(),
                previous.into(),
                commit.into(),
            ],
            None,
        )?;
        format!("{previous}..{commit}")
    } else {
        commit.to_string()
    };
    let log = shell.output(&[
        "git".into(),
        "log".into(),
        "--no-merges".into(),
        "--reverse".into(),
        "--format=%H%x09%s".into(),
        range,
        "--".into(),
    ])?;
    let mut notes = format!("# AgentInc {version}\n\n");
    for line in log.lines() {
        let (sha, subject) = line.split_once('\t').context("invalid commit log record")?;
        ensure!(
            sha.len() == 40 && sha.chars().all(|c| c.is_ascii_hexdigit()),
            "invalid notes commit"
        );
        notes.push_str(&format!(
            "- {} ([{}](https://github.com/{repo}/commit/{sha}))\n",
            escape_markdown(subject),
            &sha[..7]
        ));
    }
    ensure!(
        !log.trim().is_empty(),
        "no commits found for release {version}"
    );
    Ok(notes)
}

fn has_notes(body: &str) -> bool {
    body.lines().any(|line| {
        let line = line.trim();
        !line.is_empty() && !line.starts_with("**Full Changelog**:")
    })
}

/// Older link-only releases are backfilled so skipped versions still show their changes.
pub fn history(
    shell: &dyn Shell,
    base: &Path,
    repo: &str,
    version: &str,
    notes: &str,
    published: &[Published],
) -> Result<String> {
    let mut releases = vec![ReleaseNotes {
        version: version.parse()?,
        notes: notes.into(),
    }];
    for (index, release) in published.iter().enumerate() {
        let notes = if has_notes(&release.notes.notes) {
            release.notes.notes.clone()
        } else {
            generate(
                shell,
                base,
                repo,
                &release.notes.version.to_string(),
                &release.tag,
                published
                    .get(index + 1)
                    .map(|previous| previous.tag.as_str()),
            )?
        };
        releases.push(ReleaseNotes {
            version: release.notes.version.clone(),
            notes,
        });
    }
    Ok(changelog(&releases))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ainc_release::notes::parse_changelog;
    use serde_json::json;
    use std::{collections::HashMap, path::PathBuf, process::Command};

    struct Git {
        root: PathBuf,
        pages: String,
    }
    impl Git {
        fn command(&self, args: &[&str]) -> Result<String> {
            let output = Command::new("git")
                .current_dir(&self.root)
                .args(args)
                .output()?;
            ensure!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            Ok(String::from_utf8(output.stdout)?.trim().into())
        }
        fn commit(&self, name: &str) -> String {
            fs::write(self.root.join("changes"), name).unwrap();
            self.command(&["add", "changes"]).unwrap();
            self.command(&[
                "-c",
                "user.name=Notes Test",
                "-c",
                "user.email=notes@example.com",
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-qm",
                name,
            ])
            .unwrap();
            self.command(&["rev-parse", "HEAD"]).unwrap()
        }
    }
    impl Shell for Git {
        fn output(&self, args: &[String]) -> Result<String> {
            if args[0] == "gh" {
                return Ok(self.pages.clone());
            }
            self.command(&args[1..].iter().map(String::as_str).collect::<Vec<_>>())
        }
        fn run(&self, args: &[String], _: Option<&HashMap<String, String>>) -> Result<()> {
            self.output(args).map(|_| ())
        }
        fn output_bytes(&self, _: &[String]) -> Result<Vec<u8>> {
            unreachable!()
        }
        fn probe(&self, _: &[String]) -> Result<(i32, String)> {
            unreachable!()
        }
        fn wait_for_ci(&self, _: &str, _: &str) -> Result<()> {
            unreachable!()
        }
    }

    fn release(tag: &str, draft: bool, prerelease: bool, body: &str) -> Value {
        json!({"tag_name": tag, "draft": draft, "prerelease": prerelease, "body": body})
    }

    #[test]
    fn direct_commits_use_previous_published_release_not_draft_or_unpublished_tags() {
        let root = tempfile::tempdir().unwrap();
        let git = Git {
            root: root.path().into(),
            pages: json!([
                [
                    release("v0.4.2", true, false, "Draft"),
                    release("build-abc", false, false, "Build")
                ],
                [
                    release("v0.4.1", false, true, "Prerelease"),
                    release("v0.5.0-rc.1", false, false, "Mislabelled prerelease"),
                    release("v0.4.0", false, false, "- Earlier notes")
                ],
            ])
            .to_string(),
        };
        git.command(&["init", "-q"]).unwrap();
        git.commit("Already released");
        git.command(&["tag", "v0.4.0"]).unwrap();
        let first = git.commit("Fix dropped replies");
        git.command(&["tag", "v0.4.1"]).unwrap();
        let shipped = git.commit("Keep [search] visible <safely>");
        git.command(&["tag", "v0.4.2"]).unwrap();
        git.commit("Not in this release");
        let published = published(&git, "owner/repo", "0.5.0").unwrap();
        assert_eq!(published.len(), 1);
        assert_eq!(published[0].tag, "v0.4.0");
        let notes = generate(
            &git,
            root.path(),
            "owner/repo",
            "0.5.0",
            &shipped,
            Some(&published[0].tag),
        )
        .unwrap();
        assert!(notes.contains("Fix dropped replies"));
        assert!(notes.contains("Keep \\[search\\] visible \\<safely\\>"));
        assert!(notes.contains(&format!("https://github.com/owner/repo/commit/{first}")));
        assert!(!notes.contains("Already released"));
        assert!(!notes.contains("Not in this release"));
        let history =
            history(&git, root.path(), "owner/repo", "0.5.0", &notes, &published).unwrap();
        let parsed = parse_changelog(&history).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].version.to_string(), "0.5.0");
        assert_eq!(parsed[1].notes, "- Earlier notes");
    }

    #[test]
    fn first_release_includes_history_and_link_only_releases_are_backfilled() {
        let root = tempfile::tempdir().unwrap();
        let git = Git {
            root: root.path().into(),
            pages: "[]".into(),
        };
        git.command(&["init", "-q"]).unwrap();
        let commit = git.commit("Initial feature");
        git.command(&["tag", "v0.4.0"]).unwrap();
        let first = generate(&git, root.path(), "owner/repo", "0.4.0", &commit, None).unwrap();
        assert!(first.contains("Initial feature"));
        let old = vec![Published {
            tag: "v0.4.0".into(),
            notes: ReleaseNotes {
                version: "0.4.0".parse().unwrap(),
                notes: "**Full Changelog**: https://example.com/diff".into(),
            },
        }];
        let history = history(&git, root.path(), "owner/repo", "0.5.0", "New notes", &old).unwrap();
        assert_eq!(
            parse_changelog(&history).unwrap()[1].notes,
            first.strip_prefix("# AgentInc 0.4.0\n\n").unwrap().trim()
        );
    }

    #[test]
    fn handwritten_override_does_not_need_git_and_empty_override_fails() {
        let root = tempfile::tempdir().unwrap();
        let git = Git {
            root: root.path().into(),
            pages: "[]".into(),
        };
        let path = root.path().join("docs/releases/0.5.0.md");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "# AgentInc 0.5.0\n\n- Curated **notes**").unwrap();
        assert_eq!(
            generate(
                &git,
                root.path(),
                "owner/repo",
                "0.5.0",
                "not-a-commit",
                None
            )
            .unwrap(),
            fs::read_to_string(&path).unwrap()
        );
        fs::write(path, "\n").unwrap();
        assert!(
            generate(
                &git,
                root.path(),
                "owner/repo",
                "0.5.0",
                "not-a-commit",
                None
            )
            .is_err()
        );
    }
}
