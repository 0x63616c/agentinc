//! Smart Home: switches and climate from World Wide Webb's control center.
//! Reads go straight to the control center; changes are durable actions.
use crate::{
    control_center::{AccessToken, ControlCenter},
    durable::{self, ActionKind, ActionRow, ActionView},
    product::{ApiError, ErrorBody, Product},
    secrets::SecretStore,
    tickets::Actor,
};
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::{get, post, put},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{FromRow, PgPool};
use std::sync::Arc;
use utoipa::ToSchema;

/// The switches AgentInc exposes, each one control-center group.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SwitchKey {
    All,
    Lamps,
    BedroomLamps,
    LivingRoomLamps,
    KitchenCeiling,
    UnderCabinet,
}
impl SwitchKey {
    pub const ALL: [Self; 6] = [
        Self::All,
        Self::Lamps,
        Self::BedroomLamps,
        Self::LivingRoomLamps,
        Self::KitchenCeiling,
        Self::UnderCabinet,
    ];
    fn control(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Lamps => "lamps",
            Self::BedroomLamps => "bedroomLamps",
            Self::LivingRoomLamps => "otherLamps",
            Self::KitchenCeiling => "ceiling",
            Self::UnderCabinet => "cabinet",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::All => "All lights",
            Self::Lamps => "All lamps",
            Self::BedroomLamps => "Bedroom lamps",
            Self::LivingRoomLamps => "Living room lamps",
            Self::KitchenCeiling => "Kitchen ceiling",
            Self::UnderCabinet => "Under cabinet",
        }
    }
    fn room(self) -> &'static str {
        match self {
            Self::All | Self::Lamps => "Everywhere",
            Self::BedroomLamps => "Bedroom",
            Self::LivingRoomLamps => "Living room",
            Self::KitchenCeiling | Self::UnderCabinet => "Kitchen",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClimateMode {
    Off,
    Cool,
    Heat,
    HeatCool,
}
impl ClimateMode {
    fn wire(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Cool => "cool",
            Self::Heat => "heat",
            Self::HeatCool => "heat_cool",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Cool => "Cool",
            Self::Heat => "Heat",
            Self::HeatCool => "Auto",
        }
    }
}
/// The control center accepts whole degrees Fahrenheit in this band.
pub const SETPOINT_MIN: i64 = 67;
pub const SETPOINT_MAX: i64 = 77;
/// Auto keeps heating and cooling setpoints at least this far apart.
pub const RANGE_GAP: i64 = 2;

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema, PartialEq)]
pub struct HomeSwitch {
    pub key: SwitchKey,
    pub label: String,
    pub room: String,
    pub on: bool,
    /// A change is on its way to the lights.
    pub pending: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema, PartialEq)]
pub struct HomeClimate {
    pub mode: ClimateMode,
    /// Indoor temperature in °F.
    pub ambient: Option<f64>,
    /// What the system is doing now, such as Cooling or Idle.
    pub action: Option<String>,
    pub target: Option<i64>,
    pub target_low: Option<i64>,
    pub target_high: Option<i64>,
    pub pending: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema, FromRow, PartialEq)]
