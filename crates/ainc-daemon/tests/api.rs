//! The HTTP seam every module shares: one error shape, one status per failure,
//! the shared layers applied once.
use ainc_daemon::{
    product::{Command, CommandRequest, Product},
    tickets::{TicketCommand, TicketCommandRequest},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;

fn app(pool: &PgPool) -> Router {
    ainc_daemon::product_router(Product::new(pool.clone(), "owner-fixture".into()).unwrap())
}

async fn send(app: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    assert_eq!(
        response
            .headers()
            .get_all(ainc_release::SERVER_HEADER)
            .iter()
            .count(),
        1,
        "the version header is applied exactly once"
    );
    let body = to_bytes(response.into_body(), 1 << 20).await.unwrap();
    let body = if body.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&body).unwrap()
    };
    (status, body)
}

fn get(path: &str, token: Option<&str>) -> Request<Body> {
    let mut request =
        Request::get(path).header(ainc_release::CLIENT_HEADER, ainc_release::client_header());
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    request.body(Body::empty()).unwrap()
}

fn post(path: &str, token: &str, body: &impl serde::Serialize) -> Request<Body> {
    Request::post(path)
        .header(ainc_release::CLIENT_HEADER, ainc_release::client_header())
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(body).unwrap()))
        .unwrap()
}

#[sqlx::test]
async fn an_unknown_credential_is_unauthorized_on_every_endpoint(pool: PgPool) {
    let app = app(&pool);
    for path in [
        "/v1/state",
        "/v1/tickets",
        "/v1/tickets/1/activity",
        "/v1/automations",
        "/v1/workspaces",
        "/v1/terminal/sessions",
        "/v1/connection",
    ] {
        for token in [None, Some("wrong-token")] {
            let (status, body) = send(&app, get(path, token)).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{path} {token:?}");
            assert_eq!(body["code"], "unauthorized", "{path}");
            assert_eq!(body["message"], "Owner credential required.");
        }
    }
}

#[sqlx::test]
async fn a_missing_record_is_not_found_everywhere(pool: PgPool) {
    let app = app(&pool);
    let ticket = TicketCommandRequest {
        operation_id: uuid::Uuid::new_v4().to_string(),
        command: TicketCommand::Rename {
            id: 404,
            revision: 0,
            title: "gone".into(),
        },
    };
    let (status, body) = send(&app, post("/v1/tickets/commands", "owner-fixture", &ticket)).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(body["code"], "not_found");
    let product = CommandRequest {
        operation_id: uuid::Uuid::new_v4().to_string(),
        command: Command::SelectConversation { id: 404 },
    };
    let (status, body) = send(&app, post("/v1/commands", "owner-fixture", &product)).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(body["code"], "not_found");
    let workspace = serde_json::json!({"operation_id": uuid::Uuid::new_v4().to_string(), "command": {"kind": "switch", "id": "nowhere"}});
    let (status, body) = send(
        &app,
        post("/v1/workspaces/commands", "owner-fixture", &workspace),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, body) = send(
        &app,
        Request::delete(format!("/v1/terminal/sessions/{}", uuid::Uuid::new_v4()))
            .header(ainc_release::CLIENT_HEADER, ainc_release::client_header())
            .header("authorization", "Bearer owner-fixture")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(body["code"], "not_found");
}

#[sqlx::test]
async fn an_old_client_gets_the_shared_error_body(pool: PgPool) {
    let app = app(&pool);
    let (status, body) = send(
        &app,
        Request::get("/v1/state")
            .header(ainc_release::CLIENT_HEADER, "mac/0.0.1 (api 1)")
            .header("authorization", "Bearer owner-fixture")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::UPGRADE_REQUIRED);
    assert_eq!(
        body,
        serde_json::json!({"code": "upgrade_required", "message": "Update to continue"})
    );
    // Unversioned endpoints need no client header.
    let (status, body) = send(&app, Request::get("/version").body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["version"], ainc_release::VERSION);
}

#[tokio::test]
async fn readiness_reports_an_unreachable_database_as_an_error_body() {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_millis(500))
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .unwrap();
    let app = app(&pool);
    let (status, body) = send(
        &app,
        Request::get("/health/ready").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["code"], "unavailable");
    assert!(body["message"].is_string());
}
