//! Calendar events owned by AgentInc, plus read-only mirrors of the user's macOS
//! calendars. The Mac app reads the calendar store (it holds the permission) and
//! hands each snapshot over as an import; the import is a durable action.
use crate::{
    durable::{self, ActionKind, ActionRow, ActionView},
    product::{ApiError, ErrorBody, Product},
    tickets::Actor,
};
use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool};
use utoipa::{IntoParams, ToSchema};

const DAY: i64 = 86_400;
/// The longest event and the widest import window AgentInc accepts.
const MAX_SPAN: i64 = 400 * DAY;
const MAX_IMPORT: usize = 5_000;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, ToSchema, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum EventSource {
    /// Created in AgentInc; people and agents can edit it.
    Agentinc,
    /// Mirrored from the macOS calendar store; edit it in Calendar.
    Macos,
}
/// Times are Unix seconds; `ends_at` is exclusive, so an all-day event runs from
/// one local midnight to the next.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema, FromRow, PartialEq)]
pub struct CalendarEvent {
    pub id: String,
    pub source: EventSource,
    pub calendar: String,
    pub color: Option<String>,
    pub title: String,
    pub location: Option<String>,
    pub notes: Option<String>,
    pub starts_at: i64,
    pub ends_at: i64,
    pub all_day: bool,
    pub revision: i64,
}
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct CalendarSnapshot {
    pub events: Vec<CalendarEvent>,
    /// The latest import from the macOS calendar store.
    pub last_import: Option<ActionView>,
}
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CalendarCommand {
    Create {
        title: String,
        starts_at: i64,
        ends_at: i64,
        all_day: bool,
        location: Option<String>,
        notes: Option<String>,
    },
    Update {
        id: String,
        revision: i64,
        title: String,
        starts_at: i64,
        ends_at: i64,
        all_day: bool,
        location: Option<String>,
        notes: Option<String>,
    },
    Delete {
        id: String,
        revision: i64,
    },
}
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct CalendarRequest {
    pub operation_id: String,
    pub command: CalendarCommand,
}
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct CalendarReceipt {
    pub result_id: String,
}
/// One event read from the macOS calendar store. `external_id` is stable per
/// occurrence, so repeating events import once per date.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema, PartialEq)]
pub struct ImportedEvent {
    pub external_id: String,
    pub calendar: String,
    pub color: Option<String>,
    pub title: String,
    pub location: Option<String>,
    pub notes: Option<String>,
    pub starts_at: i64,
    pub ends_at: i64,
    pub all_day: bool,
}
/// Everything the calendar store holds between `window_start` and
/// `window_end`; mirrored events in that window that are absent are removed.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema, PartialEq)]
pub struct CalendarImportRequest {
    pub window_start: i64,
    pub window_end: i64,
    pub events: Vec<ImportedEvent>,
}
#[derive(Clone, Debug, Deserialize, IntoParams)]
pub struct CalendarRange {
    /// Unix seconds; defaults to 31 days ago.
    pub from: Option<i64>,
    /// Unix seconds, exclusive; defaults to 180 days ahead.
    pub to: Option<i64>,
}

fn invalid(message: &str) -> ApiError {
    ApiError::new(StatusCode::BAD_REQUEST, "invalid", message)
}
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}
fn clip(text: &str, limit: usize) -> String {
    text.trim().chars().take(limit).collect()
}
fn optional(text: Option<String>, limit: usize) -> Option<String> {
    text.map(|t| clip(&t, limit)).filter(|t| !t.is_empty())
}

