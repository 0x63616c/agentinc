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
            CliCommand::AutomationsState => Self::cli_automations_state(),
            CliCommand::AutomationsCommand => Self::cli_automations_command(),
            CliCommand::CalendarState => Self::cli_calendar_state(),
            CliCommand::CalendarCommand => Self::cli_calendar_command(),
            CliCommand::CalendarImport => Self::cli_calendar_import(),
            CliCommand::ProductCommand => Self::cli_product_command(),
            CliCommand::ConnectionStatus => Self::cli_connection_status(),
            CliCommand::ConnectionCancel => Self::cli_connection_cancel(),
            CliCommand::ConnectionLogin => Self::cli_connection_login(),
            CliCommand::ConnectionLogout => Self::cli_connection_logout(),
            CliCommand::HomeState => Self::cli_home_state(),
            CliCommand::HomeCommand => Self::cli_home_command(),
            CliCommand::HomeConnect => Self::cli_home_connect(),
            CliCommand::HomeDisconnect => Self::cli_home_disconnect(),
            CliCommand::ProductState => Self::cli_product_state(),
            CliCommand::TemporalExecutions => Self::cli_temporal_executions(),
            CliCommand::TerminalSessionsList => Self::cli_terminal_sessions_list(),
            CliCommand::TerminalSessionsCreate => Self::cli_terminal_sessions_create(),
            CliCommand::TerminalSessionsClose => Self::cli_terminal_sessions_close(),
            CliCommand::TicketsState => Self::cli_tickets_state(),
            CliCommand::TicketsCommand => Self::cli_tickets_command(),
            CliCommand::TicketContract => Self::cli_ticket_contract(),
            CliCommand::WorkspacesState => Self::cli_workspaces_state(),
            CliCommand::WorkspacesCommand => Self::cli_workspaces_command(),
            CliCommand::GetVersion => Self::cli_get_version(),
        }
    }
    pub fn cli_health_live() -> ::clap::Command {
        ::clap::Command::new("")
    }
    pub fn cli_health_ready() -> ::clap::Command {
        ::clap::Command::new("")
    }
    pub fn cli_automations_state() -> ::clap::Command {
        ::clap::Command::new("")
    }
    pub fn cli_automations_command() -> ::clap::Command {
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
    pub fn cli_calendar_state() -> ::clap::Command {
        ::clap::Command::new("")
            .arg(
                ::clap::Arg::new("from")
                    .long("from")
                    .value_parser(::clap::value_parser!(i64))
                    .required(false)
                    .help("Unix seconds; defaults to 31 days ago."),
            )
            .arg(
                ::clap::Arg::new("to")
                    .long("to")
                    .value_parser(::clap::value_parser!(i64))
                    .required(false)
                    .help("Unix seconds, exclusive; defaults to 180 days ahead."),
            )
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
    pub fn cli_calendar_import() -> ::clap::Command {
        ::clap::Command::new("")
            .arg(
                ::clap::Arg::new("window-end")
                    .long("window-end")
                    .value_parser(::clap::value_parser!(i64))
                    .required_unless_present("json-body"),
            )
            .arg(
                ::clap::Arg::new("window-start")
                    .long("window-start")
                    .value_parser(::clap::value_parser!(i64))
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
    pub fn cli_product_command() -> ::clap::Command {
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
    pub fn cli_connection_status() -> ::clap::Command {
        ::clap::Command::new("")
    }
    pub fn cli_connection_cancel() -> ::clap::Command {
        ::clap::Command::new("")
    }
    pub fn cli_connection_login() -> ::clap::Command {
        ::clap::Command::new("")
    }
    pub fn cli_connection_logout() -> ::clap::Command {
        ::clap::Command::new("")
    }
    pub fn cli_home_state() -> ::clap::Command {
        ::clap::Command::new("")
    }
    pub fn cli_home_command() -> ::clap::Command {
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
    pub fn cli_home_connect() -> ::clap::Command {
        ::clap::Command::new("")
            .arg(
                ::clap::Arg::new("access-client-id")
                    .long("access-client-id")
                    .value_parser(::clap::value_parser!(::std::string::String))
                    .required(false)
                    .help(
                        "Cloudflare Access service token; omit both for an endpoint without Access.",
                    ),
            )
            .arg(
                ::clap::Arg::new("access-client-secret")
                    .long("access-client-secret")
                    .value_parser(::clap::value_parser!(::std::string::String))
                    .required(false),
            )
            .arg(
                ::clap::Arg::new("base-url")
                    .long("base-url")
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
    pub fn cli_home_disconnect() -> ::clap::Command {
        ::clap::Command::new("")
    }
    pub fn cli_product_state() -> ::clap::Command {
        ::clap::Command::new("")
    }
    pub fn cli_temporal_executions() -> ::clap::Command {
        ::clap::Command::new("")
            .arg(
                ::clap::Arg::new("page")
                    .long("page")
                    .value_parser(::clap::value_parser!(::std::string::String))
                    .required(false)
                    .help("Opaque next-page token"),
            )
            .arg(
                ::clap::Arg::new("status")
                    .long("status")
                    .value_parser(::clap::value_parser!(::std::string::String))
                    .required(false)
                    .help(
                        "All, Running, Completed, Failed, Canceled, Terminated, TimedOut, ContinuedAsNew, or Paused",
                    ),
            )
    }
    pub fn cli_terminal_sessions_list() -> ::clap::Command {
        ::clap::Command::new("")
    }
    pub fn cli_terminal_sessions_create() -> ::clap::Command {
        ::clap::Command::new("")
            .arg(
                ::clap::Arg::new("id")
                    .long("id")
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
    pub fn cli_terminal_sessions_close() -> ::clap::Command {
        ::clap::Command::new("").arg(
            ::clap::Arg::new("id")
                .long("id")
                .value_parser(::clap::value_parser!(::std::string::String))
                .required(true),
        )
    }
    pub fn cli_tickets_state() -> ::clap::Command {
        ::clap::Command::new("")
    }
    pub fn cli_tickets_command() -> ::clap::Command {
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
            CliCommand::AutomationsState => self.execute_automations_state(matches).await,
            CliCommand::AutomationsCommand => self.execute_automations_command(matches).await,
            CliCommand::CalendarState => self.execute_calendar_state(matches).await,
            CliCommand::CalendarCommand => self.execute_calendar_command(matches).await,
            CliCommand::CalendarImport => self.execute_calendar_import(matches).await,
            CliCommand::ProductCommand => self.execute_product_command(matches).await,
            CliCommand::ConnectionStatus => self.execute_connection_status(matches).await,
            CliCommand::ConnectionCancel => self.execute_connection_cancel(matches).await,
            CliCommand::ConnectionLogin => self.execute_connection_login(matches).await,
            CliCommand::ConnectionLogout => self.execute_connection_logout(matches).await,
            CliCommand::HomeState => self.execute_home_state(matches).await,
            CliCommand::HomeCommand => self.execute_home_command(matches).await,
            CliCommand::HomeConnect => self.execute_home_connect(matches).await,
            CliCommand::HomeDisconnect => self.execute_home_disconnect(matches).await,
            CliCommand::ProductState => self.execute_product_state(matches).await,
            CliCommand::TemporalExecutions => self.execute_temporal_executions(matches).await,
            CliCommand::TerminalSessionsList => self.execute_terminal_sessions_list(matches).await,
            CliCommand::TerminalSessionsCreate => {
                self.execute_terminal_sessions_create(matches).await
            }
            CliCommand::TerminalSessionsClose => {
                self.execute_terminal_sessions_close(matches).await
            }
            CliCommand::TicketsState => self.execute_tickets_state(matches).await,
            CliCommand::TicketsCommand => self.execute_tickets_command(matches).await,
            CliCommand::TicketContract => self.execute_ticket_contract(matches).await,
            CliCommand::WorkspacesState => self.execute_workspaces_state(matches).await,
            CliCommand::WorkspacesCommand => self.execute_workspaces_command(matches).await,
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
    pub async fn execute_automations_state(
        &self,
        matches: &::clap::ArgMatches,
    ) -> anyhow::Result<()> {
        let mut request = self.client.automations_state();
        self.config
            .execute_automations_state(matches, &mut request)?;
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
    pub async fn execute_automations_command(
        &self,
        matches: &::clap::ArgMatches,
    ) -> anyhow::Result<()> {
        let mut request = self.client.automations_command();
        if let Some(value) = matches.get_one::<::std::string::String>("operation-id") {
            request = request.body_map(|body| body.operation_id(value.clone()));
        }
        if let Some(value) = matches.get_one::<std::path::PathBuf>("json-body") {
            let body_txt = std::fs::read_to_string(value)
                .with_context(|| format!("failed to read {}", value.display()))?;
            let body_value = serde_json::from_str::<types::AutomationRequest>(&body_txt)
                .with_context(|| format!("failed to parse {}", value.display()))?;
            request = request.body(body_value);
        }
        self.config
            .execute_automations_command(matches, &mut request)?;
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
        if let Some(value) = matches.get_one::<i64>("from") {
            request = request.from(value.clone());
        }
        if let Some(value) = matches.get_one::<i64>("to") {
            request = request.to(value.clone());
        }
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
    pub async fn execute_calendar_import(
        &self,
        matches: &::clap::ArgMatches,
    ) -> anyhow::Result<()> {
        let mut request = self.client.calendar_import();
        if let Some(value) = matches.get_one::<i64>("window-end") {
            request = request.body_map(|body| body.window_end(value.clone()));
        }
        if let Some(value) = matches.get_one::<i64>("window-start") {
            request = request.body_map(|body| body.window_start(value.clone()));
        }
        if let Some(value) = matches.get_one::<std::path::PathBuf>("json-body") {
            let body_txt = std::fs::read_to_string(value)
                .with_context(|| format!("failed to read {}", value.display()))?;
            let body_value = serde_json::from_str::<types::CalendarImportRequest>(&body_txt)
                .with_context(|| format!("failed to parse {}", value.display()))?;
            request = request.body(body_value);
        }
        self.config.execute_calendar_import(matches, &mut request)?;
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
    pub async fn execute_product_command(
        &self,
        matches: &::clap::ArgMatches,
    ) -> anyhow::Result<()> {
        let mut request = self.client.product_command();
        if let Some(value) = matches.get_one::<::std::string::String>("operation-id") {
            request = request.body_map(|body| body.operation_id(value.clone()));
        }
        if let Some(value) = matches.get_one::<std::path::PathBuf>("json-body") {
            let body_txt = std::fs::read_to_string(value)
                .with_context(|| format!("failed to read {}", value.display()))?;
            let body_value = serde_json::from_str::<types::CommandRequest>(&body_txt)
                .with_context(|| format!("failed to parse {}", value.display()))?;
            request = request.body(body_value);
        }
        self.config.execute_product_command(matches, &mut request)?;
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
    pub async fn execute_connection_status(
        &self,
        matches: &::clap::ArgMatches,
    ) -> anyhow::Result<()> {
        let mut request = self.client.connection_status();
        self.config
            .execute_connection_status(matches, &mut request)?;
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
    pub async fn execute_connection_cancel(
        &self,
        matches: &::clap::ArgMatches,
    ) -> anyhow::Result<()> {
        let mut request = self.client.connection_cancel();
        self.config
            .execute_connection_cancel(matches, &mut request)?;
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
    pub async fn execute_connection_login(
        &self,
        matches: &::clap::ArgMatches,
    ) -> anyhow::Result<()> {
        let mut request = self.client.connection_login();
        self.config
            .execute_connection_login(matches, &mut request)?;
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
    pub async fn execute_connection_logout(
        &self,
        matches: &::clap::ArgMatches,
    ) -> anyhow::Result<()> {
        let mut request = self.client.connection_logout();
        self.config
            .execute_connection_logout(matches, &mut request)?;
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
    pub async fn execute_home_state(&self, matches: &::clap::ArgMatches) -> anyhow::Result<()> {
        let mut request = self.client.home_state();
        self.config.execute_home_state(matches, &mut request)?;
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
    pub async fn execute_home_command(&self, matches: &::clap::ArgMatches) -> anyhow::Result<()> {
        let mut request = self.client.home_command();
        if let Some(value) = matches.get_one::<::std::string::String>("operation-id") {
            request = request.body_map(|body| body.operation_id(value.clone()));
        }
        if let Some(value) = matches.get_one::<std::path::PathBuf>("json-body") {
            let body_txt = std::fs::read_to_string(value)
                .with_context(|| format!("failed to read {}", value.display()))?;
            let body_value = serde_json::from_str::<types::HomeRequest>(&body_txt)
                .with_context(|| format!("failed to parse {}", value.display()))?;
            request = request.body(body_value);
        }
        self.config.execute_home_command(matches, &mut request)?;
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
    pub async fn execute_home_connect(&self, matches: &::clap::ArgMatches) -> anyhow::Result<()> {
        let mut request = self.client.home_connect();
        if let Some(value) = matches.get_one::<::std::string::String>("access-client-id") {
            request = request.body_map(|body| body.access_client_id(value.clone()));
        }
        if let Some(value) = matches.get_one::<::std::string::String>("access-client-secret") {
            request = request.body_map(|body| body.access_client_secret(value.clone()));
        }
        if let Some(value) = matches.get_one::<::std::string::String>("base-url") {
            request = request.body_map(|body| body.base_url(value.clone()));
        }
        if let Some(value) = matches.get_one::<std::path::PathBuf>("json-body") {
            let body_txt = std::fs::read_to_string(value)
                .with_context(|| format!("failed to read {}", value.display()))?;
            let body_value = serde_json::from_str::<types::HomeConnectionRequest>(&body_txt)
                .with_context(|| format!("failed to parse {}", value.display()))?;
            request = request.body(body_value);
        }
        self.config.execute_home_connect(matches, &mut request)?;
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
    pub async fn execute_home_disconnect(
        &self,
        matches: &::clap::ArgMatches,
    ) -> anyhow::Result<()> {
        let mut request = self.client.home_disconnect();
        self.config.execute_home_disconnect(matches, &mut request)?;
        let result = request.send().await;
        match result {
            Ok(r) => {
                self.config.success_no_item(&r);
                Ok(())
            }
            Err(r) => {
                self.config.error(&r);
                Err(anyhow::Error::new(r))
            }
        }
    }
    pub async fn execute_product_state(&self, matches: &::clap::ArgMatches) -> anyhow::Result<()> {
        let mut request = self.client.product_state();
        self.config.execute_product_state(matches, &mut request)?;
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
    pub async fn execute_temporal_executions(
        &self,
        matches: &::clap::ArgMatches,
    ) -> anyhow::Result<()> {
        let mut request = self.client.temporal_executions();
        if let Some(value) = matches.get_one::<::std::string::String>("page") {
            request = request.page(value.clone());
        }
        if let Some(value) = matches.get_one::<::std::string::String>("status") {
            request = request.status(value.clone());
        }
        self.config
            .execute_temporal_executions(matches, &mut request)?;
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
    pub async fn execute_terminal_sessions_list(
        &self,
        matches: &::clap::ArgMatches,
    ) -> anyhow::Result<()> {
        let mut request = self.client.terminal_sessions_list();
        self.config
            .execute_terminal_sessions_list(matches, &mut request)?;
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
    pub async fn execute_terminal_sessions_create(
        &self,
        matches: &::clap::ArgMatches,
    ) -> anyhow::Result<()> {
        let mut request = self.client.terminal_sessions_create();
        if let Some(value) = matches.get_one::<::std::string::String>("id") {
            request = request.body_map(|body| body.id(value.clone()));
        }
        if let Some(value) = matches.get_one::<std::path::PathBuf>("json-body") {
            let body_txt = std::fs::read_to_string(value)
                .with_context(|| format!("failed to read {}", value.display()))?;
            let body_value = serde_json::from_str::<types::CreateTerminalSession>(&body_txt)
                .with_context(|| format!("failed to parse {}", value.display()))?;
            request = request.body(body_value);
        }
        self.config
            .execute_terminal_sessions_create(matches, &mut request)?;
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
    pub async fn execute_terminal_sessions_close(
        &self,
        matches: &::clap::ArgMatches,
    ) -> anyhow::Result<()> {
        let mut request = self.client.terminal_sessions_close();
        if let Some(value) = matches.get_one::<::std::string::String>("id") {
            request = request.id(value.clone());
        }
        self.config
            .execute_terminal_sessions_close(matches, &mut request)?;
        let result = request.send().await;
        match result {
            Ok(r) => {
                self.config.success_no_item(&r);
                Ok(())
            }
            Err(r) => {
                self.config.error(&r);
                Err(anyhow::Error::new(r))
            }
        }
    }
    pub async fn execute_tickets_state(&self, matches: &::clap::ArgMatches) -> anyhow::Result<()> {
        let mut request = self.client.tickets_state();
        self.config.execute_tickets_state(matches, &mut request)?;
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
    pub async fn execute_tickets_command(
        &self,
        matches: &::clap::ArgMatches,
    ) -> anyhow::Result<()> {
        let mut request = self.client.tickets_command();
        if let Some(value) = matches.get_one::<::std::string::String>("operation-id") {
            request = request.body_map(|body| body.operation_id(value.clone()));
        }
        if let Some(value) = matches.get_one::<std::path::PathBuf>("json-body") {
            let body_txt = std::fs::read_to_string(value)
                .with_context(|| format!("failed to read {}", value.display()))?;
            let body_value = serde_json::from_str::<types::TicketCommandRequest>(&body_txt)
                .with_context(|| format!("failed to parse {}", value.display()))?;
            request = request.body(body_value);
        }
        self.config.execute_tickets_command(matches, &mut request)?;
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
    fn execute_automations_state(
        &self,
        matches: &::clap::ArgMatches,
        request: &mut builder::AutomationsState,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    fn execute_automations_command(
        &self,
        matches: &::clap::ArgMatches,
        request: &mut builder::AutomationsCommand,
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
    fn execute_calendar_import(
        &self,
        matches: &::clap::ArgMatches,
        request: &mut builder::CalendarImport,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    fn execute_product_command(
        &self,
        matches: &::clap::ArgMatches,
        request: &mut builder::ProductCommand,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    fn execute_connection_status(
        &self,
        matches: &::clap::ArgMatches,
        request: &mut builder::ConnectionStatus,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    fn execute_connection_cancel(
        &self,
        matches: &::clap::ArgMatches,
        request: &mut builder::ConnectionCancel,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    fn execute_connection_login(
        &self,
        matches: &::clap::ArgMatches,
        request: &mut builder::ConnectionLogin,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    fn execute_connection_logout(
        &self,
        matches: &::clap::ArgMatches,
        request: &mut builder::ConnectionLogout,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    fn execute_home_state(
        &self,
        matches: &::clap::ArgMatches,
        request: &mut builder::HomeState,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    fn execute_home_command(
        &self,
        matches: &::clap::ArgMatches,
        request: &mut builder::HomeCommand,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    fn execute_home_connect(
        &self,
        matches: &::clap::ArgMatches,
        request: &mut builder::HomeConnect,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    fn execute_home_disconnect(
        &self,
        matches: &::clap::ArgMatches,
        request: &mut builder::HomeDisconnect,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    fn execute_product_state(
        &self,
        matches: &::clap::ArgMatches,
        request: &mut builder::ProductState,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    fn execute_temporal_executions(
        &self,
        matches: &::clap::ArgMatches,
        request: &mut builder::TemporalExecutions,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    fn execute_terminal_sessions_list(
        &self,
        matches: &::clap::ArgMatches,
        request: &mut builder::TerminalSessionsList,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    fn execute_terminal_sessions_create(
        &self,
        matches: &::clap::ArgMatches,
        request: &mut builder::TerminalSessionsCreate,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    fn execute_terminal_sessions_close(
        &self,
        matches: &::clap::ArgMatches,
        request: &mut builder::TerminalSessionsClose,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    fn execute_tickets_state(
        &self,
        matches: &::clap::ArgMatches,
        request: &mut builder::TicketsState,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    fn execute_tickets_command(
        &self,
        matches: &::clap::ArgMatches,
        request: &mut builder::TicketsCommand,
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
    AutomationsState,
    AutomationsCommand,
    CalendarState,
    CalendarCommand,
    CalendarImport,
    ProductCommand,
    ConnectionStatus,
    ConnectionCancel,
    ConnectionLogin,
    ConnectionLogout,
    HomeState,
    HomeCommand,
    HomeConnect,
    HomeDisconnect,
    ProductState,
    TemporalExecutions,
    TerminalSessionsList,
    TerminalSessionsCreate,
    TerminalSessionsClose,
    TicketsState,
    TicketsCommand,
    TicketContract,
    WorkspacesState,
    WorkspacesCommand,
    GetVersion,
}
impl CliCommand {
    pub fn iter() -> impl Iterator<Item = CliCommand> {
        vec![
            CliCommand::HealthLive,
            CliCommand::HealthReady,
            CliCommand::AutomationsState,
            CliCommand::AutomationsCommand,
            CliCommand::CalendarState,
            CliCommand::CalendarCommand,
            CliCommand::CalendarImport,
            CliCommand::ProductCommand,
            CliCommand::ConnectionStatus,
            CliCommand::ConnectionCancel,
            CliCommand::ConnectionLogin,
            CliCommand::ConnectionLogout,
            CliCommand::HomeState,
            CliCommand::HomeCommand,
            CliCommand::HomeConnect,
            CliCommand::HomeDisconnect,
            CliCommand::ProductState,
            CliCommand::TemporalExecutions,
            CliCommand::TerminalSessionsList,
            CliCommand::TerminalSessionsCreate,
            CliCommand::TerminalSessionsClose,
            CliCommand::TicketsState,
            CliCommand::TicketsCommand,
            CliCommand::TicketContract,
            CliCommand::WorkspacesState,
            CliCommand::WorkspacesCommand,
            CliCommand::GetVersion,
        ]
        .into_iter()
    }
    pub fn operation_id(&self) -> &'static str {
        match self {
            CliCommand::HealthLive => "health_live",
            CliCommand::HealthReady => "health_ready",
            CliCommand::AutomationsState => "automations_state",
            CliCommand::AutomationsCommand => "automations_command",
            CliCommand::CalendarState => "calendar_state",
            CliCommand::CalendarCommand => "calendar_command",
            CliCommand::CalendarImport => "calendar_import",
            CliCommand::ProductCommand => "product_command",
            CliCommand::ConnectionStatus => "connection_status",
            CliCommand::ConnectionCancel => "connection_cancel",
            CliCommand::ConnectionLogin => "connection_login",
            CliCommand::ConnectionLogout => "connection_logout",
            CliCommand::HomeState => "home_state",
            CliCommand::HomeCommand => "home_command",
            CliCommand::HomeConnect => "home_connect",
            CliCommand::HomeDisconnect => "home_disconnect",
            CliCommand::ProductState => "product_state",
            CliCommand::TemporalExecutions => "temporal_executions",
            CliCommand::TerminalSessionsList => "terminal_sessions_list",
            CliCommand::TerminalSessionsCreate => "terminal_sessions_create",
            CliCommand::TerminalSessionsClose => "terminal_sessions_close",
            CliCommand::TicketsState => "tickets_state",
            CliCommand::TicketsCommand => "tickets_command",
            CliCommand::TicketContract => "ticket_contract",
            CliCommand::WorkspacesState => "workspaces_state",
            CliCommand::WorkspacesCommand => "workspaces_command",
            CliCommand::GetVersion => "get_version",
        }
    }
}
