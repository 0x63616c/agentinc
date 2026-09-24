use ainc_daemon::{
    product::Product,
    tickets::{
        AssigneeKind, TicketCommand as Command, TicketCommandRequest as Request, TicketReceipt,
        TicketSnapshot, TicketStatus,
    },
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request as HttpRequest, StatusCode},
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tower::ServiceExt;

fn app(pool: &PgPool) -> Router {
    ainc_daemon::product_router(Product::new(pool.clone(), "owner-fixture".into()).unwrap())
}
fn request(command: Command) -> Request {
    Request {
        operation_id: uuid::Uuid::new_v4().to_string(),
        command,
    }
}
async fn command(app: &Router, token: &str, request: &Request) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            HttpRequest::post("/v1/tickets/commands")
                .header("agent-inc-client", ainc_release::client_header())
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
        .unwrap();
    (status, body)
}
async fn apply(app: &Router, cmd: Command) -> TicketReceipt {
    let (status, body) = command(app, "owner-fixture", &request(cmd)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    serde_json::from_value(body).unwrap()
}
async fn snapshot(app: &Router, token: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            HttpRequest::get("/v1/tickets")
                .header("agent-inc-client", ainc_release::client_header())
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    (
        status,
        serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
            .unwrap(),
    )
}
async fn state(app: &Router) -> TicketSnapshot {
    serde_json::from_value(snapshot(app, "owner-fixture").await.1).unwrap()
}
async fn agent(app: &Router) -> String {
    apply(
        app,
        Command::RegisterAgent {
            name: "Fixture agent".into(),
            instructions: "Use fixture tools".into(),
            model: "fixture".into(),
        },
    )
    .await;
    state(app)
        .await
        .assignees
        .into_iter()
        .find(|a| a.kind == AssigneeKind::Agent)
        .unwrap()
        .id
}
async fn assigned(pool: &PgPool) -> (Router, i64, String) {
    let app = app(pool);
    let agent = agent(&app).await;
    let id = apply(
        &app,
        Command::Create {
            title: "Produce inspectable work".into(),
        },
    )
    .await
    .result_id
    .unwrap();
    apply(
        &app,
        Command::Assign {
            id,
            revision: 0,
            assignee_kind: AssigneeKind::Agent,
            assignee_id: agent.clone(),
        },
    )
    .await;
    (app, id, agent)
}
async fn credential(pool: &PgPool, id: i64, token: &str) {
    sqlx::query("INSERT INTO agent_credentials(token_hash,run_id,workspace_id) SELECT $1,run_id,'local' FROM ticket_runs WHERE ticket_id=$2 ORDER BY generation DESC LIMIT 1").bind(format!("{:x}",Sha256::digest(token.as_bytes()))).bind(id).execute(pool).await.unwrap();
}

#[sqlx::test]
async fn assignment_and_outbox_are_atomic_repeat_safe_and_revision_checked(pool: PgPool) {
    let app = app(&pool);
    let agent = agent(&app).await;
    let id = apply(
        &app,
        Command::Create {
            title: "Café 👋".into(),
        },
    )
    .await
    .result_id
    .unwrap();
    let assign = request(Command::Assign {
        id,
        revision: 0,
        assignee_kind: AssigneeKind::Agent,
        assignee_id: agent,
    });
    let (one, two) = tokio::join!(
        command(&app, "owner-fixture", &assign),
        command(&app, "owner-fixture", &assign)
    );
    assert_eq!(one.0, StatusCode::OK);
    assert_eq!(one, two);
    let counts: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM dispatch_outbox),(SELECT count(*) FROM ticket_runs)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(counts, (1, 1));
    let stale = request(Command::Rename {
        id,
        revision: 0,
        title: "stale".into(),
    });
    assert_eq!(
        command(&app, "owner-fixture", &stale).await.0,
        StatusCode::CONFLICT
    );
    let invalid = request(Command::Assign {
        id,
        revision: 1,
        assignee_kind: AssigneeKind::Agent,
        assignee_id: "missing".into(),
    });
    assert_eq!(
        command(&app, "owner-fixture", &invalid).await.0,
        StatusCode::CONFLICT
    );
    let current = state(&app).await;
    assert_eq!(current.tickets[0].title, "Café 👋");
    assert_eq!(current.tickets[0].revision, 1);
    assert_eq!(current.runs[0].state, "queued");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM dispatch_outbox")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

#[sqlx::test]
async fn agent_scope_fences_comments_reassignment_and_cancelled_receipts(pool: PgPool) {
    let (app, id, agent) = assigned(&pool).await;
    credential(&pool, id, "agent-fixture").await;
    let other = apply(
        &app,
        Command::Create {
            title: "Outside the assignment".into(),
        },
    )
    .await
    .result_id
    .unwrap();
    let scoped: TicketSnapshot =
        serde_json::from_value(snapshot(&app, "agent-fixture").await.1).unwrap();
    assert_eq!(scoped.tickets.len(), 1);
    assert_eq!(scoped.tickets[0].id, id);
    let own = request(Command::AddComment {
        ticket_id: id,
        body: "Evidence from fixture".into(),
    });
    assert_eq!(command(&app, "agent-fixture", &own).await.0, StatusCode::OK);
    assert_eq!(command(&app, "agent-fixture", &own).await.0, StatusCode::OK);
    assert_eq!(
        command(
            &app,
            "agent-fixture",
            &request(Command::AddComment {
                ticket_id: other,
                body: "escape".into()
            })
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        command(
            &app,
            "agent-fixture",
            &request(Command::Assign {
                id,
                revision: 1,
                assignee_kind: AssigneeKind::Human,
                assignee_id: "owner".into()
            })
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    apply(
        &app,
        Command::Assign {
            id,
            revision: 1,
            assignee_kind: AssigneeKind::Agent,
            assignee_id: agent,
        },
    )
    .await;
    assert_eq!(
        command(&app, "agent-fixture", &own).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        snapshot(&app, "agent-fixture").await.0,
        StatusCode::FORBIDDEN
    );
    credential(&pool, id, "replacement-fixture").await;
    apply(&app, Command::Cancel { id, revision: 2 }).await;
    assert_eq!(
        command(&app, "replacement-fixture", &own).await.0,
        StatusCode::FORBIDDEN
    );
    let state = state(&app).await;
    assert_eq!(state.comments.len(), 1);
    assert_eq!(
        state.tickets.iter().find(|t| t.id == id).unwrap().status,
        TicketStatus::ToDo
    );
    assert!(state.runs.iter().all(|r| r.state == "cancelled"));
    let actions: Vec<String> = sqlx::query_scalar("SELECT action FROM dispatch_outbox ORDER BY id")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(actions, ["start", "cancel", "start", "cancel"]);
}

#[sqlx::test]
async fn owner_cannot_read_or_write_another_workspace(pool: PgPool) {
    sqlx::query("INSERT INTO workspaces VALUES ('other')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO principals VALUES ('other','owner','human','Another person')")
        .execute(&pool)
        .await
        .unwrap();
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO tickets(workspace_id,title) VALUES ('other','Private') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let app = app(&pool);
    assert!(state(&app).await.tickets.is_empty());
    assert_eq!(
        command(
            &app,
            "owner-fixture",
            &request(Command::Rename {
                id,
                revision: 0,
                title: "escape".into()
            })
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        command(
            &app,
            "wrong-token",
            &request(Command::Create {
                title: "escape".into()
            })
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT title FROM tickets WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "Private"
    );
}

#[sqlx::test(migrations = false)]
async fn previous_rows_keep_ids_titles_and_completion_during_migration(pool: PgPool) {
    for sql in [
        include_str!("../migrations/20260923000000_bootstrap.sql"),
        include_str!("../migrations/20260923010000_product_state.sql"),
    ] {
        sqlx::raw_sql(sql).execute(&pool).await.unwrap();
    }
    sqlx::query(
        "INSERT INTO todos(id,title,completed) VALUES (4,'Do this',false),(9,'Already done',true)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!("../migrations/20260923020000_tickets.sql"))
        .execute(&pool)
        .await
        .unwrap();
    let rows: Vec<(i64, String, String)> =
        sqlx::query_as("SELECT id,title,status FROM tickets ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        rows,
        vec![
            (4, "Do this".into(), "to_do".into()),
            (9, "Already done".into(), "done".into())
        ]
    );
    assert!(
        sqlx::query("UPDATE tickets SET status='blocked'")
            .execute(&pool)
            .await
            .is_err()
    );
}

#[sqlx::test]
async fn moving_agent_work_to_backlog_cancels_and_to_do_dispatches_again(pool: PgPool) {
    let (app, id, _) = assigned(&pool).await;
    apply(
        &app,
        Command::SetStatus {
            id,
            revision: 1,
            status: TicketStatus::Backlog,
        },
    )
    .await;
    let current = state(&app).await;
    assert_eq!(current.runs[0].state, "cancelled");
    assert_eq!(current.tickets[0].status, TicketStatus::Backlog);
    apply(
        &app,
        Command::SetStatus {
            id,
            revision: 2,
            status: TicketStatus::ToDo,
        },
    )
    .await;
    let current = state(&app).await;
    assert_eq!(current.runs.len(), 2);
    assert_eq!(current.runs[1].state, "queued");
    assert!(current.runs[1].generation > current.runs[0].generation);
    assert_ne!(current.runs[1].run_id, current.runs[0].run_id);
}

#[sqlx::test]
async fn deletion_uses_a_receipt_and_preserves_inspectable_run_history(pool: PgPool) {
    let (app, assigned, token) = assigned(&pool).await;
    let deletion = request(Command::Delete {
        id: assigned,
        revision: 1,
    });
    assert_eq!(
        command(&app, &token, &deletion).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        command(&app, "owner-fixture", &deletion).await.0,
        StatusCode::CONFLICT
    );
    let id = apply(
        &app,
        Command::Create {
            title: "Remove unused Ticket".into(),
        },
    )
    .await
    .result_id
    .unwrap();
    apply(
        &app,
        Command::AddComment {
            ticket_id: id,
            body: "User-entered draft".into(),
        },
    )
    .await;
    let deletion = request(Command::Delete { id, revision: 0 });
    let first = command(&app, "owner-fixture", &deletion).await;
    assert_eq!(first.0, StatusCode::OK);
    assert_eq!(command(&app, "owner-fixture", &deletion).await, first);
    let state = state(&app).await;
    assert_eq!(state.tickets.len(), 1);
    assert_eq!(state.runs.len(), 1);
    assert!(state.comments.is_empty());
}