pub fn router(product: Product) -> Router {
    Router::new()
        .route("/v1/calendar", get(state))
        .route("/v1/calendar/commands", post(command))
        .route("/v1/calendar/imports", post(import))
        .with_state(product)
}
async fn owner(product: &Product, headers: &HeaderMap) -> Result<Actor, ApiError> {
    product.authorize(headers)?;
    Ok(Actor::owner_in(
        crate::workspaces::current(&product.pool).await?,
    ))
}
#[utoipa::path(get,path="/v1/calendar",operation_id="calendar_state",params(CalendarRange),responses((status=200,body=CalendarSnapshot),(status=400,body=ErrorBody),(status=401,body=ErrorBody),(status=503,body=ErrorBody)))]
pub async fn state(
    State(product): State<Product>,
    headers: HeaderMap,
    Query(range): Query<CalendarRange>,
) -> Result<Json<CalendarSnapshot>, ApiError> {
    let actor = owner(&product, &headers).await?;
    let from = range.from.unwrap_or_else(|| now() - 31 * DAY);
    let to = range.to.unwrap_or_else(|| now() + 180 * DAY);
    if to <= from || to - from > MAX_SPAN {
        return Err(invalid("Ask for a range of up to 400 days."));
    }
    Ok(Json(snapshot(&product.pool, &actor, from, to).await?))
}
#[utoipa::path(post,path="/v1/calendar/commands",operation_id="calendar_command",request_body=CalendarRequest,responses((status=200,body=CalendarReceipt),(status=400,body=ErrorBody),(status=401,body=ErrorBody),(status=409,body=ErrorBody),(status=503,body=ErrorBody)))]
pub async fn command(
    State(product): State<Product>,
    headers: HeaderMap,
    Json(request): Json<CalendarRequest>,
) -> Result<Json<CalendarReceipt>, ApiError> {
    let actor = owner(&product, &headers).await?;
    Ok(Json(execute(&product.pool, &actor, request).await?))
}
#[utoipa::path(post,path="/v1/calendar/imports",operation_id="calendar_import",request_body=CalendarImportRequest,responses((status=200,body=CalendarReceipt),(status=400,body=ErrorBody),(status=401,body=ErrorBody),(status=503,body=ErrorBody)))]
pub async fn import(
    State(product): State<Product>,
    headers: HeaderMap,
    Json(request): Json<CalendarImportRequest>,
) -> Result<Json<CalendarReceipt>, ApiError> {
    let actor = owner(&product, &headers).await?;
    Ok(Json(enqueue_import(&product.pool, &actor, request).await?))
}

pub(crate) async fn snapshot(
    pool: &PgPool,
    actor: &Actor,
    from: i64,
    to: i64,
) -> Result<CalendarSnapshot, ApiError> {
    let events = sqlx::query_as(
        "SELECT id,source,calendar,color,title,location,notes,starts_at,ends_at,all_day,revision FROM calendar_events
         WHERE workspace_id=$1 AND user_id=$2 AND ends_at>=$3 AND starts_at<$4 ORDER BY starts_at,ends_at,title,id",
    )
    .bind(&actor.workspace)
    .bind(&actor.id)
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await?;
    let last_import: Option<ActionRow> = sqlx::query_as(
        "SELECT id,kind,input,state,error,created_at,finished_at FROM durable_actions
         WHERE workspace_id=$1 AND actor_id=$2 AND kind='calendar_import' ORDER BY seq DESC LIMIT 1",
    )
    .bind(&actor.workspace)
    .bind(&actor.id)
    .fetch_optional(pool)
    .await?;
    Ok(CalendarSnapshot {
        events,
        last_import: last_import.as_ref().map(ActionRow::view),
    })
}

struct Draft {
    title: String,
    starts_at: i64,
    ends_at: i64,
    all_day: bool,
    location: Option<String>,
    notes: Option<String>,
}
fn draft(
    title: String,
    starts_at: i64,
    ends_at: i64,
    all_day: bool,
    location: Option<String>,
    notes: Option<String>,
) -> Result<Draft, ApiError> {
    let title = clip(&title, 500);
    if title.is_empty() || ends_at < starts_at || ends_at - starts_at > MAX_SPAN {
        return Err(invalid(
            "Give the event a title and an end at or after its start, within 400 days.",
        ));
    }
    Ok(Draft {
        title,
        starts_at,
        ends_at,
        all_day,
        location: optional(location, 500),
        notes: optional(notes, 8000),
    })
}

