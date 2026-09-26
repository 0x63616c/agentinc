use anyhow::{Context, Result, anyhow, bail};
use progenitor::{GenerationSettings, Generator, InterfaceStyle};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    io::Write,
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Command, Stdio},
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
    let mut command = Command::new(program);
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

fn generate(root: &Path, check: bool) -> Result<()> {
    let mut spec = ainc_daemon::openapi();
    // This API uses the common 3.0/3.1 schema subset. Validate that assumption
    // before changing the dialect marker; never silently reinterpret nullability.
    fn compatible(value: &serde_json::Value) -> Result<()> {
        match value {
            serde_json::Value::Object(map) => {
                for (key, child) in map {
                    if [
                        "const",
                        "if",
                        "then",
                        "else",
                        "unevaluatedProperties",
                        "$schema",
                    ]
                    .contains(&key.as_str())
                    {
                        bail!("OpenAPI 3.1-only schema key: {key}");
                    }
                    if key == "type" && child.is_array() {
                        bail!("OpenAPI 3.1 union type needs conversion");
                    }
                    compatible(child)?;
                }
            }
            serde_json::Value::Array(items) => {
                for child in items {
                    compatible(child)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    // Utoipa represents nullable primitives as a JSON Schema type array.
    // Convert exactly that shape to the OpenAPI 3.0 nullable keyword.
    fn nullable(value: &mut serde_json::Value) -> Result<()> {
        match value {
            serde_json::Value::Object(map) => {
                if let Some(serde_json::Value::Array(types)) = map.get("type") {
                    let non_null: Vec<_> =
                        types.iter().filter(|v| **v != "null").cloned().collect();
                    if types.len() != 2 || non_null.len() != 1 {
                        bail!("unsupported schema type union");
                    }
                    map.insert("type".into(), non_null[0].clone());
                    map.insert("nullable".into(), true.into());
                }
                for child in map.values_mut() {
                    nullable(child)?;
                }
            }
            serde_json::Value::Array(items) => {
                for child in items {
                    nullable(child)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    nullable(&mut spec)?;
    compatible(&spec)?;
    if let Some(license) = spec["info"]["license"].as_object_mut() {
        license.remove("identifier"); // OpenAPI 3.1 field; name remains for 3.0.3.
    }
    spec["openapi"] = "3.0.3".into();
    let spec_text = format!("{}\n", serde_json::to_string_pretty(&spec)?);
    let parsed: openapiv3::OpenAPI = serde_json::from_value(spec)?;
    let mut settings = GenerationSettings::default();
    settings
        .with_interface(InterfaceStyle::Builder)
        .with_derive("schemars::JsonSchema")
        .with_pre_hook_async(syn::parse_quote!(crate::client_header))
        .with_post_hook_async(syn::parse_quote!(crate::server_compatibility));
    let mut generator = Generator::new(&settings);
    fn format(source: String) -> Result<String> {
        // Progenitor emits block-doc examples whose fences rustfmt indents into
        // invalid doctests. Keep them as comments in the checked-in output.
        let source = source.replace("/**", "/*");
        let mut rustfmt = Command::new("rustfmt")
            .args(["--edition", "2024", "--emit", "stdout"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;
        rustfmt
            .stdin
            .take()
            .context("rustfmt stdin")?
            .write_all(source.as_bytes())?;
        let output = rustfmt.wait_with_output()?;
        if !output.status.success() {
            bail!("rustfmt failed");
        }
        Ok(String::from_utf8(output.stdout)?)
    }
    let client = format(prettyplease::unparse(&syn::parse2(
        generator.generate_tokens(&parsed)?,
    )?))?;
    // Progenitor clones Copy query parameters (for example `bool`), which
    // strict Clippy rejects; the generated CLI is not hand-maintained.
    let cli = format!(
        "#![allow(clippy::clone_on_copy)]\n{}",
        format(prettyplease::unparse(&syn::parse2(
            generator.cli(&parsed, "ainc_client")?,
        )?))?
    );
    for (path, content) in [
        (root.join("api/openapi-3.0.json"), spec_text),
        (root.join("crates/ainc-client/src/generated.rs"), client),
        (root.join("crates/ainc-cli/src/generated.rs"), cli),
    ] {
        if check {
            if fs::read_to_string(&path).ok().as_deref() != Some(&content) {
                bail!("generated file differs: {}", path.display());
            }
        } else {
            fs::create_dir_all(path.parent().context("generated file parent")?)?;
            fs::write(path, content)?;
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    let operation = args
        .next()
        .ok_or_else(|| anyhow!("usage: cargo xtask dev|down|doctor|generate"))?;
    let root = root()?;
    let instance = identity(&root)?;
    match operation.as_str() {
        "release" => {
            let status = Command::new("python3")
                .arg(root.join("scripts/release/prepare.py"))
                .args(args)
                .status()?;
            anyhow::ensure!(status.success(), "release preparation failed");
            Ok(())
        }
        "generate" => generate(&root, args.next().as_deref() == Some("--check")),
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
