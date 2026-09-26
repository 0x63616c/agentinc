//! Smart Home and Calendar through the HTTP API, with changes applied by the
//! real durable-action runner on a Temporal test server and a fake control
//! center reached over real HTTP.
use ainc_daemon::{
    durable::{Effects, Runner},
    home::Home,
    product::Product,
    secrets::{MemoryStore, SecretStore},
};
use axum::{
    Json, Router,
    body::{Body, to_bytes},
    extract::State,
    http::{HeaderMap, Method, Request, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde_json::{Value, json};
use sqlx::{PgPool, postgres::PgListener};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

/// The control center's side of the wire: group state, the thermostat, and
/// every mutation it received with the Access headers that came with it.
#[derive(Default)]
struct Fake {
    groups: serde_json::Map<String, Value>,
    climate: Value,
    calls: Vec<(String, Value, Option<String>)>,
}
type Shared = Arc<Mutex<Fake>>;
fn data(value: Value) -> Response {
    Json(json!({"result": {"data": value}})).into_response()
}
fn access(headers: &HeaderMap) -> Option<String> {
    headers
        .get("cf-access-client-secret")
        .and_then(|h| h.to_str().ok())
        .map(str::to_owned)
}
async fn control_center() -> (String, Shared) {
    let fake: Shared = Arc::default();
    {
        let mut state = fake.lock().unwrap();
        for key in [
            "all",
            "lamps",
            "bedroomLamps",
            "otherLamps",
            "ceiling",
            "cabinet",
        ] {
            state
                .groups
                .insert(key.into(), json!({"on": false, "pending": false}));
        }
        state.climate = json!({"mode":"cool","ambient":71.5,"action":"Cooling","target":72});
    }
    async fn list(State(fake): State<Shared>, headers: HeaderMap) -> Response {
        if access(&headers).as_deref() != Some("fixture-secret") {
            return StatusCode::FOUND.into_response();
        }
        data(Value::Object(fake.lock().unwrap().groups.clone()))
    }
    async fn climate(State(fake): State<Shared>) -> Response {
        data(fake.lock().unwrap().climate.clone())
    }
    async fn mutate(
        State(fake): State<Shared>,
        uri: axum::http::Uri,
        headers: HeaderMap,
        Json(input): Json<Value>,
    ) -> Response {
        let procedure = uri.path().trim_start_matches("/trpc/").to_owned();
        let mut state = fake.lock().unwrap();
        state
            .calls
            .push((procedure.clone(), input.clone(), access(&headers)));
        match procedure.as_str() {
            "controls.toggle" => {
                let key = input["key"].as_str().unwrap().to_owned();
                state
                    .groups
                    .insert(key, json!({"on": input["on"], "pending": false}));
            }
            "climate.setMode" => state.climate["mode"] = input,
            "climate.setTarget" => state.climate["target"] = input,
            _ => {}
        }
        data(json!({}))
    }
    let app = Router::new()
        .route("/trpc/controls.list", get(list))
        .route("/trpc/climate.get", get(climate))
        .route("/trpc/controls.toggle", post(mutate))
        .route("/trpc/climate.setMode", post(mutate))
        .route("/trpc/climate.setTarget", post(mutate))
        .with_state(fake.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, fake)
}

async fn call(
    app: &Router,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header("agent-inc-client", ainc_release::client_header())
        .header("authorization", "Bearer owner-fixture")
        .header("content-type", "application/json")
        .body(body.map_or_else(Body::empty, |b| Body::from(b.to_string())))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
fn operation(command: Value) -> Value {
    json!({"operation_id": uuid::Uuid::new_v4().to_string(), "command": command})
}
/// Wait, on the action channel rather than a timer, until `done` holds.
async fn until(listener: &mut PgListener, mut done: impl AsyncFnMut() -> bool) {
    while !done().await {
        listener.recv().await.unwrap();
    }
}

#[sqlx::test]
async fn home_and_calendar_changes_run_as_durable_actions(pool: PgPool) {
    let (url, fake) = control_center().await;
    let secrets = Arc::new(MemoryStore::default());
    let home = Home::new(secrets.clone());
    let product = Product::new(pool.clone(), "owner-fixture".into()).unwrap();
    let app = ainc_daemon::product_router(product.clone())
        .merge(ainc_daemon::home::router(product, home.clone()));

    // Unconnected: every switch is listed, nothing is reachable, commands wait.
    let (status, state) = call(&app, Method::GET, "/v1/home", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(state["connection"].is_null());
    assert_eq!(state["switches"].as_array().unwrap().len(), 6);
    let switch = operation(json!({"kind":"switch","key":"bedroom_lamps","on":true}));
    let (status, body) = call(
        &app,
        Method::POST,
        "/v1/home/commands",
        Some(switch.clone()),
    )
    .await;
    assert_eq!(
        (status, body["code"].as_str()),
        (StatusCode::CONFLICT, Some("not_connected"))
    );

    // Half an Access token is refused; a whole one lands in the Keychain only.
    let (status, _) = call(
        &app,
        Method::PUT,
        "/v1/home/connection",
        Some(json!({"base_url": url, "access_client_id": "fixture-id"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, connection) = call(&app, Method::PUT, "/v1/home/connection",
        Some(json!({"base_url": format!("{url}/"), "access_client_id": "fixture-id", "access_client_secret": "fixture-secret"}))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(connection, json!({"base_url": url, "access_token": true}));
    assert!(!connection.to_string().contains("fixture-secret"));
    let stored: Vec<String> =
        sqlx::query_scalar("SELECT row_to_json(h)::text FROM home_connections h")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(stored.iter().all(|row| !row.contains("fixture")));
    assert!(
        secrets
            .get("control-center/local/access-client-secret")
            .unwrap()
            .is_some()
    );

    let (_, state) = call(&app, Method::GET, "/v1/home", None).await;
    assert_eq!(state["reachable"], true, "{state}");
    assert_eq!(state["climate"]["ambient"], 71.5);
    assert_eq!(state["climate"]["mode"], "cool");

    // Invalid setpoints never become actions.
    let (status, _) = call(
        &app,
        Method::POST,
        "/v1/home/commands",
        Some(operation(json!({"kind":"set_climate_target","target":90}))),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Accepted commands are durable before any effect, and their receipts replay.
    let (status, first) = call(
        &app,
        Method::POST,
        "/v1/home/commands",
        Some(switch.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let (_, again) = call(
        &app,
        Method::POST,
        "/v1/home/commands",
        Some(switch.clone()),
    )
    .await;
    assert_eq!(first, again);
    let mut changed = switch.clone();
    changed["command"]["on"] = json!(false);
    let (status, _) = call(&app, Method::POST, "/v1/home/commands", Some(changed)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (_, state) = call(&app, Method::GET, "/v1/home", None).await;
    let bedroom = state["switches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["key"] == "bedroom_lamps")
        .unwrap()
        .clone();
    assert_eq!(
        (bedroom["on"].clone(), bedroom["pending"].clone()),
        (json!(true), json!(true))
    );
    assert_eq!(state["actions"][0]["summary"], "Bedroom lamps on");

    // The real runner applies it through Temporal.
    let mut listener = PgListener::connect_with(&pool).await.unwrap();
    listener.listen("agentinc_actions").await.unwrap();
    let server = turnkeel::testing::Server::start().await.unwrap();
    let runner = Runner::start(
        Effects {
            pool: pool.clone(),
            home,
        },
        server.config(),
    )
    .await
    .unwrap();
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let worker = tokio::spawn(runner.run_until(async {
        let _ = stopped.await;
    }));
    until(&mut listener, async || {
        call(&app, Method::GET, "/v1/home", None).await.1["actions"][0]["state"] == "completed"
    })
    .await;
    let calls = fake.lock().unwrap().calls.clone();
    assert_eq!(
        calls,
        vec![(
            "controls.toggle".to_owned(),
            json!({"key":"bedroomLamps","on":true}),
            Some("fixture-secret".to_owned()),
        )]
    );
    let (_, state) = call(&app, Method::GET, "/v1/home", None).await;
    let bedroom = state["switches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["key"] == "bedroom_lamps")
        .unwrap()
        .clone();
    assert_eq!(
        (bedroom["on"].clone(), bedroom["pending"].clone()),
        (json!(true), json!(false))
    );

    let (status, _) = call(
        &app,
        Method::POST,
        "/v1/home/commands",
        Some(operation(
            json!({"kind":"set_climate_mode","mode":"heat_cool"}),
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    until(&mut listener, async || {
        call(&app, Method::GET, "/v1/home", None).await.1["actions"][0]["state"] == "completed"
    })
    .await;
    assert_eq!(fake.lock().unwrap().calls[1].1, json!("heat_cool"));

    // A calendar import is a durable action too; an identical one is not repeated.
    let import = json!({"window_start": 1_000_000, "window_end": 2_000_000, "events": [
        {"external_id":"dentist@1500000","calendar":"Home","color":"#FF9500","title":"Dentist",
         "location":null,"notes":null,"starts_at":1_500_000,"ends_at":1_503_600,"all_day":false}
    ]});
    let (status, receipt) = call(
        &app,
        Method::POST,
        "/v1/calendar/imports",
        Some(import.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    let (_, repeat) = call(&app, Method::POST, "/v1/calendar/imports", Some(import)).await;
    assert_eq!(receipt, repeat);
    let path = "/v1/calendar?from=1000000&to=2000000";
    until(&mut listener, async || {
        call(&app, Method::GET, path, None).await.1["last_import"]["state"] == "completed"
    })
    .await;
    let (_, calendar) = call(&app, Method::GET, path, None).await;
    assert_eq!(calendar["events"][0]["title"], "Dentist");
    assert_eq!(calendar["events"][0]["source"], "macos");
    assert_eq!(calendar["last_import"]["summary"], "Import 1 event");

    let _ = stop.send(());
    worker.await.unwrap().unwrap();
    server.shutdown().await.unwrap();
}

#[sqlx::test]
async fn calendar_events_are_revisioned_and_mirrors_are_read_only(pool: PgPool) {
    let product = Product::new(pool.clone(), "owner-fixture".into()).unwrap();
    let app = ainc_daemon::product_router(product);
    let (status, _) = call(&app, Method::POST, "/v1/calendar/commands", Some(operation(json!({
        "kind":"create","title":"  ","starts_at":10,"ends_at":20,"all_day":false,"location":null,"notes":null
    })))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let create = operation(json!({
        "kind":"create","title":"Plan the week","starts_at":1_700_000_000,"ends_at":1_700_003_600,
        "all_day":false,"location":" Studio ","notes":""
    }));
    let (status, receipt) = call(
        &app,
        Method::POST,
        "/v1/calendar/commands",
        Some(create.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, replay) = call(&app, Method::POST, "/v1/calendar/commands", Some(create)).await;
    assert_eq!(receipt, replay);
    let id = receipt["result_id"].as_str().unwrap();
    let range = "/v1/calendar?from=1699990000&to=1700100000";
    let (_, calendar) = call(&app, Method::GET, range, None).await;
    let event = &calendar["events"][0];
    assert_eq!(
        (event["location"].clone(), event["notes"].clone()),
        (json!("Studio"), Value::Null)
    );
    assert_eq!(
        (event["source"].clone(), event["revision"].clone()),
        (json!("agentinc"), json!(0))
    );

    let update = |revision: i64| {
        operation(json!({
            "kind":"update","id":id,"revision":revision,"title":"Plan the month","starts_at":1_700_000_000,
            "ends_at":1_700_007_200,"all_day":false,"location":null,"notes":"Bring the roadmap"
        }))
    };
    let (status, _) = call(&app, Method::POST, "/v1/calendar/commands", Some(update(0))).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(&app, Method::POST, "/v1/calendar/commands", Some(update(0))).await;
    assert_eq!(status, StatusCode::CONFLICT, "a stale revision is refused");
    let (_, calendar) = call(&app, Method::GET, range, None).await;
    assert_eq!(calendar["events"][0]["title"], "Plan the month");

    // Another workspace sees none of it.
    sqlx::query("INSERT INTO workspaces(id,name) VALUES('other','Other')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE selected_workspace SET workspace_id='other'")
        .execute(&pool)
        .await
        .unwrap();
    let (_, other) = call(&app, Method::GET, range, None).await;
    assert_eq!(other["events"], json!([]));
    let (status, _) = call(
        &app,
        Method::POST,
        "/v1/calendar/commands",
        Some(operation(json!({"kind":"delete","id":id,"revision":1}))),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    sqlx::query("UPDATE selected_workspace SET workspace_id='local'")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO calendar_events(id,workspace_id,user_id,source,external_id,calendar,title,starts_at,ends_at) VALUES('mirror','local','owner','macos','x@1','Home','Mirrored',1700000000,1700000100)")
        .execute(&pool).await.unwrap();
    let (status, body) = call(
        &app,
        Method::POST,
        "/v1/calendar/commands",
        Some(operation(
            json!({"kind":"delete","id":"mirror","revision":0}),
        )),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["message"].as_str().unwrap().contains("read-only"));
    let (status, _) = call(
        &app,
        Method::POST,
        "/v1/calendar/commands",
        Some(operation(json!({"kind":"delete","id":id,"revision":1}))),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, calendar) = call(&app, Method::GET, range, None).await;
    assert_eq!(calendar["events"].as_array().unwrap().len(), 1);
}
