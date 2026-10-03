//! Seed an isolated AgentInc stack and capture one real app frame.
//!
//! Run after `cargo xtask dev` and `crates/ainc-mac/scripts/bundle.sh automation`.
//! GPUI Pilot reads the running app's retina Metal frame. No desktop input is
//! synthesized and the app is closed immediately afterward.
//! Usage: `cargo xtask readme-capture OUTPUT.png` (raw PNG path under ignored `.local/`).
use crate::release::{parse_args, proc::ManagedChild, resolve, truthy, usage_error};
use anyhow::{Result, anyhow, bail};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const TITLE: &str = "AgentInc README Capture";
const TICKETS: [(&str, &str); 7] = [
    ("Explore offline onboarding", "backlog"),
    ("Map connector permissions", "backlog"),
    ("Draft first-run checklist", "to_do"),
    ("Review sync error copy", "to_do"),
    ("Stabilize ticket import", "in_progress"),
    ("Polish keyboard navigation", "in_progress"),
    ("Ship updater smoke test", "done"),
];
const RETINA: (u64, u64) = (2720, 1656);

fn ten_seconds() -> Instant {
    Instant::now() + Duration::from_secs(10)
}

/// Run one `gpui-pilot` command against an instance and return its `output`.
fn pilot(pilot: &Path, manifest: &Path, args: &[&str], typed: Option<&str>) -> Result<Value> {
    let mut command = Command::new(pilot);
    command
        .arg("--instance")
        .arg(manifest)
        .args(["--json"])
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command.stdin(if typed.is_some() {
        Stdio::piped()
    } else {
        Stdio::inherit()
    });
    let mut child = command.spawn()?;
    if let (Some(text), Some(mut stdin)) = (typed, child.stdin.take()) {
        let _ = stdin.write_all(text.as_bytes());
    }
    let result = child.wait_with_output()?;
    let stdout = String::from_utf8_lossy(&result.stdout);
    let reply: Value = if stdout.is_empty() {
        json!({})
    } else {
        serde_json::from_str(&stdout)?
    };
    let ok = reply.get("status").and_then(Value::as_str) == Some("ok");
    if !result.status.success() || !ok {
        let detail = if truthy(Some(&reply)) {
            reply.to_string()
        } else {
            String::from_utf8_lossy(&result.stderr).into_owned()
        };
        bail!("Pilot ({}): {detail}", args.join(", "));
    }
    Ok(reply.get("output").cloned().unwrap_or(Value::Null))
}

/// The capture session: the Pilot binary and the manifest of the app being driven.
struct Session<'a> {
    pilot: &'a Path,
    manifest: PathBuf,
}

fn field<'a>(node: &'a Value, key: &str) -> Option<&'a str> {
    node.get(key).and_then(Value::as_str)
}

