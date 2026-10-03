mod checks;
mod readme;
mod release;
mod spawn;
mod vendor_pilot_gpui;

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::Duration,
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct Instance {
    id: String,
    path: PathBuf,
    namespace: String,
    tilt_port: u16,
}

fn identity(path: &Path) -> Result<Instance> {
    let path = path.canonicalize().context("canonicalize worktree")?;
    let hash = Sha256::digest(path.to_string_lossy().as_bytes());
    let short = hash[..6]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let id = format!("agentinc-{short}");
    let tilt_port = 11000 + u16::from_be_bytes([hash[6], hash[7]]) % 40000;
    Ok(Instance {
        namespace: id.clone(),
        id,
        path,
        tilt_port,
    })
}

fn root() -> Result<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()?;
    if !output.status.success() {
        bail!("not in an AgentInc worktree");
    }
    Ok(PathBuf::from(String::from_utf8(output.stdout)?.trim()))
}

fn local(instance: &Instance) -> PathBuf {
    instance.path.join(".local/dev")
}

fn free_tilt_port(start: u16) -> Result<u16> {
    for offset in 0..100 {
        let candidate = start
            .checked_add(offset)
            .context("Tilt port range exhausted")?;
        if TcpListener::bind(("127.0.0.1", candidate)).is_ok() {
            return Ok(candidate);
        }
    }
    bail!("no free Tilt port near {start}")
}

fn write_instance(instance: &Instance) -> Result<Instance> {
    let dir = local(instance);
    fs::create_dir_all(&dir)?;
    let path = dir.join("instance.json");
    let mut selected = instance.clone();
    if path.exists() {
        let old: Instance = serde_json::from_slice(&fs::read(&path)?)?;
        if old.id != instance.id || old.path != instance.path || old.namespace != instance.namespace
        {
            bail!("existing instance identity differs from canonical worktree path");
        }
        selected.tilt_port = old.tilt_port;
    } else {
        selected.tilt_port = free_tilt_port(instance.tilt_port)?;
    }
    fs::write(path, serde_json::to_vec_pretty(&selected)?)?;
    fs::write(
        dir.join("compose.env"),
        format!(
            "AINC_DB_SUFFIX={}\n",
            instance.id.trim_start_matches("agentinc-")
        ),
    )?;
    Ok(selected)
}

fn cmd(instance: &Instance, program: &str) -> Command {
    let mut command = spawn::command(program);
    command
        .current_dir(&instance.path)
        .env("AINC_INSTANCE", &instance.id)
        .env("TILT_DEV_DIR", local(instance).join("tilt"));
    command
}