pub(crate) async fn execute(
    pool: &PgPool,
    actor: &Actor,
    request: CalendarRequest,
) -> Result<CalendarReceipt, ApiError> {
    if uuid::Uuid::parse_str(&request.operation_id).is_err() {
        return Err(invalid("Use a UUID operation ID."));
    }
    let payload = json!(request.command);
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!(
            "calendar/{}/{}",
            actor.workspace, request.operation_id
        ))
        .execute(&mut *tx)
        .await?;
    let prior: Option<(Value, String)> = sqlx::query_as(
        "SELECT command,result_id FROM calendar_receipts WHERE workspace_id=$1 AND actor_id=$2 AND operation_id=$3",
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
        return Ok(CalendarReceipt { result_id });
    }
    // Only AgentInc's own events change here; mirrors follow their calendar.
    let editable = |id: &str| {
        sqlx::query_scalar::<_, EventSource>(
            "SELECT source FROM calendar_events WHERE id=$1 AND workspace_id=$2 AND user_id=$3",
        )
        .bind(id.to_owned())
        .bind(actor.workspace.clone())
        .bind(actor.id.clone())
    };
    let result_id = match request.command {
        CalendarCommand::Create {
            title,
            starts_at,
            ends_at,
            all_day,
            location,
            notes,
        } => {
            let d = draft(title, starts_at, ends_at, all_day, location, notes)?;
            let id = uuid::Uuid::new_v4().to_string();
            sqlx::query("INSERT INTO calendar_events(id,workspace_id,user_id,source,calendar,title,location,notes,starts_at,ends_at,all_day) VALUES($1,$2,$3,'agentinc','AgentInc',$4,$5,$6,$7,$8,$9)")
                .bind(&id)
                .bind(&actor.workspace)
                .bind(&actor.id)
                .bind(d.title)
                .bind(d.location)
                .bind(d.notes)
                .bind(d.starts_at)
                .bind(d.ends_at)
                .bind(d.all_day)
                .execute(&mut *tx)
                .await?;
            id
        }
        CalendarCommand::Update {
            id,
            revision,
            title,
            starts_at,
            ends_at,
            all_day,
            location,
            notes,
        } => {
            let d = draft(title, starts_at, ends_at, all_day, location, notes)?;
            match editable(&id).fetch_optional(&mut *tx).await? {
                Some(EventSource::Macos) => return Err(read_only()),
                None => return Err(ApiError::conflict()),
                Some(EventSource::Agentinc) => {}
            }
            let changed = sqlx::query("UPDATE calendar_events SET title=$4,location=$5,notes=$6,starts_at=$7,ends_at=$8,all_day=$9,revision=revision+1 WHERE id=$1 AND workspace_id=$2 AND revision=$3")
                .bind(&id)
                .bind(&actor.workspace)
                .bind(revision)
                .bind(d.title)
                .bind(d.location)
                .bind(d.notes)
                .bind(d.starts_at)
                .bind(d.ends_at)
                .bind(d.all_day)
                .execute(&mut *tx)
                .await?
                .rows_affected();
            if changed == 0 {
                return Err(ApiError::conflict());
            }
            id
        }
        CalendarCommand::Delete { id, revision } => {
            match editable(&id).fetch_optional(&mut *tx).await? {
                Some(EventSource::Macos) => return Err(read_only()),
                None => return Err(ApiError::conflict()),
                Some(EventSource::Agentinc) => {}
            }
            let changed = sqlx::query(
                "DELETE FROM calendar_events WHERE id=$1 AND workspace_id=$2 AND revision=$3",
            )
            .bind(&id)
            .bind(&actor.workspace)
            .bind(revision)
            .execute(&mut *tx)
            .await?
            .rows_affected();
            if changed == 0 {
                return Err(ApiError::conflict());
            }
            id
        }
    };
    sqlx::query("INSERT INTO calendar_receipts(workspace_id,actor_id,operation_id,command,result_id) VALUES($1,$2,$3,$4,$5)")
        .bind(&actor.workspace)
        .bind(&actor.id)
        .bind(&request.operation_id)
        .bind(&payload)
        .bind(&result_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(CalendarReceipt { result_id })
}
fn read_only() -> ApiError {
    invalid("Events from your Mac's calendars are read-only here. Edit them in Calendar.")
}

/// Normalize what the calendar store handed over, so applying it cannot fail a
/// check later.
fn normalize(mut request: CalendarImportRequest) -> Result<CalendarImportRequest, ApiError> {
    if request.window_end <= request.window_start
        || request.window_end - request.window_start > MAX_SPAN
        || request.events.len() > MAX_IMPORT
    {
        return Err(invalid(
            "Import a window of up to 400 days and 5000 events.",
        ));
    }
    for event in &mut request.events {
        event.external_id = clip(&event.external_id, 500);
        if event.external_id.is_empty() {
            return Err(invalid("Every imported event needs an external ID."));
        }
        event.calendar = Some(clip(&event.calendar, 120))
            .filter(|c| !c.is_empty())
            .unwrap_or_else(|| "Calendar".into());
        event.color = event.color.take().filter(|c| {
            c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|c| c.is_ascii_hexdigit())
        });
        event.title = Some(clip(&event.title, 500))
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| "Untitled".into());
        event.location = optional(event.location.take(), 500);
        event.notes = optional(event.notes.take(), 8000);
        event.ends_at = event
            .ends_at
            .clamp(event.starts_at, event.starts_at + MAX_SPAN);
    }
    request
        .events
        .sort_by(|a, b| a.external_id.cmp(&b.external_id));
    request
        .events
        .dedup_by(|a, b| a.external_id == b.external_id);
    Ok(request)
}

