//! Versioned release history carried as Markdown in the schema-1 changelog.
//! Keeping the wire fields unchanged lets already-installed strict readers update.
use semver::Version;

const TITLE: &str = "# AgentInc changelog\n\n";
const HEADING: &str = "## AgentInc ";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseNotes {
    pub version: Version,
    pub notes: String,
}

/// Render a version-labelled history, newest first. Bodies remain ordinary Markdown.
pub fn changelog(releases: &[ReleaseNotes]) -> String {
    let mut releases = releases.iter().collect::<Vec<_>>();
    releases.sort_by(|a, b| b.version.cmp(&a.version));
    let mut text = TITLE.to_string();
    for release in releases {
        let notes = release.notes.trim();
        let title = format!("# AgentInc {}", release.version);
        let notes = notes
            .strip_prefix(&title)
            .filter(|rest| rest.is_empty() || rest.starts_with('\n'))
            .unwrap_or(notes)
            .trim();
        text.push_str(&format!("{HEADING}{}\n\n{notes}\n\n", release.version));
    }
    text
}

/// Only recognize our labelled format; old free-form changelogs use the legacy fallback.
pub fn parse_changelog(text: &str) -> Option<Vec<ReleaseNotes>> {
    let text = text.strip_prefix(TITLE)?;
    let mut releases: Vec<ReleaseNotes> = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    for line in text.lines() {
        // Handwritten notes may contain fenced examples, including our heading syntax.
        let trimmed = line.trim_start_matches(' ');
        if line.len() - trimmed.len() <= 3
            && let Some(marker @ ('`' | '~')) = trimmed.chars().next()
        {
            let length = trimmed.chars().take_while(|c| *c == marker).count();
            if length >= 3 {
                if let Some((opening, count)) = fence {
                    if marker == opening && length >= count && trimmed[length..].trim().is_empty() {
                        fence = None;
                    }
                } else {
                    fence = Some((marker, length));
                }
            }
        }
        if fence.is_none()
            && let Some(version) = line.strip_prefix(HEADING)
        {
            releases.push(ReleaseNotes {
                version: version.parse().ok()?,
                notes: String::new(),
            });
        } else if let Some(release) = releases.last_mut() {
            release.notes.push_str(line);
            release.notes.push('\n');
        } else if !line.trim().is_empty() {
            return None;
        }
    }
    for release in &mut releases {
        release.notes = release.notes.trim().to_string();
    }
    Some(releases)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_round_trips_markdown_in_version_order_without_repeating_titles() {
        let notes = vec![
            ReleaseNotes { version: Version::new(0, 4, 1), notes: "# AgentInc 0.4.1\n\n- **Fix** a crash".into() },
            ReleaseNotes { version: Version::new(0, 4, 3), notes: "### Features\n\n- [Details](https://example.com)\n\n```md\n## AgentInc 9.0.0\n```".into() },
        ];
        let history = changelog(&notes);
        let parsed = parse_changelog(&history).unwrap();
        assert_eq!(parsed[0], notes[1]);
        assert_eq!(parsed[1].notes, "- **Fix** a crash");
        assert_eq!(parsed[1].version, notes[0].version);
        assert!(!history.lines().any(|line| line == "# AgentInc 0.4.1"));
    }

    #[test]
    fn legacy_or_malformed_history_is_not_treated_as_versioned() {
        assert!(parse_changelog("Old history").is_none());
        assert!(parse_changelog("# AgentInc changelog\n\n## AgentInc nope\n\nNotes").is_none());
    }

    #[test]
    fn nested_shorter_or_different_code_fences_do_not_create_releases() {
        let release = ReleaseNotes {
            version: Version::new(0, 5, 0),
            notes: "````md\n```\n~~~\n## AgentInc 9.0.0\n````\n\n- Real changes".into(),
        };
        assert_eq!(
            parse_changelog(&changelog(std::slice::from_ref(&release))).unwrap(),
            [release]
        );
    }
}
