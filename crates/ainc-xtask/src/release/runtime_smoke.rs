//! Fresh isolated profile: boot without external services, mutate, drain, restart.
//! State gates only; no fixed test sleeps. Own child and profile exclusively.
use super::{http, parse_args, proc::ManagedChild, read_to_string, resolve, usage_error};
use anyhow::{Result, anyhow, bail, ensure};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    env,
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, PipeReader, Write},
    os::{fd::AsRawFd, unix::process::CommandExt},
    path::PathBuf,
    process::Command,
    time::Duration,
};

struct Smoke {
    bundle: PathBuf,
    root: PathBuf,
    version: String,
    env: HashMap<String, String>,
    blocked_signals: bool,
}

/// Held for the daemon's life: its stdout pipe is never drained after readiness, as before.
struct Daemon {
    child: ManagedChild,
    _output: BufReader<PipeReader>,
}

fn inherited_signals() -> std::io::Result<()> {
    // Runs between fork and exec: only async-signal-safe calls.
    unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        libc::sigaddset(&mut set, libc::SIGCHLD);
        libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut());
        libc::signal(libc::SIGCHLD, libc::SIG_IGN);
    }
    Ok(())
}

/// Readiness requires a successful health-check child; it must have been reaped.
pub fn check_reaped(ps: &str, daemon: i32) -> Result<()> {
    // Like `line.strip().split(None, 3)`: three fields, then the rest of the line.
    let entries: Vec<Vec<&str>> = ps
        .lines()
        .filter_map(|line| {
            let mut rest = line.trim();
            let mut row = Vec::new();
            for _ in 0..3 {
                let end = rest.find(char::is_whitespace)?;
                row.push(&rest[..end]);
                rest = rest[end..].trim_start();
            }
            row.push(rest);
            Some(row)
        })
        .collect();
    let number = |row: &Vec<&str>, index: usize| row[index].trim().parse::<i32>().unwrap_or(-1);
    let children: Vec<i32> = entries
        .iter()
        .filter(|row| number(row, 1) == daemon)
        .map(|row| number(row, 0))
        .collect();
    ensure!(
        entries
            .iter()
            .any(|row| children.contains(&number(row, 0)) && row[3].contains("--local-runtime")),
        "no --local-runtime child of the daemon"
    );
    ensure!(
        !entries
            .iter()
            .any(|row| children.contains(&number(row, 1)) && row[2].contains('Z')),
        "runtime helper left a zombie health-check child"
    );
    Ok(())
}

impl Smoke {
    fn request(&self, path: &str, body: Option<&Value>) -> Result<Option<Value>> {
        let token = read_to_string(&self.root.join("owner-token"))?;
        let url = read_to_string(&self.root.join("api-url"))?;
        let headers = [
            ("Authorization", format!("Bearer {}", token.trim())),
            ("Agent-Inc-Client", format!("mac/{} (api 1)", self.version)),
            ("Content-Type", "application/json".to_string()),
        ];
        let payload = body.map(|b| b.to_string());
        let body = http::request(
            &format!("{}{path}", url.trim()),
            if body.is_none() { "GET" } else { "POST" },
            &headers,
            payload.as_deref().map(str::as_bytes),
            Duration::from_secs(10),
        )?;
        Ok(if body.is_empty() {
            None
        } else {
            Some(serde_json::from_slice(&body)?)
        })
    }

