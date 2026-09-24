use ainc_client::*;
use anyhow::Context as _;
pub struct Cli<T: CliConfig> {
    client: Client,
    config: T,
}
impl<T: CliConfig> Cli<T> {
    pub fn new(client: Client, config: T) -> Self {
        Self { client, config }
    }
    pub fn get_command(cmd: CliCommand) -> ::clap::Command {
        match cmd {
            CliCommand::HealthLive => Self::cli_health_live(),
            CliCommand::HealthReady => Self::cli_health_ready(),
            CliCommand::TicketContract => Self::cli_ticket_contract(),
            CliCommand::GetVersion => Self::cli_get_version(),
        }
    }
    pub fn cli_health_live() -> ::clap::Command {
        ::clap::Command::new("")
    }
    pub fn cli_health_ready() -> ::clap::Command {
        ::clap::Command::new("")
    }
    pub fn cli_ticket_contract() -> ::clap::Command {
        ::clap::Command::new("")
            .arg(
                ::clap::Arg::new("title")
                    .long("title")
                    .value_parser(::clap::value_parser!(::std::string::String))
                    .required_unless_present("json-body"),
            )
            .arg(
                ::clap::Arg::new("json-body")
                    .long("json-body")
                    .value_name("JSON-FILE")
                    .required(false)
                    .value_parser(::clap::value_parser!(std::path::PathBuf))
                    .help("Path to a file that contains the full json body."),
            )
            .arg(
                ::clap::Arg::new("json-body-template")
                    .long("json-body-template")
                    .action(::clap::ArgAction::SetTrue)
                    .help("XXX"),
            )
    }
    pub fn cli_get_version() -> ::clap::Command {
        ::clap::Command::new("")
    }
    pub async fn execute(
        &self,
        cmd: CliCommand,
        matches: &::clap::ArgMatches,
    ) -> anyhow::Result<()> {
        match cmd {
            CliCommand::HealthLive => self.execute_health_live(matches).await,
            CliCommand::HealthReady => self.execute_health_ready(matches).await,
            CliCommand::TicketContract => self.execute_ticket_contract(matches).await,
            CliCommand::GetVersion => self.execute_get_version(matches).await,
        }
    }
    pub async fn execute_health_live(&self, matches: &::clap::ArgMatches) -> anyhow::Result<()> {
        let mut request = self.client.health_live();
        self.config.execute_health_live(matches, &mut request)?;
        let result = request.send().await;
        match result {
            Ok(r) => {
                self.config.success_item(&r);
                Ok(())
            }
            Err(r) => {
                self.config.error(&r);
                Err(anyhow::Error::new(r))
            }
        }
    }
    pub async fn execute_health_ready(&self, matches: &::clap::ArgMatches) -> anyhow::Result<()> {
        let mut request = self.client.health_ready();
        self.config.execute_health_ready(matches, &mut request)?;
        let result = request.send().await;
        match result {
            Ok(r) => {
                self.config.success_item(&r);
                Ok(())
            }
            Err(r) => {
                self.config.error(&r);
                Err(anyhow::Error::new(r))
            }
        }
    }
    pub async fn execute_ticket_contract(
        &self,
        matches: &::clap::ArgMatches,
    ) -> anyhow::Result<()> {
        let mut request = self.client.ticket_contract();
        if let Some(value) = matches.get_one::<::std::string::String>("title") {
            request = request.body_map(|body| body.title(value.clone()));
        }
        if let Some(value) = matches.get_one::<std::path::PathBuf>("json-body") {
            let body_txt = std::fs::read_to_string(value)
                .with_context(|| format!("failed to read {}", value.display()))?;
            let body_value = serde_json::from_str::<types::TicketContract>(&body_txt)
                .with_context(|| format!("failed to parse {}", value.display()))?;
            request = request.body(body_value);
        }
        self.config.execute_ticket_contract(matches, &mut request)?;
        let result = request.send().await;
        match result {
            Ok(r) => {
                self.config.success_item(&r);
                Ok(())
            }
            Err(r) => {
                self.config.error(&r);
                Err(anyhow::Error::new(r))
            }
        }
    }
    pub async fn execute_get_version(&self, matches: &::clap::ArgMatches) -> anyhow::Result<()> {
        let mut request = self.client.get_version();
        self.config.execute_get_version(matches, &mut request)?;
        let result = request.send().await;
        match result {
            Ok(r) => {
                self.config.success_item(&r);
                Ok(())
            }
            Err(r) => {
                self.config.error(&r);
                Err(anyhow::Error::new(r))
            }
        }
    }
}
pub trait CliConfig {
    fn success_item<T>(&self, value: &ResponseValue<T>)
    where
        T: schemars::JsonSchema + serde::Serialize + std::fmt::Debug;
    fn success_no_item(&self, value: &ResponseValue<()>);
    fn error<T>(&self, value: &Error<T>)
    where
        T: schemars::JsonSchema + serde::Serialize + std::fmt::Debug;
    fn list_start<T>(&self)
    where
        T: schemars::JsonSchema + serde::Serialize + std::fmt::Debug;
    fn list_item<T>(&self, value: &T)
    where
        T: schemars::JsonSchema + serde::Serialize + std::fmt::Debug;
    fn list_end_success<T>(&self)
    where
        T: schemars::JsonSchema + serde::Serialize + std::fmt::Debug;
    fn list_end_error<T>(&self, value: &Error<T>)
    where
        T: schemars::JsonSchema + serde::Serialize + std::fmt::Debug;
    fn execute_health_live(
        &self,
        matches: &::clap::ArgMatches,
        request: &mut builder::HealthLive,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    fn execute_health_ready(
        &self,
        matches: &::clap::ArgMatches,
        request: &mut builder::HealthReady,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    fn execute_ticket_contract(
        &self,
        matches: &::clap::ArgMatches,
        request: &mut builder::TicketContract,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    fn execute_get_version(
        &self,
        matches: &::clap::ArgMatches,
        request: &mut builder::GetVersion,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}
#[derive(Copy, Clone, Debug)]
pub enum CliCommand {
    HealthLive,
    HealthReady,
    TicketContract,
    GetVersion,
}
impl CliCommand {
    pub fn iter() -> impl Iterator<Item = CliCommand> {
        vec![
            CliCommand::HealthLive,
            CliCommand::HealthReady,
            CliCommand::TicketContract,
            CliCommand::GetVersion,
        ]
        .into_iter()
    }
    pub fn operation_id(&self) -> &'static str {
        match self {
            CliCommand::HealthLive => "health_live",
            CliCommand::HealthReady => "health_ready",
            CliCommand::TicketContract => "ticket_contract",
            CliCommand::GetVersion => "get_version",
        }
    }
}