impl Session<'_> {
    fn call(&self, args: &[&str], typed: Option<&str>) -> Result<Value> {
        pilot(self.pilot, &self.manifest, args, typed)
    }

    fn nodes(&self) -> Result<Vec<Value>> {
        let output = self.call(&["snapshot"], None)?;
        Ok(output["snapshot"]["nodes"]
            .as_array()
            .cloned()
            .unwrap_or_default())
    }

    fn node(&self, author_id: &str) -> Result<Value> {
        self.nodes()?
            .into_iter()
            .find(|n| field(n, "author_id") == Some(author_id))
            .ok_or_else(|| anyhow!("Pilot node not found: {author_id}"))
    }

    fn reference(&self, author_id: &str) -> Result<String> {
        Ok(self.node(author_id)?["reference"]
            .as_str()
            .unwrap_or_default()
            .to_string())
    }

    /// Retry a Pilot action while its node reference goes stale between snapshot and use.
    fn retry(
        &self,
        author_id: &str,
        failure: &str,
        action: impl Fn(&str) -> Result<Value>,
    ) -> Result<Value> {
        for _ in 0..20 {
            match action(&self.reference(author_id)?) {
                Err(error) if error.to_string().contains("stale_ref") => {}
                other => return other,
            }
        }
        bail!("{failure}: {author_id}")
    }

    fn click(&self, author_id: &str) -> Result<Value> {
        self.retry(author_id, "Pilot ref stayed stale", |reference| {
            self.call(&["click", reference], None)
        })
    }

    fn type_into(&self, author_id: &str, value: &str) -> Result<Value> {
        self.click(author_id)?;
        self.retry(author_id, "Pilot input stayed stale", |reference| {
            self.call(&["type", reference, "--stdin"], Some(value))
        })
    }

    fn wait(&self, kind: &str, author_id: &str) -> Result<()> {
        if kind == "absent" {
            let deadline = ten_seconds();
            while Instant::now() < deadline {
                if !self
                    .nodes()?
                    .iter()
                    .any(|n| field(n, "author_id") == Some(author_id))
                {
                    return Ok(());
                }
            }
            bail!("Pilot node remained present: {author_id}");
        }
        let gate = json!({"kind": kind, "author_id": author_id}).to_string();
        self.call(&["wait", &gate, "10000"], None).map(drop)
    }

    fn wait_status(&self, status: &str) -> Result<()> {
        let author_id = format!("tickets.status.{status}");
        let deadline = ten_seconds();
        while Instant::now() < deadline {
            let nodes = self.nodes()?;
            let find = |id: &str| {
                nodes
                    .iter()
                    .find(|n| field(n, "author_id") == Some(id))
                    .ok_or_else(|| anyhow!("Pilot node not found: {id}"))
            };
            let (status_node, back) = (find(&author_id)?, find("tickets.back")?);
            if !truthy(status_node.get("enabled")) && truthy(back.get("enabled")) {
                return Ok(());
            }
        }
        bail!("Ticket status did not settle: {status}");
    }

    fn matches(&self, title: &str) -> Result<Vec<Value>> {
        Ok(self
            .nodes()?
            .into_iter()
            .filter(|n| {
                field(n, "name") == Some(title)
                    && field(n, "author_id").unwrap_or("").starts_with("ticket.")
            })
            .collect())
    }

    fn create_ticket(&self, title: &str, status: &str) -> Result<()> {
        if self.matches(title)?.is_empty() {
            self.click("tickets.create")?;
            self.type_into("tickets.title", title)?;
            let gate =
                json!({"kind": "value", "author_id": "tickets.title", "equals": title}).to_string();
            self.call(&["wait", &gate, "10000"], None)?;
            self.click("tickets.submit")?;
            self.wait("absent", "tickets.title")?;
        }
        let deadline = ten_seconds();
        while self.matches(title)?.is_empty() && Instant::now() < deadline {}
        let found = self.matches(title)?;
        if found.len() != 1 {
            bail!("Expected one visible Ticket: {title}");
        }
        if status == "to_do" {
            return Ok(());
        }
        self.click(field(&found[0], "author_id").unwrap_or_default())?;
        self.wait("present", "tickets.back")?;
        let button = format!("tickets.status.{status}");
        if truthy(self.node(&button)?.get("enabled")) {
            self.click(&button)?;
            self.wait_status(status)?;
        }
        self.click("tickets.back")?;
        self.wait("present", "tickets.create")
    }

    fn wait_for_manifest(&self, app: &mut ManagedChild, log_path: &Path) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(20);
        while !self.manifest.is_file() {
            if app.poll().is_some() || Instant::now() >= deadline {
                bail!("{}", fs::read_to_string(log_path)?);
            }
            // Wait only for process startup; UI uses Pilot gates.
            thread::sleep(Duration::from_millis(50));
        }
        self.call(&["hello"], None).map(drop)
    }
}

/// `Path.resolve()` for a path that may not exist yet: canonicalize the longest existing
/// prefix and normalize the rest lexically.
fn resolve_lexically(path: &Path) -> PathBuf {
    let absolute = resolve(path);
    let mut existing = absolute.clone();
    let mut rest = Vec::new();
    while !existing.exists() {
        let Some(name) = existing.file_name().map(|n| n.to_owned()) else {
            break;
        };
        rest.push(name);
        existing.pop();
    }
    let mut out = fs::canonicalize(&existing).unwrap_or(existing);
    for part in rest.iter().rev() {
        out.push(part);
    }
    let mut normal = PathBuf::new();
    for component in out.components() {
        match component {
            Component::ParentDir => {
                normal.pop();
            }
            Component::CurDir => {}
            other => normal.push(other),
        }
    }
    normal
}

/// `output` must be somewhere under `<root>/.local`.
fn checked_output(root: &Path, output: &Path) -> Option<PathBuf> {
    let output = resolve_lexically(output);
    output.starts_with(root.join(".local")).then_some(output)
}