    fn start(&self) -> Result<Daemon> {
        let (reader, writer) = std::io::pipe()?;
        let mut command = Command::new(self.bundle.join("Contents/MacOS/aincd"));
        command
            .env_clear()
            .envs(&self.env)
            .stdout(writer.try_clone()?)
            .stderr(writer);
        if self.blocked_signals {
            unsafe { command.pre_exec(inherited_signals) };
        }
        let mut child = ManagedChild::spawn(command)?;
        let mut output = BufReader::new(reader);
        loop {
            // Lines already buffered are read before waiting on the pipe again.
            if output.buffer().is_empty() {
                let mut poll = libc::pollfd {
                    fd: output.get_ref().as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                };
                if unsafe { libc::poll(&mut poll, 1, 90_000) } == 0 {
                    break;
                }
            }
            let mut line = String::new();
            if output.read_line(&mut line)? == 0 {
                let status = child.wait_timeout(Duration::from_secs(600))?;
                bail!("daemon exited: {}", status.code().unwrap_or(-1));
            }
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(self.root.join("daemon.log"))?
                .write_all(line.as_bytes())?;
            if line.contains(ainc_release::DAEMON_READY) {
                if self.blocked_signals {
                    let ps = super::output("ps", &["-axo", "pid=,ppid=,stat=,command="])?;
                    check_reaped(&ps, child.id())?;
                }
                return Ok(Daemon {
                    child,
                    _output: output,
                });
            }
        }
        child.terminate();
        child.wait_timeout(Duration::from_secs(600))?;
        bail!("daemon readiness deadline");
    }

    fn drained(&self, daemon: Daemon) -> Result<()> {
        let mut daemon = daemon;
        self.request("/internal/drain", Some(&json!({})))?;
        let status = daemon.child.wait_timeout(Duration::from_secs(120))?;
        ensure!(status.code() == Some(0), "daemon exit status {status}");
        Ok(())
    }

    fn log(&self, name: &str) -> Result<File> {
        Ok(File::create(self.root.join(name))?)
    }

    fn cargo(&self, log: &File, env: &[(&str, String)], args: &[&str]) -> Result<()> {
        let mut command = crate::spawn::cargo();
        command
            .args(args)
            .envs(env.iter().map(|(k, v)| (*k, v.as_str())))
            .stdout(log.try_clone()?)
            .stderr(log.try_clone()?);
        super::checked(&mut command)
    }

    fn scenario(
        &self,
        workspace_tests: bool,
        pilot: bool,
        child: &mut Option<Daemon>,
    ) -> Result<()> {
        *child = Some(self.start()?);
        ensure!(
            self.request("/health/ready", None)?
                .is_some_and(|v| v["status"] == "ready"),
            "daemon is not ready"
        );
        self.request(
            "/v1/conversations/commands",
            Some(&json!({
                "operation_id": uuid::Uuid::new_v4().to_string(),
                "command": {"kind": "create_conversation"},
            })),
        )?;
        let mut first = self.request("/v1/conversations", None)?;
        println!(
            "Fresh bundled runtime ready{}",
            if self.blocked_signals {
                " with inherited blocked/ignored SIGCHLD; health-check child reaped"
            } else {
                ""
            }
        );
        if workspace_tests {
            let postmaster = read_to_string(&self.root.join("runtime/postgres/postmaster.pid"))?;
            let port = postmaster
                .lines()
                .nth(3)
                .ok_or_else(|| anyhow!("postmaster.pid has no port"))?;
            let password = read_to_string(&self.root.join("runtime/postgres-password"))?;
            let url = format!("postgres://agentinc:{password}@127.0.0.1:{port}/postgres");
            self.cargo(
                &self.log("workspace-tests.log")?,
                &[("DATABASE_URL", url), ("CARGO_INCREMENTAL", "0".into())],
                &["test", "--locked", "--workspace"],
            )?;
        }
        if pilot {
            let pilot_env = [
                (
                    "AINC_DISCOVERY_FILE",
                    self.root.join("api-url").to_string_lossy().into_owned(),
                ),
                ("CARGO_INCREMENTAL", "0".to_string()),
            ];
            let log = self.log("pilot.log")?;
            self.cargo(
                &log,
                &pilot_env,
                &[
                    "build",
                    "--locked",
                    "-p",
                    "ainc-mac",
                    "-p",
                    "gpui-pilot-cli",
                    "--features",
                    "ainc-mac/automation",
                ],
            )?;
            self.cargo(
                &log,
                &pilot_env,
                &[
                    "test",
                    "--locked",
                    "-p",
                    "ainc-mac",
                    "--features",
                    "automation",
                    "--test",
                    "pilot_acceptance",
                    "--",
                    "--nocapture",
                ],
            )?;
            first = self.request("/v1/conversations", None)?;
        }
        self.drained(child.take().unwrap())?;
        ensure!(
            !self.root.join("api-url").exists(),
            "discovery file remained after drain"
        );
        *child = Some(self.start()?);
        ensure!(
            self.request("/v1/conversations", None)? == first,
            "state changed across restart"
        );
        println!("Restart retained state; runtime recovered on fresh ports");
        self.drained(child.take().unwrap())?;
        println!("Drain completed and removed discovery");
        let mut killed = self.start()?;
        killed.child.kill();
        killed.child.wait_timeout(Duration::from_secs(30))?;
        *child = Some(self.start()?);
        ensure!(
            self.request("/v1/conversations", None)? == first,
            "state changed after hard kill"
        );
        self.drained(child.take().unwrap())?;
        println!("Hard-killed daemon recovered without orphaned-runtime interference");
        Ok(())
    }
}

