//! Conventional Commit subjects for the `commit-msg` Git hook.
use anyhow::{Context, Result, ensure};
use regex::Regex;
use std::{fs, path::Path};

fn validate(message: &str) -> Result<()> {
    ensure!(
        !message.contains('\u{2014}'),
        "commit messages must not contain em dashes (U+2014); use a full stop, comma or hyphen"
    );
    let subject = message.lines().next().unwrap_or_default();
    let pattern = Regex::new(r"^[a-z]+(?:\([^\s()]+\))?!?: \S.*$")?;
    ensure!(
        pattern.is_match(subject),
        "commit subject must use Conventional Commits: type(scope)!: description\n\
         Scope and ! are optional; type must be lowercase and description nonempty.\n\
         Examples: feat: add tickets, fix(ui): restore focus, chore(release): release 0.5.0\n\
         Received: {subject:?}"
    );
    Ok(())
}

pub fn run(args: &[String]) -> Result<()> {
    let [path] = args else {
        anyhow::bail!("usage: cargo xtask check-commit-msg MESSAGE_FILE");
    };
    let message = fs::read_to_string(Path::new(path))
        .with_context(|| format!("read commit message {path}"))?;
    validate(&message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_conventional_subjects_and_optional_bodies() {
        for message in [
            "feat: add tickets",
            "fix(ui): restore focus",
            "feat!: change API",
            "feat(sdk)!: change API\n\nBREAKING CHANGE: remove old method\n",
            "chore(release): release 0.5.0",
            "docs: explain café setup\r\n\r\nMore detail.\r\n",
            "test(ainc-xtask): cover hooks",
            "revert: undo previous change",
            "custom: allow other conventional types",
        ] {
            validate(message).unwrap_or_else(|error| panic!("{message:?}: {error}"));
        }
    }

    #[test]
    fn rejects_nonconventional_or_empty_subjects() {
        for message in [
            "",
            "\nfeat: hidden on second line",
            "Add tickets",
            "Release 0.5.0",
            "Feat: add tickets",
            "feat:add tickets",
            "feat: ",
            "feat:   ",
            "feat: \t",
            "feat(): add tickets",
            "feat(two words): add tickets",
            "feat((ui)): add tickets",
            "feat!!: add tickets",
            " feat: add tickets",
            "fixup! feat: add tickets",
            "Merge branch 'feature'",
        ] {
            assert!(validate(message).is_err(), "accepted {message:?}");
        }
    }

    #[test]
    fn rejects_em_dashes_in_subject_and_body() {
        for message in [
            "fix: remove punctuation\u{2014}please",
            "fix: remove punctuation\n\nNo em dashes\u{2014}in the body either.",
        ] {
            assert!(
                validate(message)
                    .unwrap_err()
                    .to_string()
                    .contains("em dashes")
            );
        }
    }

    #[test]
    fn reads_the_message_file_and_reports_missing_arguments_or_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("commit message");
        let args = [path.to_string_lossy().into_owned()];
        assert!(run(&[]).is_err());
        assert!(run(&args).is_err());
        fs::write(&path, "fix: validate commit messages\n").unwrap();
        run(&args).unwrap();
        fs::write(&path, "not conventional\n").unwrap();
        assert!(run(&args).is_err());
    }
}
