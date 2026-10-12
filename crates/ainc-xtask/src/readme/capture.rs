//! Seed an isolated AgentInc stack and capture one real app frame.
//!
//! Run after `cargo xtask dev` and `crates/ainc-mac/scripts/bundle.sh automation`.
//! GPUI Pilot reads the running app's retina Metal frame. No desktop input is
//! synthesized and the app is closed immediately afterward.
//! Usage: `cargo xtask readme-capture OUTPUT.png` (raw PNG path under ignored `.local/`).
use crate::release::{http, parse_args, proc::ManagedChild, resolve, truthy, usage_error};
use anyhow::{Result, anyhow, bail};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const TITLE: &str = "AgentInc README Capture";
/// The open Todos the Overview and the Todos page show.
const TODOS: [&str; 5] = [
    "Book the dentist",
    "Water the plants",
    "Reply to Sam about the weekend",
    "Order a new desk lamp",
    "Renew the passport",
];
/// Upcoming Events as (title, hours from the next full hour, length in hours).
const EVENTS: [(&str, i64, i64); 4] = [
    ("Coffee with Jamie", 2, 1),
    ("Team sync", 5, 1),
    ("Dinner at Nonna's", 27, 2),
    ("Weekend hike", 50, 4),
];
const RETINA: (u64, u64) = (2720, 1656);

/// Run one `gpui-pilot-cli` command against an instance and return its `output`.
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

/// The isolated daemon of the running dev stack: seed it over the owner API.
struct Stack {
    url: String,
    token: String,
}
impl Stack {
    fn open(root: &Path) -> Result<Self> {
        let dev = root.join(".local/dev");
        Ok(Self {
            url: fs::read_to_string(dev.join("api-url"))?.trim().to_owned(),
            token: fs::read_to_string(dev.join("owner-token"))?
                .trim()
                .to_owned(),
        })
    }
    fn call(&self, path: &str, body: Option<&Value>) -> Result<Value> {
        let headers = [
            ("Authorization", format!("Bearer {}", self.token)),
            (ainc_release::CLIENT_HEADER, ainc_release::client_header()),
            ("Content-Type", "application/json".to_string()),
        ];
        let payload = body.map(Value::to_string);
        let reply = http::request(
            &format!("{}{path}", self.url),
            if body.is_none() { "GET" } else { "POST" },
            &headers,
            payload.as_deref().map(str::as_bytes),
            Duration::from_secs(10),
        )?;
        Ok(serde_json::from_slice(&reply)?)
    }
    fn command(&self, family: &str, command: Value) -> Result<Value> {
        self.call(
            &format!("/v1/{family}/commands"),
            Some(&json!({"operation_id": uuid::Uuid::new_v4().to_string(), "command": command})),
        )
    }
    /// Seed the Todos and Events once; a rerun against the same stack adds nothing.
    fn seed(&self) -> Result<()> {
        let todos = self.call("/v1/todos", None)?;
        let existing: Vec<&str> = todos["todos"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|t| field(t, "title"))
            .collect();
        for title in TODOS {
            if !existing.contains(&title) {
                self.command("todos", json!({"kind": "add", "title": title}))?;
            }
        }
        let events = self.call("/v1/calendar", None)?;
        let existing: Vec<&str> = events["events"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|e| field(e, "title"))
            .collect();
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
        let next_hour = now - now.rem_euclid(3600) + 3600;
        for (title, offset, length) in EVENTS {
            if !existing.contains(&title) {
                let starts_at = next_hour + offset * 3600;
                self.command(
                    "calendar",
                    json!({"kind": "create", "title": title, "starts_at": starts_at, "ends_at": starts_at + length * 3600}),
                )?;
            }
        }
        Ok(())
    }
}

/// Seed the stack, open a fresh isolated app on the Overview and screenshot it.
fn capture(root: &Path, state: &Path, output: &Path) -> Result<()> {
    let app_path = root.join("crates/ainc-mac/dist/AgentInc Dev.app/Contents/MacOS/AgentInc");
    let pilot_path = root.join("target/debug/gpui-pilot-cli");
    let discovery = root.join(".local/dev/api-url");
    Stack::open(root)?.seed()?;
    let env = [
        ("AINC_SESSION_PATH", state.join("session.json")),
        ("AINC_DISCOVERY_FILE", discovery),
    ]
    .map(|(key, path)| (key, path.display().to_string()))
    .into_iter()
    .chain([
        ("AINC_WINDOW_TITLE", TITLE.to_string()),
        ("AINC_CAPTURE_PROFILE", "Alex".to_string()),
    ])
    .collect::<Vec<_>>();
    let log_path = state.join("app.log");
    let log = fs::File::create(&log_path)?;
    let mut app = spawn_app(&app_path, &state.join("p"), &env, &log)?;
    let result = (|| -> Result<()> {
        let session = Session {
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
        // The launch overlay covers the window until the first snapshot arrives.
        let deadline = Instant::now() + Duration::from_secs(30);
        while session
            .nodes()?
            .iter()
            .any(|n| field(n, "author_id") == Some("shell.launch"))
        {
            if Instant::now() >= deadline {
                bail!("The app did not finish starting");
            }
            thread::sleep(Duration::from_millis(200));
        }
        // The Todos page names each row's checkbox after its title, so it shows when
        // the seeded records have arrived; the Overview then has them too.
        session.click("nav.todos")?;
        loop {
            let names: Vec<String> = session
                .nodes()?
                .iter()
                .filter_map(|n| field(n, "name").map(str::to_owned))
                .collect();
            if TODOS.iter().all(|t| names.iter().any(|n| n == t)) {
                break;
            }
            if Instant::now() >= deadline {
                bail!("Seeded Todos did not load: {names:?}");
            }
            thread::sleep(Duration::from_millis(200));
        }
        session.click("nav.overview")?;
        thread::sleep(Duration::from_millis(800));
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
        || !root.join("target/debug/gpui-pilot-cli").is_file()
        || !root.join(".local/dev/api-url").is_file()
        || !root.join(".local/dev/owner-token").is_file()
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

    /// A stand-in `gpui-pilot-cli`: echoes a canned reply per sub-command and records stdin.
    fn fake_pilot(dir: &Path) -> PathBuf {
        let path = dir.join("gpui-pilot-cli");
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