fn spawn_app(
    app: &Path,
    session: &Path,
    env: &[(&str, String)],
    log: &fs::File,
) -> Result<ManagedChild> {
    let mut command = Command::new(app);
    command
        .arg("--gpui-pilot-session")
        .arg(session)
        .envs(env.iter().map(|(k, v)| (k, v)))
        .stdout(log.try_clone()?)
        .stderr(log.try_clone()?);
    Ok(ManagedChild::spawn(command)?)
}

fn stop(app: &mut ManagedChild) -> Result<()> {
    app.terminate();
    app.wait_timeout(Duration::from_secs(5)).map(drop)
}

/// Seed Tickets in a fresh isolated app, restart it, and screenshot the board.
fn capture(root: &Path, state: &Path, output: &Path) -> Result<()> {
    let app_path = root.join("crates/ainc-mac/dist/AgentInc Dev.app/Contents/MacOS/AgentInc");
    let pilot_path = root.join("target/debug/gpui-pilot");
    let discovery = root.join(".local/dev/api-url");
    let env = [
        ("AGENTINC_SESSION_PATH", state.join("session.json")),
        ("AINC_DISCOVERY_FILE", discovery),
        ("AINC_LEGACY_DIR", state.join("legacy")),
        ("AGENTINC_CODEX_HOME", state.join("codex")),
    ]
    .map(|(key, path)| (key, path.display().to_string()))
    .into_iter()
    .chain([
        ("AGENTINC_WINDOW_TITLE", TITLE.to_string()),
        ("AGENTINC_CAPTURE_WORKSPACE", "Acme Inc".to_string()),
        ("AGENTINC_CAPTURE_PROFILE", "Alex".to_string()),
    ])
    .collect::<Vec<_>>();
    let log_path = state.join("app.log");
    let log = fs::File::create(&log_path)?;
    let mut app = spawn_app(&app_path, &state.join("p"), &env, &log)?;
    let result = (|| -> Result<()> {
        let mut session = Session {
            pilot: &pilot_path,
            manifest: state.join("p/instance.json"),
        };
        session.wait_for_manifest(&mut app, &log_path)?;
        let version = session
            .node("sidebar.version")?
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_string);
        match version.as_deref() {
            Some(name) if !name.is_empty() && !name.ends_with("-dev") => {}
            other => bail!(
                "README capture needs a production-channel build: {}",
                other.unwrap_or("None")
            ),
        }
        session.click("nav.tickets")?;
        session.wait("present", "tickets.create")?;
        for (title, status) in TICKETS {
            session.create_ticket(title, status)?;
        }
        stop(&mut app)?;
        app = spawn_app(&app_path, &state.join("q"), &env, &log)?;
        session.manifest = state.join("q/instance.json");
        session.wait_for_manifest(&mut app, &log_path)?;
        session.click("nav.tickets")?;
        let deadline = ten_seconds();
        loop {
            if session
                .nodes()?
                .iter()
                .any(|n| field(n, "name") == Some("Ship updater smoke test"))
            {
                break;
            }
            if Instant::now() >= deadline {
                bail!("Seeded Tickets did not load");
            }
        }
        let visible: Vec<Option<String>> = session
            .nodes()?
            .iter()
            .filter(|n| field(n, "author_id").unwrap_or("").starts_with("ticket."))
            .map(|n| field(n, "name").map(str::to_string))
            .collect();
        let mut seen = visible.clone();
        seen.sort();
        let mut expected: Vec<Option<String>> = TICKETS
            .iter()
            .map(|(title, _)| Some(title.to_string()))
            .collect();
        expected.sort();
        if seen != expected {
            bail!("Capture stack contains unexpected Tickets: {visible:?}");
        }
        if session
            .nodes()?
            .iter()
            .any(|n| field(n, "author_id") == Some("close-evee"))
        {
            session.click("close-evee")?;
            session.wait("absent", "close-evee")?;
        }
        let capture = session.call(&["screenshot"], None)?;
        if (capture["width"].as_u64(), capture["height"].as_u64())
            != (Some(RETINA.0), Some(RETINA.1))
        {
            bail!("Unexpected retina frame: {capture}");
        }
        fs::copy(capture["path"].as_str().unwrap_or_default(), output)?;
        println!(
            "Captured real app frame {}: {}",
            capture["frame"],
            output.display()
        );
        Ok(())
    })();
    app.terminate();
    if app.wait_timeout(Duration::from_secs(5)).is_err() {
        app.kill();
        let _ = app.wait_timeout(Duration::from_secs(5));
    }
    result
}

