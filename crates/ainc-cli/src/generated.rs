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
            CliCommand::CalendarState => Self::cli_calendar_state(),
            CliCommand::CalendarCommand => Self::cli_calendar_command(),
            CliCommand::TodosState => Self::cli_todos_state(),
            CliCommand::TodosCommand => Self::cli_todos_command(),
            CliCommand::WorkspacesState => Self::cli_workspaces_state(),
            CliCommand::WorkspacesCommand => Self::cli_workspaces_command(),
            CliCommand::VersionShow => Self::cli_version_show(),
        }
    }
    pub fn cli_health_live() -> ::clap::Command {
        ::clap::Command::new("")
    }
    pub fn cli_health_ready() -> ::clap::Command {
        ::clap::Command::new("")
    }
    pub fn cli_calendar_state() -> ::clap::Command {
        ::clap::Command::new("")
    }
    pub fn cli_calendar_command() -> ::clap::Command {
        ::clap::Command::new("")
            .arg(
                ::clap::Arg::new("operation-id")
                    .long("operation-id")
                    .value_parser(::clap::value_parser!(::std::string::String))
                    .required_unless_present("json-body"),
            )
            .arg(
                ::clap::Arg::new("json-body")
                    .long("json-body")
                    .value_name("JSON-FILE")
                    .required(true)
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
    pub fn cli_todos_state() -> ::clap::Command {
        ::clap::Command::new("")
    }
    pub fn cli_todos_command() -> ::clap::Command {
        ::clap::Command::new("")
            .arg(
                ::clap::Arg::new("operation-id")
                    .long("operation-id")
                    .value_parser(::clap::value_parser!(::std::string::String))
                    .required_unless_present("json-body"),
            )
            .arg(
                ::clap::Arg::new("json-body")
                    .long("json-body")
                    .value_name("JSON-FILE")
                    .required(true)
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
    pub fn cli_workspaces_state() -> ::clap::Command {
        ::clap::Command::new("")
    }
    pub fn cli_workspaces_command() -> ::clap::Command {
        ::clap::Command::new("")
            .arg(
                ::clap::Arg::new("operation-id")
                    .long("operation-id")
                    .value_parser(::clap::value_parser!(::std::string::String))
                    .required_unless_present("json-body"),
            )
            .arg(
                ::clap::Arg::new("json-body")
                    .long("json-body")
                    .value_name("JSON-FILE")
                    .required(true)
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
    pub fn cli_version_show() -> ::clap::Command {
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
            CliCommand::CalendarState => self.execute_calendar_state(matches).await,
            CliCommand::CalendarCommand => self.execute_calendar_command(matches).await,
            CliCommand::TodosState => self.execute_todos_state(matches).await,
            CliCommand::TodosCommand => self.execute_todos_command(matches).await,
            CliCommand::WorkspacesState => self.execute_workspaces_state(matches).await,
            CliCommand::WorkspacesCommand => self.execute_workspaces_command(matches).await,
            CliCommand::VersionShow => self.execute_version_show(matches).await,
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
    pub async fn execute_calendar_state(&self, matches: &::clap::ArgMatches) -> anyhow::Result<()> {
        let mut request = self.client.calendar_state();
        self.config.execute_calendar_state(matches, &mut request)?;
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
    pub async fn execute_calendar_command(
        &self,
        matches: &::clap::ArgMatches,
    ) -> anyhow::Result<()> {
        let mut request = self.client.calendar_command();
        if let Some(value) = matches.get_one::<::std::string::String>("operation-id") {
            request = request.body_map(|body| body.operation_id(value.clone()));
        }
        if let Some(value) = matches.get_one::<std::path::PathBuf>("json-body") {
            let body_txt = std::fs::read_to_string(value)
                .with_context(|| format!("failed to read {}", value.display()))?;
            let body_value = serde_json::from_str::<types::CalendarRequest>(&body_txt)
                .with_context(|| format!("failed to parse {}", value.display()))?;
            request = request.body(body_value);
        }
        self.config
            .execute_calendar_command(matches, &mut request)?;
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
    pub async fn execute_todos_state(&self, matches: &::clap::ArgMatches) -> anyhow::Result<()> {
        let mut request = self.client.todos_state();
        self.config.execute_todos_state(matches, &mut request)?;
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
    pub async fn execute_todos_command(&self, matches: &::clap::ArgMatches) -> anyhow::Result<()> {
        let mut request = self.client.todos_command();
        if let Some(value) = matches.get_one::<::std::string::String>("operation-id") {
            request = request.body_map(|body| body.operation_id(value.clone()));
        }
        if let Some(value) = matches.get_one::<std::path::PathBuf>("json-body") {
            let body_txt = std::fs::read_to_string(value)
                .with_context(|| format!("failed to read {}", value.display()))?;
            let body_value = serde_json::from_str::<types::TodoRequest>(&body_txt)
                .with_context(|| format!("failed to parse {}", value.display()))?;
            request = request.body(body_value);
        }
        self.config.execute_todos_command(matches, &mut request)?;
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
    pub async fn execute_workspaces_state(
        &self,
        matches: &::clap::ArgMatches,
    ) -> anyhow::Result<()> {
        let mut request = self.client.workspaces_state();
        self.config
            .execute_workspaces_state(matches, &mut request)?;
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
    pub async fn execute_workspaces_command(
        &self,
        matches: &::clap::ArgMatches,
    ) -> anyhow::Result<()> {
        let mut request = self.client.workspaces_command();
        if let Some(value) = matches.get_one::<::std::string::String>("operation-id") {
            request = request.body_map(|body| body.operation_id(value.clone()));
        }
        if let Some(value) = matches.get_one::<std::path::PathBuf>("json-body") {
            let body_txt = std::fs::read_to_string(value)
                .with_context(|| format!("failed to read {}", value.display()))?;
            let body_value = serde_json::from_str::<types::WorkspaceRequest>(&body_txt)
                .with_context(|| format!("failed to parse {}", value.display()))?;
            request = request.body(body_value);
        }
        self.config
            .execute_workspaces_command(matches, &mut request)?;
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
    pub async fn execute_version_show(&self, matches: &::clap::ArgMatches) -> anyhow::Result<()> {
        let mut request = self.client.version_show();
        self.config.execute_version_show(matches, &mut request)?;
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
    fn execute_calendar_state(
        &self,
        matches: &::clap::ArgMatches,
        request: &mut builder::CalendarState,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    fn execute_calendar_command(
        &self,
        matches: &::clap::ArgMatches,
        request: &mut builder::CalendarCommand,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    fn execute_todos_state(
        &self,
        matches: &::clap::ArgMatches,
        request: &mut builder::TodosState,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    fn execute_todos_command(
        &self,
        matches: &::clap::ArgMatches,
        request: &mut builder::TodosCommand,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    fn execute_workspaces_state(
        &self,
        matches: &::clap::ArgMatches,
        request: &mut builder::WorkspacesState,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    fn execute_workspaces_command(
        &self,
        matches: &::clap::ArgMatches,
        request: &mut builder::WorkspacesCommand,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    fn execute_version_show(
        &self,
        matches: &::clap::ArgMatches,
        request: &mut builder::VersionShow,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}
#[derive(Copy, Clone, Debug)]
pub enum CliCommand {
    HealthLive,
    HealthReady,
    CalendarState,
    CalendarCommand,
    TodosState,
    TodosCommand,
    WorkspacesState,
    WorkspacesCommand,
    VersionShow,
}
impl CliCommand {
    pub fn iter() -> impl Iterator<Item = CliCommand> {
        vec![
            CliCommand::HealthLive,
            CliCommand::HealthReady,
            CliCommand::CalendarState,
            CliCommand::CalendarCommand,
            CliCommand::TodosState,
            CliCommand::TodosCommand,
            CliCommand::WorkspacesState,
            CliCommand::WorkspacesCommand,
            CliCommand::VersionShow,
        ]
        .into_iter()
    }
    pub fn operation_id(&self) -> &'static str {
        match self {
            CliCommand::HealthLive => "health_live",
            CliCommand::HealthReady => "health_ready",
            CliCommand::CalendarState => "calendar_state",
            CliCommand::CalendarCommand => "calendar_command",
            CliCommand::TodosState => "todos_state",
            CliCommand::TodosCommand => "todos_command",
            CliCommand::WorkspacesState => "workspaces_state",
            CliCommand::WorkspacesCommand => "workspaces_command",
            CliCommand::VersionShow => "version_show",
        }
    }
}