pub struct HomeConnection {
    pub base_url: String,
    /// A Cloudflare Access service token is stored in the Keychain.
    pub access_token: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct HomeSnapshot {
    pub connection: Option<HomeConnection>,
    pub reachable: bool,
    pub error: Option<String>,
    pub switches: Vec<HomeSwitch>,
    pub climate: Option<HomeClimate>,
    /// Recent Smart Home actions, newest first.
    pub actions: Vec<ActionView>,
}
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HomeCommand {
    Switch { key: SwitchKey, on: bool },
    SetClimateMode { mode: ClimateMode },
    SetClimateTarget { target: i64 },
    SetClimateRange { low: i64, high: i64 },
}
impl HomeCommand {
    pub(crate) fn summary(&self) -> String {
        match self {
            Self::Switch { key, on } => {
                format!("{} {}", key.label(), if *on { "on" } else { "off" })
            }
            Self::SetClimateMode { mode } => format!("Climate {}", mode.label()),
            Self::SetClimateTarget { target } => format!("Climate to {target}°"),
            Self::SetClimateRange { low, high } => format!("Climate {low}–{high}°"),
        }
    }
    fn validate(&self) -> Result<(), ApiError> {
        let band = SETPOINT_MIN..=SETPOINT_MAX;
        let valid = match self {
            Self::Switch { .. } | Self::SetClimateMode { .. } => true,
            Self::SetClimateTarget { target } => band.contains(target),
            Self::SetClimateRange { low, high } => {
                band.contains(low) && band.contains(high) && high - low >= RANGE_GAP
            }
        };
        if valid {
            Ok(())
        } else {
            Err(invalid(&format!(
                "Use whole degrees from {SETPOINT_MIN}° to {SETPOINT_MAX}°, with Auto at least {RANGE_GAP}° apart."
            )))
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct HomeRequest {
    pub operation_id: String,
    pub command: HomeCommand,
}
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct HomeReceipt {
    /// The durable action that applies the command.
    pub result_id: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct HomeConnectionRequest {
    pub base_url: String,
    /// Cloudflare Access service token; omit both for an endpoint without Access.
    pub access_client_id: Option<String>,
    pub access_client_secret: Option<String>,
}

fn invalid(message: &str) -> ApiError {
    ApiError::new(StatusCode::BAD_REQUEST, "invalid", message)
}

/// The Smart Home service: Keychain access and the control-center HTTP client.
#[derive(Clone)]
pub struct Home {
    secrets: Arc<dyn SecretStore>,
    http: reqwest::Client,
}
impl Home {
    pub fn new(secrets: Arc<dyn SecretStore>) -> Self {
        Self {
            secrets,
            http: ControlCenter::http(),
        }
    }
    fn secret_names(workspace: &str) -> [String; 2] {
        [
            format!("control-center/{workspace}/access-client-id"),
            format!("control-center/{workspace}/access-client-secret"),
        ]
    }
    /// The workspace's control center, or `None` when it is not connected.
    pub(crate) async fn control_center(
        &self,
        pool: &PgPool,
        workspace: &str,
    ) -> anyhow::Result<Option<ControlCenter>> {
        let Some(connection) = connection(pool, workspace).await? else {
            return Ok(None);
        };
        let access = if connection.access_token {
            let [id, secret] = Self::secret_names(workspace);
            match (self.secrets.get(&id)?, self.secrets.get(&secret)?) {
                (Some(client_id), Some(client_secret)) => Some(AccessToken {
                    client_id,
                    client_secret,
                }),
                _ => anyhow::bail!(
                    "The Access token is missing from the Keychain. Reconnect in Settings."
                ),
            }
        } else {
            None
        };
        Ok(Some(ControlCenter::new(
            self.http.clone(),
            &connection.base_url,
            access,
        )))
    }
}

async fn connection(pool: &PgPool, workspace: &str) -> Result<Option<HomeConnection>, sqlx::Error> {
    sqlx::query_as("SELECT base_url,access_token FROM home_connections WHERE workspace_id=$1")
        .bind(workspace)
        .fetch_optional(pool)
        .await
}

#[derive(Clone)]
pub struct HomeState {
    product: Product,
    home: Home,
}
pub fn router(product: Product, home: Home) -> Router {
    Router::new()
        .route("/v1/home", get(state))
        .route("/v1/home/commands", post(command))
        .route("/v1/home/connection", put(connect).delete(disconnect))
        .with_state(HomeState { product, home })
}
async fn owner(state: &HomeState, headers: &HeaderMap) -> Result<Actor, ApiError> {
    state.product.authorize(headers)?;
    Ok(Actor::owner_in(
        crate::workspaces::current(&state.product.pool).await?,
    ))
}
#[utoipa::path(get,path="/v1/home",operation_id="home_state",responses((status=200,body=HomeSnapshot),(status=401,body=ErrorBody),(status=503,body=ErrorBody)))]
pub async fn state(
    State(state): State<HomeState>,
    headers: HeaderMap,
) -> Result<Json<HomeSnapshot>, ApiError> {
    let actor = owner(&state, &headers).await?;
    Ok(Json(
        snapshot(&state.product.pool, &state.home, &actor).await?,
    ))
}
#[utoipa::path(post,path="/v1/home/commands",operation_id="home_command",request_body=HomeRequest,responses((status=200,body=HomeReceipt),(status=400,body=ErrorBody),(status=401,body=ErrorBody),(status=409,body=ErrorBody),(status=503,body=ErrorBody)))]
pub async fn command(
    State(state): State<HomeState>,
    headers: HeaderMap,
    Json(request): Json<HomeRequest>,
) -> Result<Json<HomeReceipt>, ApiError> {
    let actor = owner(&state, &headers).await?;
    Ok(Json(execute(&state.product.pool, &actor, request).await?))
}
#[utoipa::path(put,path="/v1/home/connection",operation_id="home_connect",request_body=HomeConnectionRequest,responses((status=200,body=HomeConnection),(status=400,body=ErrorBody),(status=401,body=ErrorBody),(status=503,body=ErrorBody)))]
pub async fn connect(
    State(state): State<HomeState>,
    headers: HeaderMap,
    Json(request): Json<HomeConnectionRequest>,
) -> Result<Json<HomeConnection>, ApiError> {
    let actor = owner(&state, &headers).await?;
    Ok(Json(
        save_connection(&state.product.pool, &state.home, &actor, request).await?,
    ))
}
#[utoipa::path(delete,path="/v1/home/connection",operation_id="home_disconnect",responses((status=204),(status=401,body=ErrorBody),(status=503,body=ErrorBody)))]
pub async fn disconnect(
    State(state): State<HomeState>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let actor = owner(&state, &headers).await?;
    for name in Home::secret_names(&actor.workspace) {
        state.home.secrets.delete(&name).map_err(keychain)?;
    }
    sqlx::query("DELETE FROM home_connections WHERE workspace_id=$1")
        .bind(&actor.workspace)
        .execute(&state.product.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
fn keychain(error: anyhow::Error) -> ApiError {
    ApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "keychain_unavailable",
        &error.to_string(),
    )
}

pub(crate) async fn save_connection(
    pool: &PgPool,
    home: &Home,
    actor: &Actor,
    request: HomeConnectionRequest,
) -> Result<HomeConnection, ApiError> {
    let base_url = request.base_url.trim().trim_end_matches('/').to_owned();
    let parsed =
        reqwest::Url::parse(&base_url).map_err(|_| invalid("Use an http or https URL."))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err(invalid("Use an http or https URL."));
    }
    let token = match (
        request
            .access_client_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
        request
            .access_client_secret
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
    ) {
        (Some(id), Some(secret)) => Some((id, secret)),
        (None, None) => None,
        _ => {
            return Err(invalid(
                "Enter both the Access client ID and secret, or neither.",
            ));
        }
    };
    let [id_name, secret_name] = Home::secret_names(&actor.workspace);
    match token {
        Some((id, secret)) => {
            home.secrets.set(&id_name, id).map_err(keychain)?;
            home.secrets.set(&secret_name, secret).map_err(keychain)?;
        }
        None => {
            home.secrets.delete(&id_name).map_err(keychain)?;
            home.secrets.delete(&secret_name).map_err(keychain)?;
        }
    }
    Ok(sqlx::query_as(
        "INSERT INTO home_connections(workspace_id,base_url,access_token) VALUES($1,$2,$3)
         ON CONFLICT(workspace_id) DO UPDATE SET base_url=excluded.base_url,access_token=excluded.access_token
         RETURNING base_url,access_token",
    )
    .bind(&actor.workspace)
    .bind(base_url)
    .bind(token.is_some())
    .fetch_one(pool)
    .await?)
}

pub(crate) async fn execute(
    pool: &PgPool,
    actor: &Actor,
    request: HomeRequest,
) -> Result<HomeReceipt, ApiError> {
    if uuid::Uuid::parse_str(&request.operation_id).is_err() {
        return Err(invalid("Use a UUID operation ID."));
    }
    request.command.validate()?;
    let payload = json!(request.command);
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("home/{}/{}", actor.workspace, request.operation_id))
        .execute(&mut *tx)
        .await?;
    let prior: Option<(Value, String)> = sqlx::query_as(
        "SELECT command,result_id FROM home_receipts WHERE workspace_id=$1 AND actor_id=$2 AND operation_id=$3",
    )
    .bind(&actor.workspace)
    .bind(&actor.id)
    .bind(&request.operation_id)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some((old, result_id)) = prior {
        if old != payload {
            return Err(ApiError::conflict());
        }
        return Ok(HomeReceipt { result_id });
    }
    let connected: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM home_connections WHERE workspace_id=$1)")
            .bind(&actor.workspace)
            .fetch_one(&mut *tx)
            .await?;
    if !connected {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "not_connected",
            "Connect the control center in Settings first.",
        ));
    }
    let result_id = durable::enqueue(&mut tx, actor, ActionKind::Home, &payload, None).await?;
    sqlx::query("INSERT INTO home_receipts(workspace_id,actor_id,operation_id,command,result_id) VALUES($1,$2,$3,$4,$5)")
        .bind(&actor.workspace)
        .bind(&actor.id)
        .bind(&request.operation_id)
        .bind(&payload)
        .bind(&result_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(HomeReceipt { result_id })
}

pub(crate) async fn snapshot(
    pool: &PgPool,
    home: &Home,
    actor: &Actor,
) -> Result<HomeSnapshot, ApiError> {
    let actions = durable::recent(pool, &actor.workspace, ActionKind::Home, 8).await?;
    let mut snapshot = HomeSnapshot {
        connection: connection(pool, &actor.workspace).await?,
        reachable: false,
        error: None,
        switches: SwitchKey::ALL
            .into_iter()
            .map(|key| HomeSwitch {
                key,
                label: key.label().into(),
                room: key.room().into(),
                on: false,
                pending: false,
            })
            .collect(),
        climate: None,
        actions: vec![],
    };
    match home.control_center(pool, &actor.workspace).await {
        Ok(None) => {}
        Err(error) => snapshot.error = Some(error.to_string()),
        Ok(Some(center)) => match tokio::try_join!(center.controls(), center.climate()) {
            Err(error) => snapshot.error = Some(error.to_string()),
            Ok((controls, climate)) => {
                snapshot.reachable = true;
                for switch in &mut snapshot.switches {
                    let group = match switch.key {
                        SwitchKey::All => controls.all,
                        SwitchKey::Lamps => controls.lamps,
                        SwitchKey::BedroomLamps => controls.bedroom_lamps,
                        SwitchKey::LivingRoomLamps => controls.other_lamps,
                        SwitchKey::KitchenCeiling => controls.ceiling,
                        SwitchKey::UnderCabinet => controls.cabinet,
                    };
                    switch.on = group.on;
                    switch.pending = group.pending;
                }
                snapshot.climate = Some(HomeClimate {
                    mode: match climate.mode.as_str() {
                        "cool" => ClimateMode::Cool,
                        "heat" => ClimateMode::Heat,
                        "heat_cool" => ClimateMode::HeatCool,
                        _ => ClimateMode::Off,
                    },
                    ambient: climate.ambient,
                    action: climate.action,
                    target: climate.target,
                    target_low: climate.target_low,
                    target_high: climate.target_high,
                    pending: false,
                });
            }
        },
    }
    // Unfinished actions win over the last reading, oldest first, so the newest
    // intent is what the page shows while it travels.
    for action in actions.iter().rev().filter(|a| a.is_open()) {
        let Ok(command) = serde_json::from_value::<HomeCommand>(action.input.clone()) else {
            continue;
        };
        match command {
            HomeCommand::Switch { key, on } => {
                if let Some(switch) = snapshot.switches.iter_mut().find(|s| s.key == key) {
                    switch.on = on;
                    switch.pending = true;
                }
            }
            command => {
                if let Some(climate) = &mut snapshot.climate {
                    climate.pending = true;
                    match command {
                        HomeCommand::SetClimateMode { mode } => climate.mode = mode,
                        HomeCommand::SetClimateTarget { target } => climate.target = Some(target),
                        HomeCommand::SetClimateRange { low, high } => {
                            climate.target_low = Some(low);
                            climate.target_high = Some(high);
                        }
                        HomeCommand::Switch { .. } => {}
                    }
                }
            }
        }
    }
    snapshot.actions = actions.iter().map(ActionRow::view).collect();
    Ok(snapshot)
}

/// Apply one Smart Home action. Every command sets a state, so a retry after a
/// lost answer cannot double-apply.
pub(crate) async fn apply(
    pool: &PgPool,
    home: &Home,
    workspace: &str,
    input: &Value,
) -> anyhow::Result<()> {
    let command: HomeCommand = serde_json::from_value(input.clone())?;
    let center = home
        .control_center(pool, workspace)
        .await?
        .ok_or_else(|| anyhow::anyhow!("The control center was disconnected."))?;
    match command {
        HomeCommand::Switch { key, on } => center.toggle(key.control(), on).await,
        HomeCommand::SetClimateMode { mode } => center.set_mode(mode.wire()).await,
        HomeCommand::SetClimateTarget { target } => center.set_target(target).await,
        HomeCommand::SetClimateRange { low, high } => center.set_range(low, high).await,
    }
}
