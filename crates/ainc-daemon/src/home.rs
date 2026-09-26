//! Smart Home: switches and climate from World Wide Webb's control center.
//! Reads go straight to the control center; changes are durable actions.
//! The control-center link is the workspace's Smart Home Connection.
use crate::{
    control_center::{AccessToken, ControlCenter},
    durable::{self, ActionKind, ActionRow, ActionState, ActionView, Applied},
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
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use utoipa::ToSchema;

/// The switches AgentInc exposes, each one control-center group.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, ToSchema, PartialEq, Eq, Hash)]
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
    /// The individual switches a group covers; empty for a single switch.
    pub fn members(self) -> &'static [Self] {
        match self {
            Self::All => &[
                Self::BedroomLamps,
                Self::LivingRoomLamps,
                Self::KitchenCeiling,
                Self::UnderCabinet,
            ],
            Self::Lamps => &[Self::BedroomLamps, Self::LivingRoomLamps],
            _ => &[],
        }
    }
    /// The single switches this one sets.
    fn covers(self) -> &'static [Self] {
        match self.members() {
            [] => std::slice::from_ref(match self {
                Self::BedroomLamps => &Self::BedroomLamps,
                Self::LivingRoomLamps => &Self::LivingRoomLamps,
                Self::KitchenCeiling => &Self::KitchenCeiling,
                _ => &Self::UnderCabinet,
            }),
            members => members,
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
    /// Every light it covers is on.
    pub on: bool,
    /// A change is on its way to the lights.
    pub pending: bool,
    /// The single switches a group covers; empty for a single switch.
    pub members: Vec<SwitchKey>,
    /// How many of the lights it covers are on, out of `total`.
    pub lit: i64,
    pub total: i64,
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
    /// The setpoints the thermostat accepts, and the Auto gap.
    pub min: i64,
    pub max: i64,
    pub gap: i64,
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
    /// A later `self` makes an earlier `other` moot when it sets everything
    /// `other` would set.
    fn covers(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Switch { key: newer, .. }, Self::Switch { key: older, .. }) => older
                .covers()
                .iter()
                .all(|light| newer.covers().contains(light)),
            (newer, older) => std::mem::discriminant(newer) == std::mem::discriminant(older),
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
    /// Access tokens by workspace, so polling never waits on the Keychain.
    tokens: Arc<Mutex<HashMap<String, AccessToken>>>,
}
impl Home {
    pub fn new(secrets: Arc<dyn SecretStore>) -> Self {
        Self {
            secrets,
            http: ControlCenter::http(),
            tokens: Arc::default(),
        }
    }
    fn secret_names(workspace: &str) -> [String; 2] {
        [
            format!("control-center/{workspace}/access-client-id"),
            format!("control-center/{workspace}/access-client-secret"),
        ]
    }
    /// Keychain calls block, so they run off the async threads.
    async fn keychain<T: Send + 'static>(
        &self,
        work: impl FnOnce(&dyn SecretStore) -> anyhow::Result<T> + Send + 'static,
    ) -> anyhow::Result<T> {
        let secrets = self.secrets.clone();
        tokio::task::spawn_blocking(move || work(secrets.as_ref())).await?
    }
    async fn access_token(&self, workspace: &str) -> anyhow::Result<AccessToken> {
        if let Some(token) = self.tokens.lock().expect("token cache").get(workspace) {
            return Ok(token.clone());
        }
        let [id, secret] = Self::secret_names(workspace);
        let token = self
            .keychain(
                move |secrets| match (secrets.get(&id)?, secrets.get(&secret)?) {
                    (Some(client_id), Some(client_secret)) => Ok(AccessToken {
                        client_id,
                        client_secret,
                    }),
                    _ => anyhow::bail!(
                        "The Access token is missing from the Keychain. Reconnect in Settings."
                    ),
                },
            )
            .await?;
        self.tokens
            .lock()
            .expect("token cache")
            .insert(workspace.to_owned(), token.clone());
        Ok(token)
    }
    fn forget(&self, workspace: &str) {
        self.tokens.lock().expect("token cache").remove(workspace);
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
            Some(self.access_token(workspace).await?)
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
    sqlx::query("DELETE FROM home_connections WHERE workspace_id=$1")
        .bind(&actor.workspace)
        .execute(&state.product.pool)
        .await?;
    state.home.forget(&actor.workspace);
    let names = Home::secret_names(&actor.workspace);
    state
        .home
        .keychain(move |secrets| {
            for name in &names {
                secrets.delete(name)?;
            }
            Ok(())
        })
        .await
        .map_err(keychain)?;
    Ok(StatusCode::NO_CONTENT)
}
fn keychain(error: anyhow::Error) -> ApiError {
    ApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "keychain_unavailable",
        &error.to_string(),
    )
}
/// A token may travel in the clear only to this Mac or across the tailnet.
fn private_host(url: &reqwest::Url) -> bool {
    match url.host() {
        Some(url::Host::Ipv4(ip)) => {
            ip.is_loopback() || (ip.octets()[0] == 100 && (64..=127).contains(&ip.octets()[1]))
        }
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        Some(url::Host::Domain(host)) => host == "localhost" || host.ends_with(".ts.net"),
        None => false,
    }
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
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty()),
        request
            .access_client_secret
            .map(|s| s.trim().to_owned())
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
    if token.is_some() && parsed.scheme() != "https" && !private_host(&parsed) {
        return Err(invalid(
            "Use an https address so the Access token is never sent in the clear.",
        ));
    }
    // The row and the Keychain change together: a Keychain failure rolls the row back.
    let mut tx = pool.begin().await?;
    let saved: HomeConnection = sqlx::query_as(
        "INSERT INTO home_connections(workspace_id,base_url,access_token) VALUES($1,$2,$3)
         ON CONFLICT(workspace_id) DO UPDATE SET base_url=excluded.base_url,access_token=excluded.access_token
         RETURNING base_url,access_token",
    )
    .bind(&actor.workspace)
    .bind(base_url)
    .bind(token.is_some())
    .fetch_one(&mut *tx)
    .await?;
    home.forget(&actor.workspace);
    let [id_name, secret_name] = Home::secret_names(&actor.workspace);
    home.keychain(move |secrets| match token {
        Some((id, secret)) => {
            secrets.set(&id_name, &id)?;
            secrets.set(&secret_name, &secret)
        }
        None => {
            secrets.delete(&id_name)?;
            secrets.delete(&secret_name)
        }
    })
    .await
    .map_err(keychain)?;
    tx.commit().await?;
    Ok(saved)
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
    let result_id = durable::enqueue(
        &mut tx,
        actor,
        ActionKind::Home,
        &payload,
        &request.command.summary(),
        None,
    )
    .await?;
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

