#![allow(deprecated)] // The Todo commands stay callable until the release after their deprecation.
use ainc_daemon::{
    legacy,
    product::{self, Command, CommandRequest, Product},
};
use sqlx::PgPool;

async fn execute(
    pool: &PgPool,
    request: CommandRequest,
) -> Result<product::CommandReceipt, ainc_daemon::api::CommandError> {
    product::execute_in(pool, "local", request).await
}
async fn snapshot(pool: &PgPool) -> Result<product::Snapshot, sqlx::Error> {
    product::snapshot_in(pool, "local").await
}

fn request(command: Command) -> CommandRequest {
    CommandRequest {
        operation_id: uuid::Uuid::new_v4().to_string(),
        command,
    }
}
async fn apply(pool: &PgPool, command: Command) -> i64 {
    execute(pool, request(command))
        .await
        .unwrap()
        .result_id
        .unwrap()
}

#[sqlx::test]
async fn commands_are_durable_repeat_safe_and_validate_conflicts(pool: PgPool) {
    let command = request(Command::CreateTodo {
        title: "Café 👋".into(),
    });
    let ack = execute(&pool, command.clone()).await.unwrap();
    assert_eq!(
        execute(&pool, command.clone()).await.unwrap().result_id,
        ack.result_id
    );
    let mut conflicting = command;
    conflicting.command = Command::CreateTodo {
        title: "different".into(),
    };
    assert!(execute(&pool, conflicting).await.is_err());
    assert!(
        execute(&pool, request(Command::CreateTodo { title: "  ".into() }))
            .await
            .is_err()
    );
    let state = snapshot(&pool).await.unwrap();
    assert_eq!(state.todos.len(), 1);
    assert_eq!(state.todos[0].title, "Café 👋");
    apply(
        &pool,
        Command::CompleteTodo {
            id: ack.result_id.unwrap(),
            completed: true,
        },
    )
    .await;
    assert!(snapshot(&pool).await.unwrap().todos[0].completed);
}