pub fn cli(root: &Path, args: &[String]) -> Result<()> {
    let opts = parse_args("readme-capture", args, &[], &[]);
    let [output] = opts.positional.as_slice() else {
        usage_error("readme-capture", "expected exactly one argument: output");
    };
    let root = resolve(root);
    let app = root.join("crates/ainc-mac/dist/AgentInc Dev.app/Contents/MacOS/AgentInc");
    if !app.is_file()
        || !root.join("target/debug/gpui-pilot").is_file()
        || !root.join(".local/dev/api-url").is_file()
    {
        usage_error(
            "readme-capture",
            "start the isolated stack and build the automation bundle first",
        );
    }
    let Some(output) = checked_output(&root, Path::new(output)) else {
        usage_error(
            "readme-capture",
            "raw capture must stay under this worktree's ignored .local/",
        );
    };
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let state = tempfile::Builder::new()
        .prefix("readme-")
        .tempdir_in(root.join(".local"))?;
    capture(&root, state.path(), &output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// A stand-in `gpui-pilot`: echoes a canned reply per sub-command and records stdin.
    fn fake_pilot(dir: &Path) -> PathBuf {
        let path = dir.join("gpui-pilot");
        let script = r#"#!/bin/sh
# args: --instance M --json CMD ...
cmd="$4"
case "$cmd" in
  snapshot) echo '{"status":"ok","output":{"snapshot":{"nodes":[{"author_id":"a","name":"A","reference":"r1","enabled":true},{"author_id":null,"name":"B"}]}}}' ;;
  type) cat > "$(dirname "$2")/typed"; echo '{"status":"ok","output":{}}' ;;
  stale) echo '{"status":"error","code":"stale_ref"}'; exit 1 ;;
  *) echo '{"status":"ok","output":{"echo":"'"$cmd"'"}}' ;;
esac
"#;
        fs::write(&path, script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    #[test]
    fn pilot_replies_are_parsed_and_failures_carry_the_reply() {
        let dir = tempfile::tempdir().unwrap();
        let bin = fake_pilot(dir.path());
        let manifest = dir.path().join("instance.json");
        let session = Session {
            pilot: &bin,
            manifest: manifest.clone(),
        };
        assert_eq!(session.call(&["hello"], None).unwrap()["echo"], "hello");
        let nodes = session.nodes().unwrap();
        assert_eq!(nodes.len(), 2);
        assert_eq!(session.reference("a").unwrap(), "r1");
        assert_eq!(
            session.node("missing").unwrap_err().to_string(),
            "Pilot node not found: missing"
        );
        session
            .call(&["type", "r1", "--stdin"], Some("hello text"))
            .unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join("typed")).unwrap(),
            "hello text"
        );
        let error = session.call(&["stale"], None).unwrap_err().to_string();
        assert!(
            error.starts_with("Pilot (stale): ") && error.contains("stale_ref"),
            "{error}"
        );
    }

    #[test]
    fn stale_refs_are_retried_a_bounded_number_of_times() {
        let dir = tempfile::tempdir().unwrap();
        let bin = fake_pilot(dir.path());
        let session = Session {
            pilot: &bin,
            manifest: dir.path().join("instance.json"),
        };
        let attempts = std::cell::Cell::new(0);
        let error = session
            .retry("a", "Pilot ref stayed stale", |reference| {
                assert_eq!(reference, "r1");
                attempts.set(attempts.get() + 1);
                session.call(&["stale"], None)
            })
            .unwrap_err();
        assert_eq!(attempts.get(), 20);
        assert_eq!(error.to_string(), "Pilot ref stayed stale: a");
        // Other errors are not retried.
        attempts.set(0);
        let error = session
            .retry("a", "x", |_| {
                attempts.set(attempts.get() + 1);
                bail!("boom")
            })
            .unwrap_err();
        assert_eq!((attempts.get(), error.to_string().as_str()), (1, "boom"));
    }

    #[test]
    fn output_must_stay_under_the_worktree_local_directory() {
        let dir = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(dir.path()).unwrap();
        fs::create_dir_all(root.join(".local")).unwrap();
        let inside = root.join(".local/new/shot.png");
        assert_eq!(checked_output(&root, &inside), Some(inside.clone()));
        assert!(checked_output(&root, &root.join(".local/../shot.png")).is_none());
        assert!(checked_output(&root, &root.join("docs/shot.png")).is_none());
        assert!(checked_output(&root, &root.join(".localish/shot.png")).is_none());
    }
}
