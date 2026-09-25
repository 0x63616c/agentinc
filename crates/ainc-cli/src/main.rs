#[allow(unused_variables, dead_code)]
mod generated;

use ainc_client::{Client, Error, ResponseValue};
use anyhow::{Context, Result, bail, ensure};
use clap::{Arg, ArgAction, ArgMatches, Command};
use generated::{Cli, CliCommand, CliConfig};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    env, fs,
    path::{Path, PathBuf},
    time::Duration,
};

struct Output {
    json: bool,
}

fn display(value: &Value, json_output: bool) {
    if json_output {
        println!("{}", serde_json::to_string_pretty(value).unwrap());
    } else {
        match value {
            Value::Array(items) => {
                for item in items {
                    display(item, false);
                }
            }
            Value::Object(map) => {
                for (key, value) in map {
                    if let Some(items) = value.as_array() {
                        println!("{key} ({}):", items.len());
                        for item in items {
                            println!("  {}", compact(item));
                        }
                    } else {
                        println!("{key}: {}", compact(value));
                    }
                }
            }
            _ => println!("{}", compact(value)),
        }
    }
}
fn compact(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Object(fields) => fields
            .iter()
            .map(|(key, value)| format!("{key}={}", compact(value)))
            .collect::<Vec<_>>()
            .join("  "),
        _ => value.to_string(),
    }
}
impl CliConfig for Output {
    fn success_item<T: schemars::JsonSchema + Serialize + std::fmt::Debug>(
        &self,
        value: &ResponseValue<T>,
    ) {
        display(&serde_json::to_value(value.as_ref()).unwrap(), self.json);
    }
    fn success_no_item(&self, _: &ResponseValue<()>) {
        if !self.json {
            println!("OK");
        }
    }
    fn error<T: schemars::JsonSchema + Serialize + std::fmt::Debug>(&self, value: &Error<T>) {
        eprintln!("{value}");
    }
    fn list_start<T: schemars::JsonSchema + Serialize + std::fmt::Debug>(&self) {}
    fn list_item<T: schemars::JsonSchema + Serialize + std::fmt::Debug>(&self, value: &T) {
        display(&serde_json::to_value(value).unwrap(), self.json);
    }
    fn list_end_success<T: schemars::JsonSchema + Serialize + std::fmt::Debug>(&self) {}
    fn list_end_error<T: schemars::JsonSchema + Serialize + std::fmt::Debug>(
        &self,
        value: &Error<T>,
    ) {
        eprintln!("{value}");
    }
}

fn dev_discovery() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.local/dev/api-url")
}
fn select_discovery(override_path: Option<PathBuf>, installed: PathBuf, dev: PathBuf) -> PathBuf {
    override_path.unwrap_or_else(|| if installed.exists() { installed } else { dev })
}
fn discovery_path() -> PathBuf {
    select_discovery(
        env::var_os("AINC_DISCOVERY_FILE").map(PathBuf::from),
        ainc_release::identity::support_dir().join("daemon/api-url"),
        dev_discovery(),
    )
}
fn configuration() -> Result<(String, PathBuf)> {
    let discovery = discovery_path();
    resolve_configuration(
        env::var("AINC_API_URL").ok(),
        env::var_os("AINC_TOKEN_FILE").map(PathBuf::from),
        &discovery,
    )
}
fn resolve_configuration(
    url_override: Option<String>,
    token_override: Option<PathBuf>,
    discovery: &Path,
) -> Result<(String, PathBuf)> {
    let url = match url_override {
        Some(url) => url,
        None => fs::read_to_string(discovery).with_context(|| format!("No running AgentInc daemon found ({}). Open AgentInc, start cargo xtask dev, or set AINC_API_URL.", discovery.display()))?,
    };
    let token = token_override.unwrap_or_else(|| discovery.with_file_name("owner-token"));
    Ok((url.trim().trim_end_matches('/').to_owned(), token))
}

fn operation_group(id: &str) -> (&str, &str) {
    match id {
        "health_live" => ("health", "live"),
        "health_ready" => ("health", "ready"),
        "get_version" => ("version", "show"),
        "ticket_contract" => ("tickets", "contract"),
        _ => {
            let (group, action) = id.split_once('_').unwrap_or(("api", id));
            (
                group,
                match action {
                    "state" => "list",
                    other => other,
                },
            )
        }
    }
}
fn schema() -> Value {
    serde_json::from_str(include_str!("../../../api/openapi-3.0.json")).expect("checked-in OpenAPI")
}
type Field = (String, String, bool);
type Variant = (String, Vec<Field>);