/// The home's single lights, from the control center and then from changes
/// still on their way, with every group derived from its members.
fn switches(
    reading: Option<&crate::control_center::Controls>,
    open: &[HomeCommand],
) -> Vec<HomeSwitch> {
    let mut lights: HashMap<SwitchKey, (bool, bool)> = HashMap::new();
    let mut pressed: Vec<SwitchKey> = vec![];
    if let Some(controls) = reading {
        for (key, group) in [
            (SwitchKey::BedroomLamps, controls.bedroom_lamps),
            (SwitchKey::LivingRoomLamps, controls.other_lamps),
            (SwitchKey::KitchenCeiling, controls.ceiling),
            (SwitchKey::UnderCabinet, controls.cabinet),
        ] {
            lights.insert(key, (group.on, group.pending));
        }
        for (key, group) in [
            (SwitchKey::All, controls.all),
            (SwitchKey::Lamps, controls.lamps),
        ] {
            if group.pending {
                pressed.push(key);
            }
        }
    }
    // Oldest first, so the newest wanted state is what shows while it travels.
    for command in open {
        if let HomeCommand::Switch { key, on } = command {
            for light in key.covers() {
                lights.insert(*light, (*on, true));
            }
            pressed.push(*key);
        }
    }
    SwitchKey::ALL
        .into_iter()
        .map(|key| {
            let covered = key.covers();
            let lit = covered
                .iter()
                .filter(|light| lights.get(light).is_some_and(|(on, _)| *on))
                .count() as i64;
            let pending = pressed.contains(&key)
                || (key.members().is_empty()
                    && lights.get(&key).is_some_and(|(_, pending)| *pending));
            HomeSwitch {
                key,
                label: key.label().into(),
                room: key.room().into(),
                on: lit == covered.len() as i64,
                pending,
                members: key.members().to_vec(),
                lit,
                total: covered.len() as i64,
            }
        })
        .collect()
}

pub(crate) async fn snapshot(
    pool: &PgPool,
    home: &Home,
    actor: &Actor,
) -> Result<HomeSnapshot, ApiError> {
    let actions = durable::recent(pool, &actor.workspace, ActionKind::Home, 8).await?;
    let open: Vec<HomeCommand> = actions
        .iter()
        .rev()
        .filter(|a| a.is_open())
        .filter_map(|a| serde_json::from_value(a.input.clone()).ok())
        .collect();
    let mut snapshot = HomeSnapshot {
        connection: connection(pool, &actor.workspace).await?,
        reachable: false,
        error: None,
        switches: switches(None, &open),
        climate: None,
        actions: actions.iter().map(ActionRow::view).collect(),
    };
    match home.control_center(pool, &actor.workspace).await {
        Ok(None) => {}
        Err(error) => snapshot.error = Some(error.to_string()),
        Ok(Some(center)) => match tokio::try_join!(center.controls(), center.climate()) {
            Err(error) => snapshot.error = Some(error.to_string()),
            Ok((controls, climate)) => {
                snapshot.reachable = true;
                snapshot.switches = switches(Some(&controls), &open);
                let mut reading = HomeClimate {
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
                    min: SETPOINT_MIN,
                    max: SETPOINT_MAX,
                    gap: RANGE_GAP,
                };
                for command in &open {
                    reading.pending |= !matches!(command, HomeCommand::Switch { .. });
                    match command {
                        HomeCommand::SetClimateMode { mode } => reading.mode = *mode,
                        HomeCommand::SetClimateTarget { target } => reading.target = Some(*target),
                        HomeCommand::SetClimateRange { low, high } => {
                            reading.target_low = Some(*low);
                            reading.target_high = Some(*high);
                        }
                        HomeCommand::Switch { .. } => {}
                    }
                }
                snapshot.climate = Some(reading);
            }
        },
    }
    Ok(snapshot)
}