pub fn cli(args: &[String]) -> Result<()> {
    let opts = parse_args(
        "release-runtime-smoke",
        args,
        &["--workspace-tests", "--pilot", "--blocked-signals"],
        &[],
    );
    let [bundle, profile] = &opts.positional[..] else {
        usage_error(
            "release-runtime-smoke",
            "expected arguments: bundle profile",
        );
    };
    let bundle = PathBuf::from(bundle);
    let release: Value = serde_json::from_str(&read_to_string(
        &bundle.join("Contents/Resources/release.json"),
    )?)?;
    let version = release["version"]
        .as_str()
        .ok_or_else(|| anyhow!("release.json has no version"))?
        .to_string();
    let profile = PathBuf::from(profile);
    let absolute = resolve(&profile);
    if let Some(parent) = absolute.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(&absolute)?;
    let root = fs::canonicalize(&absolute)?;
    let mut env: HashMap<String, String> = env::vars().collect();
    let path = |name: &str| root.join(name).to_string_lossy().into_owned();
    env.insert("AINC_DISCOVERY_FILE".into(), path("api-url"));
    env.insert("AINC_LEGACY_DIR".into(), path("legacy"));
    env.insert("AINC_CODEX_HOME".into(), path("codex"));
    env.insert("RUST_LOG".into(), "info".into());
    for name in ["DATABASE_URL", "AINC_RUNTIME_CONFIG", "AINC_DATABASE_URL"] {
        env.remove(name);
    }
    let smoke = Smoke {
        bundle: resolve(&bundle),
        root,
        version,
        env,
        blocked_signals: opts.flag("--blocked-signals"),
    };
    let mut child: Option<Daemon> = None;
    let result = smoke.scenario(
        opts.flag("--workspace-tests"),
        opts.flag("--pilot"),
        &mut child,
    );
    if let Some(mut daemon) = child
        && daemon.child.poll().is_none()
    {
        daemon.child.terminate();
        daemon.child.wait_timeout(Duration::from_secs(120))?;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const PS: &str = "  100     1 Ss   /bundle/aincd\n  101   100 S    /bundle/runtime/temporal --local-runtime\n  102   101 S    pg_ctl\n";

    #[test]
    fn reaped_health_check_children_pass() {
        check_reaped(PS, 100).unwrap();
    }

    #[test]
    fn zombie_descendants_and_missing_runtime_fail() {
        let zombie = format!("{PS}  103   101 Z    (health)\n");
        assert!(
            check_reaped(&zombie, 100)
                .unwrap_err()
                .to_string()
                .contains("zombie")
        );
        assert!(check_reaped("  100     1 Ss   /bundle/aincd\n", 100).is_err());
    }
}