pub(crate) async fn enqueue_import(
    pool: &PgPool,
    actor: &Actor,
    request: CalendarImportRequest,
) -> Result<CalendarReceipt, ApiError> {
    let request = normalize(request)?;
    let input = json!(request);
    let digest = format!("{:x}", Sha256::digest(input.to_string().as_bytes()));
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("calendar-import/{}/{}", actor.workspace, actor.id))
        .execute(&mut *tx)
        .await?;
    // An unchanged calendar needs no new action.
    let latest: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT id,digest FROM durable_actions WHERE workspace_id=$1 AND actor_id=$2 AND kind='calendar_import' AND state<>'failed' ORDER BY seq DESC LIMIT 1",
    )
    .bind(&actor.workspace)
    .bind(&actor.id)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some((id, Some(previous))) = latest
        && previous == digest
    {
        return Ok(CalendarReceipt { result_id: id });
    }
    let result_id = durable::enqueue(
        &mut tx,
        actor,
        ActionKind::CalendarImport,
        &input,
        Some(&digest),
    )
    .await?;
    tx.commit().await?;
    Ok(CalendarReceipt { result_id })
}

/// Mirror one import. Upserts by external ID and removals within the window
/// make a retry harmless; an older import never overwrites a newer one.
pub(crate) async fn apply_import(pool: &PgPool, action_id: &str) -> anyhow::Result<()> {
    let (workspace, user, seq, input): (String, String, i64, Value) =
        sqlx::query_as("SELECT workspace_id,actor_id,seq,input FROM durable_actions WHERE id=$1")
            .bind(action_id)
            .fetch_one(pool)
            .await?;
    let request: CalendarImportRequest = serde_json::from_value(input)?;
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("calendar-import/{workspace}/{user}"))
        .execute(&mut *tx)
        .await?;
    let superseded: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM durable_actions WHERE workspace_id=$1 AND actor_id=$2 AND kind='calendar_import' AND state='completed' AND seq>$3)",
    )
    .bind(&workspace)
    .bind(&user)
    .bind(seq)
    .fetch_one(&mut *tx)
    .await?;
    if superseded {
        return Ok(());
    }
    for event in &request.events {
        sqlx::query(
            "INSERT INTO calendar_events(id,workspace_id,user_id,source,external_id,calendar,color,title,location,notes,starts_at,ends_at,all_day)
             VALUES($1,$2,$3,'macos',$4,$5,$6,$7,$8,$9,$10,$11,$12)
             ON CONFLICT(workspace_id,user_id,source,external_id) DO UPDATE SET
               calendar=excluded.calendar,color=excluded.color,title=excluded.title,location=excluded.location,
               notes=excluded.notes,starts_at=excluded.starts_at,ends_at=excluded.ends_at,all_day=excluded.all_day,
               revision=calendar_events.revision+1
             WHERE (calendar_events.calendar,calendar_events.color,calendar_events.title,calendar_events.location,calendar_events.notes,
                    calendar_events.starts_at,calendar_events.ends_at,calendar_events.all_day)
               IS DISTINCT FROM (excluded.calendar,excluded.color,excluded.title,excluded.location,excluded.notes,
                    excluded.starts_at,excluded.ends_at,excluded.all_day)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&workspace)
        .bind(&user)
        .bind(&event.external_id)
        .bind(&event.calendar)
        .bind(&event.color)
        .bind(&event.title)
        .bind(&event.location)
        .bind(&event.notes)
        .bind(event.starts_at)
        .bind(event.ends_at)
        .bind(event.all_day)
        .execute(&mut *tx)
        .await?;
    }
    let kept: Vec<&str> = request
        .events
        .iter()
        .map(|e| e.external_id.as_str())
        .collect();
    sqlx::query(
        "DELETE FROM calendar_events WHERE workspace_id=$1 AND user_id=$2 AND source='macos'
         AND starts_at>=$3 AND starts_at<$4 AND NOT (external_id = ANY($5))",
    )
    .bind(&workspace)
    .bind(&user)
    .bind(request.window_start)
    .bind(request.window_end)
    .bind(&kept)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}