/// Apply one Smart Home action, in order. Changes in a workspace apply one at
/// a time; a newer change that sets everything this one would makes it moot,
/// and an older unfinished change goes first. Every command sets a state, so
/// a retry after a lost answer cannot double-apply.
pub(crate) async fn apply(
    pool: &PgPool,
    home: &Home,
    workspace: &str,
    seq: i64,
    input: &Value,
) -> anyhow::Result<Applied> {
    let command: HomeCommand = serde_json::from_value(input.clone())?;
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("home-effects/{workspace}"))
        .execute(&mut *tx)
        .await?;
    let others: Vec<(i64, Value, ActionState)> = sqlx::query_as(
        "SELECT seq,input,state FROM durable_actions WHERE workspace_id=$1 AND kind='home' AND seq<>$2
         AND state IN ('queued','running','completed') AND created_at > extract(epoch FROM now())::bigint-120",
    )
    .bind(workspace)
    .bind(seq)
    .fetch_all(&mut *tx)
    .await?;
    for (other, input, _) in others.iter().filter(|(other, ..)| *other > seq) {
        let _ = other;
        if serde_json::from_value::<HomeCommand>(input.clone())
            .is_ok_and(|newer| newer.covers(&command))
        {
            return Ok(Applied::Superseded);
        }
    }
    if others
        .iter()
        .any(|(other, _, state)| *other < seq && *state != ActionState::Completed)
    {
        anyhow::bail!("Waiting for an earlier change to finish.");
    }
    let center = home
        .control_center(pool, workspace)
        .await?
        .ok_or_else(|| anyhow::anyhow!("The control center was disconnected."))?;
    match command {
        HomeCommand::Switch { key, on } => center.toggle(key.control(), on).await,
        HomeCommand::SetClimateMode { mode } => center.set_mode(mode.wire()).await,
        HomeCommand::SetClimateTarget { target } => center.set_target(target).await,
        HomeCommand::SetClimateRange { low, high } => center.set_range(low, high).await,
    }?;
    tx.commit().await?;
    Ok(Applied::Done)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_newer_change_only_supersedes_what_it_fully_covers() {
        let switch = |key, on| HomeCommand::Switch { key, on };
        let all_off = switch(SwitchKey::All, false);
        let bedroom_on = switch(SwitchKey::BedroomLamps, true);
        assert!(all_off.covers(&bedroom_on));
        assert!(
            !bedroom_on.covers(&all_off),
            "the other lights still go off"
        );
        assert!(switch(SwitchKey::Lamps, true).covers(&switch(SwitchKey::LivingRoomLamps, false)));
        let target = HomeCommand::SetClimateTarget { target: 70 };
        assert!(HomeCommand::SetClimateTarget { target: 72 }.covers(&target));
        assert!(
            !HomeCommand::SetClimateMode {
                mode: ClimateMode::Cool
            }
            .covers(&target)
        );
    }

    #[test]
    fn groups_follow_their_members_and_pending_changes() {
        let reading = crate::control_center::Controls {
            all: Default::default(),
            lamps: Default::default(),
            bedroom_lamps: crate::control_center::Group {
                on: true,
                pending: false,
            },
            other_lamps: Default::default(),
            ceiling: crate::control_center::Group {
                on: true,
                pending: false,
            },
            cabinet: Default::default(),
        };
        let state = |switches: &[HomeSwitch], key| {
            let s = switches.iter().find(|s| s.key == key).unwrap();
            (s.on, s.lit, s.total, s.pending)
        };
        let now = switches(Some(&reading), &[]);
        assert_eq!(state(&now, SwitchKey::All), (false, 2, 4, false));
        assert_eq!(state(&now, SwitchKey::Lamps), (false, 1, 2, false));
        let travelling = switches(
            Some(&reading),
            &[HomeCommand::Switch {
                key: SwitchKey::All,
                on: true,
            }],
        );
        assert_eq!(state(&travelling, SwitchKey::All), (true, 4, 4, true));
        assert_eq!(
            state(&travelling, SwitchKey::UnderCabinet),
            (true, 1, 1, true)
        );
    }
}