fn variants(spec: &Value, group: &str) -> Vec<Variant> {
    let request = match group {
        "tickets" => "TicketCommand",
        "automations" => "AutomationCommand",
        "product" => "Command",
        _ => return vec![],
    };
    let variants = &spec["components"]["schemas"][request]["oneOf"];
    variants
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|variant| {
            let kind = variant["properties"]["kind"]["enum"][0].as_str()?;
            let required = variant["required"].as_array()?;
            let fields = variant["properties"]
                .as_object()?
                .iter()
                .filter(|(name, _)| name.as_str() != "kind")
                .map(|(name, field)| {
                    let typ = if field.get("$ref").is_some()
                        || field["type"]
                            .as_str()
                            .is_some_and(|t| t == "object" || t == "array")
                    {
                        "json"
                    } else {
                        field["type"].as_str().unwrap_or("string")
                    };
                    (
                        name.clone(),
                        typ.to_owned(),
                        required.iter().any(|v| v.as_str() == Some(name)),
                    )
                })
                .collect();
            Some((kind.replace('_', "-"), fields))
        })
        .collect()
}
fn command_tree(spec: &Value) -> Command {
    let mut root = Command::new("ainc")
        .about("AgentInc command line")
        .subcommand_required(true)
        .arg(
            Arg::new("json")
                .long("json")
                .global(true)
                .action(ArgAction::SetTrue)
                .help("Print machine-readable JSON"),
        )
        .subcommand(Command::new("status").about("Check the running daemon"))
        .subcommand(Command::new("version").about("Show the daemon version"))
        .subcommand(Command::new("install-cli").about("Link ainc into ~/.local/bin"))
        .subcommand(
            Command::new("completions")
                .about("Generate shell completions")
                .arg(Arg::new("shell").required(true).value_parser([
                    "bash",
                    "zsh",
                    "fish",
                    "elvish",
                    "powershell",
                ])),
        );
    let mut groups: std::collections::BTreeMap<&str, Command> = std::collections::BTreeMap::new();
    for operation in CliCommand::iter() {
        let (group, action) = operation_group(operation.operation_id());
        if group == "version" {
            continue;
        }
        let command = Cli::<Output>::get_command(operation).name(action);
        let entry = groups
            .entry(group)
            .or_insert_with(|| Command::new(group).subcommand_required(true));
        *entry = entry.clone().subcommand(command);
    }
    for group in ["tickets", "automations", "product"] {
        if let Some(entry) = groups.get_mut(group) {
            for (kind, fields) in variants(spec, group) {
                let mut command =
                    Command::new(Box::leak(kind.clone().into_boxed_str()) as &'static str)
                        .about(format!("{} {}", group, kind));
                for (name, typ, required) in fields {
                    let flag = name.replace('_', "-");
                    let name: &'static str = Box::leak(name.into_boxed_str());
                    let flag: &'static str = Box::leak(flag.into_boxed_str());
                    command = command.arg(
                        Arg::new(name)
                            .long(flag)
                            .required_unless_present(if required { "json-body" } else { name })
                            .help(format!("{} field", typ)),
                    );
                }
                *entry = entry.clone().subcommand(
                    command.arg(
                        Arg::new("json-body")
                            .long("json-body")
                            .value_name("JSON-FILE")
                            .conflicts_with_all(fields_for_kind(spec, group, &kind)),
                    ),
                );
            }
        }
    }
    for (_, group) in groups {
        root = root.subcommand(group);
    }
    root
}
fn fields_for_kind(spec: &Value, group: &str, kind: &str) -> Vec<&'static str> {
    variants(spec, group)
        .into_iter()
        .find(|(name, _)| name == kind)
        .map(|(_, fields)| {
            fields
                .into_iter()
                .map(|(name, _, _)| Box::leak(name.into_boxed_str()) as &'static str)
                .collect()
        })
        .unwrap_or_default()
}
fn operation(id: &str) -> CliCommand {
    CliCommand::iter()
        .find(|v| v.operation_id() == id)
        .expect("operation in generated CLI")
}
fn generated_matches(op: CliCommand, body: Option<&Path>) -> Result<ArgMatches> {
    let mut args = vec![op.operation_id().to_owned()];
    if let Some(body) = body {
        args.extend(["--json-body".into(), body.to_string_lossy().into_owned()]);
    }
    Ok(Cli::<Output>::get_command(op)
        .name(op.operation_id())
        .try_get_matches_from(args)?)
}
async fn execute(client: &Cli<Output>, op: CliCommand, args: &ArgMatches) -> Result<()> {
    client.execute(op, args).await
}

fn install_cli() -> Result<()> {
    let source = env::current_exe()?;
    let destination =
        PathBuf::from(env::var_os("HOME").context("HOME is missing")?).join(".local/bin/ainc");
    fs::create_dir_all(destination.parent().unwrap())?;
    if destination.exists() || destination.is_symlink() {
        ensure!(
            destination.is_symlink() && fs::read_link(&destination)? == source,
            "{} already exists; remove it before installing",
            destination.display()
        );
    } else {
        std::os::unix::fs::symlink(source, &destination)?;
    }
    println!(
        "Installed {} (add ~/.local/bin to PATH if needed)",
        destination.display()
    );
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let spec = schema();
    let mut tree = command_tree(&spec);
    let matches = tree.clone().get_matches();
    let json_output = matches.get_flag("json");
    let (group, args) = matches.subcommand().context("choose a command")?;
    if group == "completions" {
        let shell = args.get_one::<String>("shell").unwrap();
        let shell = shell
            .parse::<clap_complete::Shell>()
            .map_err(anyhow::Error::msg)?;
        clap_complete::generate(shell, &mut tree, "ainc", &mut std::io::stdout());
        return Ok(());
    }
    if group == "install-cli" {
        return install_cli();
    }
    let (url, token_path) = configuration()?;
    let token = fs::read_to_string(&token_path).with_context(|| {
        format!(
            "Daemon owner credential unavailable: {}",
            token_path.display()
        )
    })?;
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::AUTHORIZATION,
        format!("Bearer {}", token.trim()).parse()?,
    );
    let http = reqwest::Client::builder()
        .default_headers(headers)
        .timeout(Duration::from_secs(40))
        .build()?;
    http.get(format!("{url}/health/ready"))
        .timeout(Duration::from_secs(2))
        .send()
        .await
        .with_context(|| {
            format!("No running AgentInc daemon at {url}. Open AgentInc or start cargo xtask dev.")
        })?
        .error_for_status()
        .context("AgentInc daemon is not ready")?;
    let client = Cli::new(
        Client::new_with_client(&url, http),
        Output { json: json_output },
    );
    if group == "status" {
        return execute(
            &client,
            operation("health_ready"),
            &generated_matches(operation("health_ready"), None)?,
        )
        .await;
    }
    if group == "version" {
        return execute(
            &client,
            operation("get_version"),
            &generated_matches(operation("get_version"), None)?,
        )
        .await;
    }
    let (action, action_args) = args.subcommand().context("choose a resource command")?;
    if let Some(op) =
        CliCommand::iter().find(|op| operation_group(op.operation_id()) == (group, action))
    {
        return execute(&client, op, action_args).await;
    }
    let kind = action.replace('-', "_");
    let fields = variants(&spec, group)
        .into_iter()
        .find(|(name, _)| name == action)
        .map(|(_, fields)| fields)
        .context("unknown command")?;
    let op = operation(match group {
        "tickets" => "tickets_command",
        "automations" => "automations_command",
        "product" => "product_command",
        _ => bail!("unsupported command"),
    });
    let body = if let Some(path) = action_args.get_one::<String>("json-body") {
        fs::read_to_string(path).with_context(|| format!("failed to read {path}"))?
    } else {
        let mut command = serde_json::Map::new();
        command.insert("kind".into(), json!(kind));
        for (name, typ, _) in fields {
            if let Some(value) = action_args.get_one::<String>(&name) {
                let value = match typ.as_str() {
                    "integer" => json!(value.parse::<i64>()?),
                    "boolean" => json!(value.parse::<bool>()?),
                    "json" => serde_json::from_str(value)?,
                    _ => json!(value),
                };
                command.insert(name, value);
            }
        }
        json!({"operation_id": uuid::Uuid::new_v4().to_string(), "command": command}).to_string()
    };
    let mut file = tempfile::NamedTempFile::new()?;
    std::io::Write::write_all(&mut file, body.as_bytes())?;
    execute(&client, op, &generated_matches(op, Some(file.path()))?).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovery_precedence() {
        let dir = tempfile::tempdir().unwrap();
        let installed = dir.path().join("installed");
        let dev = dir.path().join("dev");
        assert_eq!(select_discovery(None, installed.clone(), dev.clone()), dev);
        fs::write(&installed, "url").unwrap();
        assert_eq!(select_discovery(None, installed.clone(), dev), installed);
        let override_path = dir.path().join("override");
        assert_eq!(
            select_discovery(Some(override_path.clone()), installed, PathBuf::new()),
            override_path
        );
        fs::write(&override_path, "http://localhost:1234/\n").unwrap();
        assert_eq!(
            resolve_configuration(None, None, &override_path).unwrap(),
            (
                "http://localhost:1234".into(),
                dir.path().join("owner-token")
            )
        );
        let token = dir.path().join("custom-token");
        assert_eq!(
            resolve_configuration(
                Some("http://localhost:9999".into()),
                Some(token.clone()),
                &override_path
            )
            .unwrap(),
            ("http://localhost:9999".into(), token)
        );
    }
    #[test]
    fn every_spec_operation_is_reachable() {
        let spec = schema();
        let tree = command_tree(&spec);
        for path in spec["paths"].as_object().unwrap().values() {
            for endpoint in path.as_object().unwrap().values() {
                let id = endpoint["operationId"].as_str().unwrap();
                let (group, action) = operation_group(id);
                assert!(CliCommand::iter().any(|op| op.operation_id() == id), "{id}");
                assert!(
                    tree.find_subcommand(group)
                        .and_then(|g| g.find_subcommand(action))
                        .is_some()
                        || id == "get_version",
                    "{id}"
                );
            }
        }
    }
}
