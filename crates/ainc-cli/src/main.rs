#[allow(unused_variables, dead_code)]
mod generated;

use ainc_client::{Client, Error, ResponseValue};
use anyhow::{Context, Result};
use clap::Command;
use generated::{Cli, CliCommand, CliConfig};
use serde::Serialize;
use std::{env, fs, path::PathBuf};

struct Output;

impl CliConfig for Output {
    fn success_item<T>(&self, value: &ResponseValue<T>)
    where
        T: schemars::JsonSchema + Serialize + std::fmt::Debug,
    {
        println!(
            "{}",
            serde_json::to_string(value.as_ref()).expect("generated response serializes")
        );
    }
    fn success_no_item(&self, _: &ResponseValue<()>) {}
    fn error<T>(&self, value: &Error<T>)
    where
        T: schemars::JsonSchema + Serialize + std::fmt::Debug,
    {
        eprintln!("{value}");
    }
    fn list_start<T>(&self)
    where
        T: schemars::JsonSchema + Serialize + std::fmt::Debug,
    {
    }
    fn list_item<T>(&self, value: &T)
    where
        T: schemars::JsonSchema + Serialize + std::fmt::Debug,
    {
        println!(
            "{}",
            serde_json::to_string(value).expect("generated response serializes")
        );
    }
    fn list_end_success<T>(&self)
    where
        T: schemars::JsonSchema + Serialize + std::fmt::Debug,
    {
    }
    fn list_end_error<T>(&self, value: &Error<T>)
    where
        T: schemars::JsonSchema + Serialize + std::fmt::Debug,
    {
        eprintln!("{value}");
    }
}

fn api_url() -> Result<String> {
    if let Ok(url) = env::var("AINC_API_URL") {
        return Ok(url);
    }
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.local/dev/api-url");
    Ok(fs::read_to_string(path)
        .context("API discovery missing; run cargo xtask dev or set AINC_API_URL")?
        .trim()
        .into())
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut command = Command::new("ainc").about("AgentInc development CLI");
    for operation in CliCommand::iter() {
        command = command
            .subcommand(Cli::<Output>::get_command(operation).name(operation.operation_id()));
    }
    let matches = command.get_matches();
    let (name, args) = matches.subcommand().context("choose a command")?;
    let operation = CliCommand::iter()
        .find(|operation| operation.operation_id() == name)
        .context("unknown generated command")?;
    Cli::new(Client::new(&api_url()?), Output)
        .execute(operation, args)
        .await
}