fn compose(instance: &Instance, args: &[&str]) -> Result<String> {
    let output = cmd(instance, "docker")
        .args([
            "compose",
            "--env-file",
            ".local/dev/compose.env",
            "-p",
            &instance.id,
            "-f",
            "dev/compose.yaml",
        ])
        .args(args)
        .output()
        .context("docker compose")?;
    if !output.status.success() {
        bail!(
            "docker compose failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8(output.stdout)?.trim().into())
}

fn port(instance: &Instance, service: &str, inner: &str) -> Result<u16> {
    let output = compose(instance, &["port", service, inner])?;
    output
        .rsplit(':')
        .next()
        .context("missing Compose port")?
        .parse()
        .context("parse Compose port")
}

fn temporal(instance: &Instance, args: &[&str]) -> Result<String> {
    let addr = format!("127.0.0.1:{}", port(instance, "temporal", "7233")?);
    let output = cmd(instance, "temporal")
        .args(["--address", &addr])
        .args(args)
        .output()?;
    if !output.status.success() {
        bail!(
            "temporal command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8(output.stdout)?)
}

fn bootstrap(instance: &Instance) -> Result<()> {
    // Tilt can start a dependent local process while Compose is still exposing
    // its port. Poll the server's actual health instead of assuming a delay.
    let mut ready = false;
    for _ in 0..120 {
        if temporal(instance, &["operator", "cluster", "health"]).is_ok() {
            ready = true;
            break;
        }
        thread::sleep(Duration::from_secs(1));
    }
    if !ready {
        bail!("Temporal did not become healthy");
    }
    if temporal(
        instance,
        &[
            "operator",
            "namespace",
            "describe",
            "--namespace",
            &instance.namespace,
        ],
    )
    .is_err()
    {
        temporal(
            instance,
            &[
                "operator",
                "namespace",
                "create",
                "--namespace",
                &instance.namespace,
                "--retention",
                "3d",
            ],
        )?;
    }
    let mut visible = false;
    for _ in 0..30 {
        if temporal(
            instance,
            &[
                "operator",
                "search-attribute",
                "list",
                "--namespace",
                &instance.namespace,
            ],
        )
        .is_ok()
        {
            visible = true;
            break;
        }
        thread::sleep(Duration::from_secs(1));
    }
    if !visible {
        bail!("Temporal namespace was created but did not become visible");
    }
    for name in [
        "WorkspaceId",
        "TicketId",
        "ConversationId",
        "AutomationId",
        "AgentId",
    ] {
        // The server refuses a conflicting type. A matching existing attribute is harmless.
        let result = temporal(
            instance,
            &[
                "operator",
                "search-attribute",
                "create",
                "--namespace",
                &instance.namespace,
                "--name",
                name,
                "--type",
                "Keyword",
            ],
        );
        if let Err(error) = result {
            let existing = temporal(
                instance,
                &[
                    "operator",
                    "search-attribute",
                    "list",
                    "--namespace",
                    &instance.namespace,
                ],
            )?;
            let matches = existing.lines().any(|line| {
                let mut fields = line.split_whitespace();
                fields.next() == Some(name) && fields.next() == Some("Keyword")
            });
            if !matches {
                return Err(error);
            }
        }
    }
    Ok(())
}

fn daemon(instance: &Instance) -> Result<()> {
    bootstrap(instance)?;
    let pg = port(instance, "postgres", "5432")?;
    let db = format!(
        "postgres://agentinc:agentinc@127.0.0.1:{pg}/agentinc_{}",
        instance.id.trim_start_matches("agentinc-")
    );
    let discovery = local(instance).join("api-url");
    let runtime = serde_json::json!({
        "endpoint": format!("http://127.0.0.1:{}", port(instance, "temporal", "7233")?),
        "scope": instance.namespace,
        "worker_group": format!("{}-agents", instance.id),
        "ui_url": format!("http://127.0.0.1:{}", port(instance, "temporal-ui", "8080")?),
    });
    fs::write(
        local(instance).join("runtime.json"),
        serde_json::to_vec_pretty(&runtime)?,
    )?;
    let status = cmd(instance, "cargo")
        .args(["run", "-p", "ainc-daemon", "--bin", "aincd"])
        .env("DATABASE_URL", db)
        .env("AINC_RUNTIME_CONFIG", runtime.to_string())
        .env("AINC_WORKSPACE_DIR", local(instance).join("workspace"))
        .env(
            "AINC_TOOL_ALLOW",
            "[\"read_file\",\"write_file\",\"shell\",\"git\"]",
        )
        .env("AINC_DISCOVERY_FILE", discovery)
        .env("AINC_LEGACY_DIR", local(instance).join("legacy"))
        .env("AGENTINC_CODEX_HOME", local(instance).join("codex"))
        .status()?;
    if !status.success() {
        bail!("aincd exited with {status}");
    }
    Ok(())
}

/// `generate` lives in its own crate (`ainc-generate`) so xtask never links the daemon.
fn generate(root: &Path, args: Vec<String>) -> Result<()> {
    let status = spawn::cargo()
        .args(["run", "--locked", "-p", "ainc-generate", "--"])
        .args(args)
        .current_dir(root)
        .status()
        .context("could not run cargo")?;
    anyhow::ensure!(status.success(), "generate failed");
    Ok(())
}

/// Run one command from the repo root and fail the check if it fails.
fn step(root: &Path, command: &[&str]) -> Result<()> {
    println!("$ {}", command.join(" "));
    let status = spawn::command(command[0])
        .args(&command[1..])
        .current_dir(root)
        .status()
        .with_context(|| format!("could not run {}", command[0]))?;
    anyhow::ensure!(status.success(), "check failed: {}", command.join(" "));
    Ok(())
}

/// The native UI rules: colors, spacing and component forks. CI runs this on its own, after
/// its tests have built xtask; `check` runs it as part of the static gate.
fn check_ui(root: &Path) -> Result<()> {
    let app = root.join("crates/ainc-mac");
    checks::colors::run(&app)?;
    checks::ui_spacing::run(&app)?;
    checks::ui_core::run(&app)
}

/// The static gate: everything CI checks that needs no database and no test run.
/// `just check`, the pre-commit hook and CI all come through here.
fn check(root: &Path) -> Result<()> {
    step(root, &["cargo", "fmt", "--all", "--", "--check"])?;
    check_ui(root)?;
    step(
        root,
        &[
            "cargo",
            "clippy",
            "--locked",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    )
}

fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    let operation = args.next().ok_or_else(|| {
        anyhow!(
            "usage: cargo xtask dev|down|doctor|check|check-commit-msg|generate|vendor-pilot-gpui|{}|{}",
            release::NAMES,
            readme::NAMES
        )
    })?;
    let root = root()?;
    let instance = identity(&root)?;
    match operation.as_str() {
        op if release::handles(op) => release::run(op, args.collect(), &root),
        op if readme::handles(op) => readme::run(op, args.collect(), &root),
        "check" => check(&root),
        "check-ui" => check_ui(&root),
        "check-commit-msg" => checks::commit_msg::run(&args.collect::<Vec<_>>()),
        "vendor-pilot-gpui" => vendor_pilot_gpui::cli(&args.collect::<Vec<_>>(), &root),
        "generate" => generate(&root, args.collect()),
        "dev" => {
            let mut instance = write_instance(&instance)?;
            if TcpListener::bind(("127.0.0.1", instance.tilt_port)).is_err() {
                instance.tilt_port = free_tilt_port(instance.tilt_port + 1)?;
                fs::write(
                    local(&instance).join("instance.json"),
                    serde_json::to_vec_pretty(&instance)?,
                )?;
            }
            let mut child = cmd(&instance, "tilt")
                .args([
                    "up",
                    "--file",
                    "Tiltfile",
                    "--host",
                    "127.0.0.1",
                    "--port",
                    &instance.tilt_port.to_string(),
                ])
                .spawn()?;
            let mut listening = false;
            for _ in 0..50 {
                if let Some(status) = child.try_wait()? {
                    bail!(
                        "Tilt exited before binding port {}: {status}",
                        instance.tilt_port
                    );
                }
                if TcpStream::connect(("127.0.0.1", instance.tilt_port)).is_ok() {
                    listening = true;
                    break;
                }
                thread::sleep(Duration::from_millis(100));
            }
            if !listening {
                child.kill()?;
                bail!("Tilt did not bind port {}", instance.tilt_port);
            }
            let status = child.wait()?;
            if !status.success() {
                bail!("Tilt exited with {status}");
            }
            Ok(())
        }
        "down" => {
            let instance = write_instance(&instance)?;
            let _ = cmd(&instance, "tilt")
                .args(["down", "--file", "Tiltfile"])
                .status()?;
            compose(&instance, &["down"])?;
            Ok(())
        }
        "doctor" => {
            let instance = write_instance(&instance)?;
            println!(
                "{}\npath: {}\nTilt: http://127.0.0.1:{}\nTemporal namespace: {}",
                instance.id,
                instance.path.display(),
                instance.tilt_port,
                instance.namespace
            );
            for service in ["postgres", "temporal", "temporal-ui"] {
                println!(
                    "{service}: {}",
                    compose(&instance, &["ps", "--status", "running", "--services"])
                        .unwrap_or_default()
                        .lines()
                        .any(|line| line == service)
                );
            }
            if let Ok(url) = fs::read_to_string(local(&instance).join("api-url")) {
                println!("API: {}", url.trim());
            }
            Ok(())
        }
        "serve" => {
            let instance = write_instance(&instance)?;
            daemon(&instance)
        }
        _ => bail!("unknown xtask command: {operation}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_path_controls_identity() {
        let left = tempfile::tempdir().unwrap();
        let right = tempfile::tempdir().unwrap();
        let a = identity(left.path()).unwrap();
        let b = identity(right.path()).unwrap();
        assert_ne!(a.id, b.id);
        assert_eq!(a, identity(left.path()).unwrap());
        assert!(a.id.starts_with("agentinc-"));
    }

    #[test]
    fn occupied_tilt_port_uses_another_port() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let occupied = listener.local_addr().unwrap().port();
        if occupied <= u16::MAX - 100 {
            assert_ne!(free_tilt_port(occupied).unwrap(), occupied);
        }
    }
}