#[sqlx::test]
async fn closing_client_does_not_drop_acknowledged_turn_or_completed_reply(pool: PgPool) {
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;
    let conversation = apply(&pool, Command::CreateConversation).await;
    let app = ainc_daemon::product_router(Product::new(pool.clone(), "fixture".into()).unwrap());
    let command = request(Command::Send {
        conversation_id: conversation,
        prompt: "Keep working after close".into(),
    });
    let response = app
        .clone()
        .oneshot(
            Request::post("/v1/commands")
                .header(ainc_release::CLIENT_HEADER, ainc_release::client_header())
                .header("authorization", "Bearer fixture")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&command).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    drop(response);
    drop(app); // All HTTP/window ownership is gone; the daemon keeps the record.
    let state = snapshot(&pool).await.unwrap();
    assert_eq!(state.turns[0].state, "queued");
    let id = state.turns[0].id;
    sqlx::query("UPDATE turns SET state='running' WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE turns SET state='completed',response='Saved without a window' WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    let state = snapshot(&pool).await.unwrap();
    assert_eq!(
        state.turns[0].response.as_deref(),
        Some("Saved without a window")
    );
    assert_eq!(state.turns[0].state, "completed");
    assert_eq!(execute(&pool, command).await.unwrap().result_id, Some(id));
    assert_eq!(snapshot(&pool).await.unwrap().turns.len(), 1);
}

#[sqlx::test]
async fn one_pending_turn_per_conversation_and_no_delete_while_running(pool: PgPool) {
    let first = apply(&pool, Command::CreateConversation).await;
    let second = apply(&pool, Command::CreateConversation).await;
    apply(
        &pool,
        Command::Send {
            conversation_id: first,
            prompt: "first".into(),
        },
    )
    .await;
    assert!(
        execute(
            &pool,
            request(Command::Send {
                conversation_id: first,
                prompt: "overlap".into()
            })
        )
        .await
        .is_err()
    );
    assert!(
        execute(&pool, request(Command::DeleteConversation { id: first }))
            .await
            .is_err()
    );
    apply(
        &pool,
        Command::Send {
            conversation_id: second,
            prompt: "independent".into(),
        },
    )
    .await;
    apply(&pool, Command::SelectConversation { id: second }).await;
    assert_eq!(
        snapshot(&pool)
            .await
            .unwrap()
            .settings
            .selected_conversation,
        Some(second)
    );
}

#[sqlx::test]
async fn owner_credential_required_for_reads_and_writes(pool: PgPool) {
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;
    let app = ainc_daemon::product_router(Product::new(pool, "fixture".into()).unwrap());
    let response = app
        .oneshot(
            Request::get("/v1/state")
                .header(ainc_release::CLIENT_HEADER, ainc_release::client_header())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
}

fn legacy_fixture(path: &std::path::Path) {
    let db = rusqlite::Connection::open(path.join("assistant.sqlite3")).unwrap();
    db.execute_batch("CREATE TABLE todos(id INTEGER PRIMARY KEY,title TEXT,completed INTEGER); INSERT INTO todos VALUES(9,'legacy task',1); CREATE TABLE conversations(id INTEGER PRIMARY KEY,title TEXT,updated_at INTEGER); INSERT INTO conversations VALUES(8,'Old conversation',1700000000); CREATE TABLE turns(id INTEGER PRIMARY KEY,conversation_id INTEGER,prompt TEXT,response TEXT,error TEXT); INSERT INTO turns VALUES(4,8,'hello','world',NULL),(5,8,'unfinished',NULL,NULL); CREATE TABLE assistant_settings(key TEXT PRIMARY KEY,value TEXT); INSERT INTO assistant_settings VALUES('model','example'); PRAGMA user_version=2;").unwrap();
    std::fs::write(
        path.join("session.json"),
        r#"{"tabs":["tasks","evee"],"font":"helvetica_neue","sidebar":false}"#,
    )
    .unwrap();
}

#[sqlx::test]
async fn import_preserves_order_preferences_and_sources_without_replaying(pool: PgPool) {
    // Synthetic fixture only. No test resolves the user's Application Support.
    let directory = tempfile::tempdir().unwrap();
    legacy_fixture(directory.path());
    let original = std::fs::read(directory.path().join("assistant.sqlite3")).unwrap();
    let session = std::fs::read(directory.path().join("session.json")).unwrap();
    assert!(legacy::import(&pool, directory.path()).await.unwrap());
    assert!(!legacy::import(&pool, directory.path()).await.unwrap());
    assert_eq!(
        std::fs::read(directory.path().join("assistant.sqlite3")).unwrap(),
        original
    );
    assert_eq!(
        std::fs::read(directory.path().join("session.json")).unwrap(),
        session
    );
    let state = snapshot(&pool).await.unwrap();
    assert_eq!(state.todos[0].id, 9);
    assert!(state.todos[0].completed);
    assert_eq!(state.conversations[0].updated_at, 1700000000);
    assert_eq!(state.turns.iter().map(|t| t.id).collect::<Vec<_>>(), [4, 5]);
    assert_eq!(state.turns[0].response.as_deref(), Some("world"));
    assert_eq!(state.turns[1].state, "failed");
    assert!(state.turns[1].error.is_some());
    assert_eq!(state.settings.model.as_deref(), Some("example"));
    let new = apply(&pool, Command::CreateConversation).await;
    assert!(new > 8);
    apply(&pool, Command::DeleteConversation { id: 8 }).await;
    assert!(!legacy::import(&pool, directory.path()).await.unwrap());
    assert_eq!(snapshot(&pool).await.unwrap().conversations.len(), 1);
}

#[sqlx::test]
async fn failed_import_rolls_back_and_future_schema_is_untouched(pool: PgPool) {
    let directory = tempfile::tempdir().unwrap();
    legacy_fixture(directory.path());
    let db = rusqlite::Connection::open(directory.path().join("assistant.sqlite3")).unwrap();
    db.execute("UPDATE todos SET title=''", []).unwrap();
    assert!(legacy::import(&pool, directory.path()).await.is_err());
    assert!(snapshot(&pool).await.unwrap().conversations.is_empty());
    db.execute("UPDATE todos SET title='fixed fixture'", [])
        .unwrap();
    db.execute_batch("PRAGMA user_version=3;").unwrap();
    let before = std::fs::read(directory.path().join("assistant.sqlite3")).unwrap();
    assert!(legacy::import(&pool, directory.path()).await.is_err());
    assert_eq!(
        std::fs::read(directory.path().join("assistant.sqlite3")).unwrap(),
        before
    );
    db.execute_batch("PRAGMA user_version=2;").unwrap();
    assert!(legacy::import(&pool, directory.path()).await.unwrap());
}

mod todo_adapter {
    use super::*;
    use ainc_daemon::tickets::{
        self, ActivityKind, LinkKind, TicketCommand, TicketCommandRequest, TicketStatus,
    };
    use axum::{
        Router,
        body::{Body, to_bytes},
        http::Request as HttpRequest,
    };
    use tower::ServiceExt;

    fn app(pool: &PgPool) -> Router {
        ainc_daemon::product_router(Product::new(pool.clone(), "owner-fixture".into()).unwrap())
    }
    async fn ticket(app: &Router, command: TicketCommand) -> i64 {
        let request = TicketCommandRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            command,
        };
        let response = app
            .clone()
            .oneshot(
                HttpRequest::post("/v1/tickets/commands")
                    .header(ainc_release::CLIENT_HEADER, ainc_release::client_header())
                    .header("authorization", "Bearer owner-fixture")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let body: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 1 << 16).await.unwrap())
                .unwrap();
        body["result_id"].as_i64().unwrap_or_default()
    }
    /// Every column of a Ticket row that does not name the row itself.
    async fn row(
        pool: &PgPool,
        id: i64,
    ) -> (String, String, Vec<String>, String, String, i64, i64) {
        sqlx::query_as("SELECT status,priority,labels,assignee_kind,assignee_id,revision,generation FROM tickets WHERE id=$1")
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap()
    }
    async fn history(app: &Router, id: i64) -> Vec<(ActivityKind, Option<String>, Option<String>)> {
        let response = app
            .clone()
            .oneshot(
                HttpRequest::get(format!("/v1/tickets/{id}/activity"))
                    .header(ainc_release::CLIENT_HEADER, ainc_release::client_header())
                    .header("authorization", "Bearer owner-fixture")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let entries: Vec<tickets::TicketActivity> =
            serde_json::from_slice(&to_bytes(response.into_body(), 1 << 20).await.unwrap())
                .unwrap();
        entries
            .into_iter()
            .map(|e| (e.kind, e.from_value, e.to_value))
            .collect()
    }

    #[sqlx::test]
    async fn todo_commands_write_the_same_rows_and_history_as_ticket_commands(pool: PgPool) {
        let app = app(&pool);
        let todo = apply(
            &pool,
            Command::CreateTodo {
                title: "Same".into(),
            },
        )
        .await;
        let full = ticket(
            &app,
            TicketCommand::Create {
                title: "Same".into(),
            },
        )
        .await;
        assert_eq!(row(&pool, todo).await, row(&pool, full).await);
        // Both land at the top of To do, newest first.
        let column: Vec<i64> = sqlx::query_scalar(
            "SELECT id FROM tickets WHERE status='to_do' ORDER BY position,id DESC",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(column, [full, todo]);
        apply(
            &pool,
            Command::CompleteTodo {
                id: todo,
                completed: true,
            },
        )
        .await;
        ticket(
            &app,
            TicketCommand::SetStatus {
                id: full,
                revision: 0,
                status: TicketStatus::Done,
            },
        )
        .await;
        assert_eq!(row(&pool, todo).await, row(&pool, full).await);
        assert_eq!(history(&app, todo).await, history(&app, full).await);
        assert_eq!(
            history(&app, todo).await,
            [
                (ActivityKind::Created, None, Some("Same".into())),
                (
                    ActivityKind::Status,
                    Some("to_do".into()),
                    Some("done".into())
                ),
            ]
        );
        // A stale or missing Todo is a conflict, as it was before.
        assert!(
            execute(
                &pool,
                request(Command::CompleteTodo {
                    id: todo + 100,
                    completed: true
                })
            )
            .await
            .is_err()
        );
    }

    #[sqlx::test]
    async fn deleting_a_todo_records_the_lost_relationship_on_the_other_ticket(pool: PgPool) {
        let app = app(&pool);
        let kept = apply(
            &pool,
            Command::CreateTodo {
                title: "Kept".into(),
            },
        )
        .await;
        let gone = apply(
            &pool,
            Command::CreateTodo {
                title: "Gone".into(),
            },
        )
        .await;
        ticket(
            &app,
            TicketCommand::Link {
                from_id: gone,
                to_id: kept,
                link: LinkKind::Blocks,
            },
        )
        .await;
        apply(&pool, Command::DeleteTodo { id: gone }).await;
        assert_eq!(snapshot(&pool).await.unwrap().todos.len(), 1);
        let last = history(&app, kept).await.pop().unwrap();
        assert_eq!(
            last,
            (
                ActivityKind::Unlinked,
                Some("blocked_by".into()),
                Some(gone.to_string())
            )
        );
    }
}

#[sqlx::test]
async fn conversation_resource_endpoints_share_state_with_the_deprecated_ones(pool: PgPool) {
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;
    let app = ainc_daemon::product_router(Product::new(pool.clone(), "fixture".into()).unwrap());
    let send = |request: Request<Body>| {
        let app = app.clone();
        async move {
            let response = app.oneshot(request).await.unwrap();
            let status = response.status();
            let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            (
                status,
                serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
            )
        }
    };
    let authorized = |builder: axum::http::request::Builder| {
        builder
            .header(ainc_release::CLIENT_HEADER, ainc_release::client_header())
            .header("authorization", "Bearer fixture")
            .header("content-type", "application/json")
    };
    let (status, receipt) = send(
        authorized(Request::post("/v1/conversations/commands"))
            .body(Body::from(
                serde_json::json!({
                    "operation_id": uuid::Uuid::new_v4().to_string(),
                    "command": {"kind": "create"},
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(status, 200);
    let id = receipt["result_id"].as_i64().unwrap();
    let (status, snapshot) = send(
        authorized(Request::get("/v1/conversations"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(snapshot["conversations"][0]["id"], id);
    assert!(snapshot["conversations"][0].get("updated").is_none());
    // The deprecated snapshot shows the same Conversation.
    assert_eq!(snapshot_of_state(&pool).await, vec![id]);
    let api = ainc_daemon::openapi();
    assert_eq!(api["paths"]["/v1/state"]["get"]["deprecated"], true);
    assert_eq!(api["paths"]["/v1/commands"]["post"]["deprecated"], true);
    assert!(api["paths"]["/v1/conversations"]["get"]["deprecated"].is_null());
}
async fn snapshot_of_state(pool: &PgPool) -> Vec<i64> {
    snapshot(pool)
        .await
        .unwrap()
        .conversations
        .iter()
        .map(|c| c.id)
        .collect()
}
