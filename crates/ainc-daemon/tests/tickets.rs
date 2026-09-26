use ainc_daemon::{
    product::Product,
    tickets::{
        ActivityKind, AssigneeKind, LinkKind, TicketActivity, TicketCommand as Command,
        TicketCommandRequest as Request, TicketLink, TicketPriority, TicketReceipt, TicketSnapshot,
        TicketStatus,
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

async fn activity(app: &Router, token: &str, id: i64) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            HttpRequest::get(format!("/v1/tickets/{id}/activity"))
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
async fn history(app: &Router, id: i64) -> Vec<TicketActivity> {
    let (status, body) = activity(app, "owner-fixture", id).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    serde_json::from_value(body).unwrap()
}
async fn detailed(app: &Router, title: &str, status: TicketStatus) -> i64 {
    apply(
        app,
        Command::CreateDetailed {
            title: title.into(),
            description: None,
            status: Some(status),
            priority: None,
            labels: None,
            assignee_id: None,
        },
    )
    .await
    .result_id
    .unwrap()
}
fn column(state: &TicketSnapshot, status: TicketStatus) -> Vec<&str> {
    state
        .tickets
        .iter()
        .filter(|t| t.status == status)
        .map(|t| t.title.as_str())
        .collect()
}
fn revision(state: &TicketSnapshot, id: i64) -> i64 {
    state.tickets.iter().find(|t| t.id == id).unwrap().revision
}

#[sqlx::test]
async fn detailed_create_validates_and_fills_every_board_field(pool: PgPool) {
    let app = app(&pool);
    let agent = agent(&app).await;
    let id = apply(
        &app,
        Command::CreateDetailed {
            title: "  Plan the launch  ".into(),
            description: Some("Checklist and owners.\n".into()),
            status: Some(TicketStatus::Blocked),
            priority: Some(TicketPriority::Urgent),
            labels: Some(vec!["Launch".into(), " launch ".into(), "Ops".into()]),
            assignee_id: Some(agent.clone()),
        },
    )
    .await
    .result_id
    .unwrap();
    let current = state(&app).await;
    let ticket = &current.tickets[0];
    assert_eq!(ticket.id, id);
    assert_eq!(ticket.title, "Plan the launch");
    assert_eq!(ticket.description, "Checklist and owners.");
    assert_eq!(ticket.status, TicketStatus::Blocked);
    assert_eq!(ticket.priority, TicketPriority::Urgent);
    assert_eq!(ticket.labels, ["Launch", "Ops"]);
    assert_eq!(
        (ticket.assignee_kind, ticket.assignee_id.as_str()),
        (AssigneeKind::Agent, agent.as_str())
    );
    assert!(ticket.created_at > 0 && ticket.updated_at >= ticket.created_at);
    // Blocked is not actionable, so assigning an agent starts nothing.
    assert!(current.runs.is_empty());
    let plain = apply(
        &app,
        Command::Create {
            title: "Defaults".into(),
        },
    )
    .await
    .result_id
    .unwrap();
    let current = state(&app).await;
    let plain = current.tickets.iter().find(|t| t.id == plain).unwrap();
    assert_eq!(
        (plain.status, plain.priority),
        (TicketStatus::ToDo, TicketPriority::None)
    );
    assert!(plain.labels.is_empty() && plain.description.is_empty());
    for invalid in [
        Command::CreateDetailed {
            title: " ".into(),
            description: None,
            status: None,
            priority: None,
            labels: None,
            assignee_id: None,
        },
        Command::CreateDetailed {
            title: "Labels".into(),
            description: None,
            status: None,
            priority: None,
            labels: Some(vec!["a,b".into()]),
            assignee_id: None,
        },
        Command::CreateDetailed {
            title: "Labels".into(),
            description: None,
            status: None,
            priority: None,
            labels: Some((0..11).map(|n| format!("l{n}")).collect()),
            assignee_id: None,
        },
        Command::CreateDetailed {
            title: "Stranger".into(),
            description: None,
            status: None,
            priority: None,
            labels: None,
            assignee_id: Some("nobody".into()),
        },
    ] {
        assert_eq!(
            command(&app, "owner-fixture", &request(invalid)).await.0,
            StatusCode::BAD_REQUEST
        );
    }
    // An agent created straight into To do starts work with the description in its prompt.
    let actionable = apply(
        &app,
        Command::CreateDetailed {
            title: "Write the notes".into(),
            description: Some("Use the launch checklist.".into()),
            status: None,
            priority: None,
            labels: None,
            assignee_id: Some(agent),
        },
    )
    .await
    .result_id
    .unwrap();
    let prompt: String = sqlx::query_scalar("SELECT prompt FROM ticket_runs WHERE ticket_id=$1")
        .bind(actionable)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(prompt, "Write the notes\n\nUse the launch checklist.");
}

#[sqlx::test]
async fn moves_reorder_columns_persist_and_refuse_stale_views(pool: PgPool) {
    let app = app(&pool);
    let a = detailed(&app, "A", TicketStatus::ToDo).await;
    let b = detailed(&app, "B", TicketStatus::ToDo).await;
    let c = detailed(&app, "C", TicketStatus::ToDo).await;
    // New Tickets arrive at the top of their column.
    assert_eq!(
        column(&state(&app).await, TicketStatus::ToDo),
        ["C", "B", "A"]
    );
    apply(
        &app,
        Command::Move {
            id: c,
            revision: 0,
            status: TicketStatus::ToDo,
            after: Some(a),
        },
    )
    .await;
    assert_eq!(
        column(&state(&app).await, TicketStatus::ToDo),
        ["B", "A", "C"]
    );
    // Reordering inside a column is not an edit.
    assert_eq!(revision(&state(&app).await, c), 0);
    apply(
        &app,
        Command::Move {
            id: a,
            revision: 0,
            status: TicketStatus::Blocked,
            after: None,
        },
    )
    .await;
    let current = state(&app).await;
    assert_eq!(column(&current, TicketStatus::ToDo), ["B", "C"]);
    assert_eq!(column(&current, TicketStatus::Blocked), ["A"]);
    assert_eq!(revision(&current, a), 1);
    apply(
        &app,
        Command::Move {
            id: b,
            revision: 0,
            status: TicketStatus::Blocked,
            after: Some(a),
        },
    )
    .await;
    apply(
        &app,
        Command::Move {
            id: c,
            revision: 0,
            status: TicketStatus::Blocked,
            after: Some(a),
        },
    )
    .await;
    assert_eq!(
        column(&state(&app).await, TicketStatus::Blocked),
        ["A", "C", "B"]
    );
    // A neighbour from another column, a stale revision and following itself are refused.
    let d = detailed(&app, "D", TicketStatus::Done).await;
    for (stale, status) in [
        (
            Command::Move {
                id: d,
                revision: 0,
                status: TicketStatus::Done,
                after: Some(a),
            },
            StatusCode::CONFLICT,
        ),
        (
            Command::Move {
                id: a,
                revision: 0,
                status: TicketStatus::Done,
                after: None,
            },
            StatusCode::CONFLICT,
        ),
        (
            Command::Move {
                id: d,
                revision: 0,
                status: TicketStatus::Done,
                after: Some(d),
            },
            StatusCode::BAD_REQUEST,
        ),
    ] {
        assert_eq!(
            command(&app, "owner-fixture", &request(stale)).await.0,
            status
        );
    }
    assert_eq!(
        column(&state(&app).await, TicketStatus::Blocked),
        ["A", "C", "B"]
    );
    // Every status column is available, in board order.
    for status in [
        TicketStatus::Backlog,
        TicketStatus::InProgress,
        TicketStatus::Cancelled,
    ] {
        detailed(&app, &format!("{status:?}"), status).await;
    }
    let order: Vec<_> = state(&app).await.tickets.iter().map(|t| t.status).collect();
    let mut sorted = order.clone();
    sorted.sort_by_key(|status| TicketStatus::ALL.iter().position(|s| s == status));
    assert_eq!(order, sorted);
}

#[sqlx::test]
async fn blocked_and_cancelled_stop_agent_work_and_to_do_restarts_it(pool: PgPool) {
    let (app, id, _) = assigned(&pool).await;
    apply(
        &app,
        Command::Move {
            id,
            revision: 1,
            status: TicketStatus::Blocked,
            after: None,
        },
    )
    .await;
    let current = state(&app).await;
    assert_eq!(current.runs[0].state, "cancelled");
    apply(
        &app,
        Command::SetStatus {
            id,
            revision: 2,
            status: TicketStatus::ToDo,
        },
    )
    .await;
    apply(
        &app,
        Command::SetStatus {
            id,
            revision: 3,
            status: TicketStatus::Cancelled,
        },
    )
    .await;
    let current = state(&app).await;
    assert_eq!(current.runs.len(), 2);
    assert!(current.runs.iter().all(|run| run.state == "cancelled"));
    let outbox: Vec<String> = sqlx::query_scalar("SELECT action FROM dispatch_outbox ORDER BY id")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(outbox, ["start", "cancel", "start", "cancel"]);
}

#[sqlx::test]
async fn relationships_are_canonical_loop_free_and_one_parent(pool: PgPool) {
    let app = app(&pool);
    let a = detailed(&app, "A", TicketStatus::ToDo).await;
    let b = detailed(&app, "B", TicketStatus::ToDo).await;
    let c = detailed(&app, "C", TicketStatus::ToDo).await;
    let link = |from_id, to_id, link| Command::Link {
        from_id,
        to_id,
        link,
    };
    apply(&app, link(a, b, LinkKind::Blocks)).await;
    apply(&app, link(b, c, LinkKind::Blocks)).await;
    // Relates-to is symmetric and stored once, whichever side asks.
    apply(&app, link(c, a, LinkKind::RelatesTo)).await;
    apply(&app, link(a, c, LinkKind::RelatesTo)).await;
    apply(&app, link(a, b, LinkKind::ParentOf)).await;
    apply(&app, link(c, a, LinkKind::Duplicates)).await;
    let current = state(&app).await;
    assert_eq!(
        current.links,
        [
            TicketLink {
                from_id: a,
                to_id: b,
                kind: LinkKind::Blocks
            },
            TicketLink {
                from_id: a,
                to_id: b,
                kind: LinkKind::ParentOf
            },
            TicketLink {
                from_id: a,
                to_id: c,
                kind: LinkKind::RelatesTo
            },
            TicketLink {
                from_id: b,
                to_id: c,
                kind: LinkKind::Blocks
            },
            TicketLink {
                from_id: c,
                to_id: a,
                kind: LinkKind::Duplicates
            },
        ]
    );
    for refused in [
        link(c, a, LinkKind::Blocks),
        link(b, a, LinkKind::ParentOf),
        link(c, b, LinkKind::ParentOf),
        link(a, a, LinkKind::RelatesTo),
        link(a, c, LinkKind::Duplicates),
    ] {
        assert_eq!(
            command(&app, "owner-fixture", &request(refused)).await.0,
            StatusCode::BAD_REQUEST
        );
    }
    // Both sides record the relationship from their own point of view.
    let linked = |entries: Vec<TicketActivity>| -> Vec<(String, String)> {
        entries
            .into_iter()
            .filter(|e| e.kind == ActivityKind::Linked)
            .map(|e| (e.from_value.unwrap(), e.to_value.unwrap()))
            .collect()
    };
    assert_eq!(
        linked(history(&app, b).await),
        [
            ("blocked_by".into(), a.to_string()),
            ("blocks".into(), c.to_string()),
            ("child_of".into(), a.to_string())
        ]
    );
    apply(
        &app,
        Command::Unlink {
            from_id: c,
            to_id: a,
            link: LinkKind::RelatesTo,
        },
    )
    .await;
    assert!(
        !state(&app)
            .await
            .links
            .iter()
            .any(|l| l.kind == LinkKind::RelatesTo)
    );
    assert!(
        history(&app, a)
            .await
            .iter()
            .any(|e| e.kind == ActivityKind::Unlinked)
    );
    // Deleting a Ticket removes its relationships.
    let current = state(&app).await;
    apply(
        &app,
        Command::Delete {
            id: c,
            revision: revision(&current, c),
        },
    )
    .await;
    assert!(
        state(&app)
            .await
            .links
            .iter()
            .all(|l| l.from_id != c && l.to_id != c)
    );
}

#[sqlx::test]
async fn links_stay_inside_one_workspace(pool: PgPool) {
    sqlx::query("INSERT INTO workspaces VALUES ('other')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO principals VALUES ('other','owner','human','Another person')")
        .execute(&pool)
        .await
        .unwrap();
    let foreign: i64 = sqlx::query_scalar(
        "INSERT INTO tickets(workspace_id,title) VALUES ('other','Private') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let app = app(&pool);
    let own = detailed(&app, "Mine", TicketStatus::ToDo).await;
    let escape = request(Command::Link {
        from_id: own,
        to_id: foreign,
        link: LinkKind::RelatesTo,
    });
    assert_eq!(
        command(&app, "owner-fixture", &escape).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        activity(&app, "owner-fixture", foreign).await.1,
        serde_json::json!([])
    );
}

#[sqlx::test]
async fn history_records_every_edit_in_order_and_touches_updated_time(pool: PgPool) {
    let app = app(&pool);
    let agent = agent(&app).await;
    let id = detailed(&app, "Draft", TicketStatus::Backlog).await;
    sqlx::query("UPDATE tickets SET updated_at=0 WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    let edits = [
        Command::Rename {
            id,
            revision: 0,
            title: "Final".into(),
        },
        Command::Describe {
            id,
            revision: 1,
            description: "Scope".into(),
        },
        Command::SetPriority {
            id,
            revision: 2,
            priority: TicketPriority::High,
        },
        Command::SetLabels {
            id,
            revision: 3,
            labels: vec!["Home".into(), "Money".into()],
        },
        Command::Assign {
            id,
            revision: 4,
            assignee_kind: AssigneeKind::Agent,
            assignee_id: agent.clone(),
        },
        Command::SetStatus {
            id,
            revision: 5,
            status: TicketStatus::ToDo,
        },
        Command::Cancel { id, revision: 6 },
    ];
    for edit in edits {
        apply(&app, edit).await;
    }
    // Unchanged values record nothing and keep the revision.
    apply(
        &app,
        Command::SetPriority {
            id,
            revision: 7,
            priority: TicketPriority::High,
        },
    )
    .await;
    let entries = history(&app, id).await;
    let summary: Vec<_> = entries
        .iter()
        .map(|e| (e.kind, e.from_value.as_deref(), e.to_value.as_deref()))
        .collect();
    assert_eq!(
        summary,
        [
            (ActivityKind::Created, None, Some("Draft")),
            (ActivityKind::Renamed, Some("Draft"), Some("Final")),
            (ActivityKind::Described, None, None),
            (ActivityKind::Priority, Some("none"), Some("high")),
            (ActivityKind::Labels, Some(""), Some("Home,Money")),
            (ActivityKind::Assigned, Some("owner"), Some(agent.as_str())),
            (ActivityKind::Status, Some("backlog"), Some("to_do")),
            (ActivityKind::Work, None, Some("queued")),
            (ActivityKind::Work, None, Some("cancelled")),
        ]
    );
    assert!(
        entries
            .iter()
            .all(|e| e.actor_id == "owner" && e.conversation_id.is_none())
    );
    let run = &state(&app).await.runs[0];
    assert!(
        entries
            .iter()
            .filter(|e| e.kind == ActivityKind::Work)
            .all(|e| e.run_id.as_deref() == Some(run.run_id.as_str()))
    );
    let ticket = state(&app)
        .await
        .tickets
        .into_iter()
        .find(|t| t.id == id)
        .unwrap();
    assert_eq!(ticket.revision, 7);
    assert!(ticket.updated_at > 0);
}

#[sqlx::test]
async fn agent_credential_reads_only_its_own_history_and_links(pool: PgPool) {
    let (app, id, _) = assigned(&pool).await;
    credential(&pool, id, "agent-fixture").await;
    let other = detailed(&app, "Elsewhere", TicketStatus::Backlog).await;
    let third = detailed(&app, "Third", TicketStatus::Backlog).await;
    apply(
        &app,
        Command::Link {
            from_id: other,
            to_id: id,
            link: LinkKind::Blocks,
        },
    )
    .await;
    apply(
        &app,
        Command::Link {
            from_id: other,
            to_id: third,
            link: LinkKind::Blocks,
        },
    )
    .await;
    assert_eq!(activity(&app, "agent-fixture", id).await.0, StatusCode::OK);
    assert_eq!(
        activity(&app, "agent-fixture", other).await.0,
        StatusCode::FORBIDDEN
    );
    let scoped: TicketSnapshot =
        serde_json::from_value(snapshot(&app, "agent-fixture").await.1).unwrap();
    assert_eq!(
        scoped.links,
        [TicketLink {
            from_id: other,
            to_id: id,
            kind: LinkKind::Blocks
        }]
    );
    let moved = request(Command::Move {
        id,
        revision: 1,
        status: TicketStatus::Done,
        after: None,
    });
    assert_eq!(
        command(&app, "agent-fixture", &moved).await.0,
        StatusCode::FORBIDDEN
    );
}

#[sqlx::test(migrations = false)]
async fn board_migration_orders_existing_columns_and_starts_their_history(pool: PgPool) {
    let migrations = [
        include_str!("../migrations/20260923000000_bootstrap.sql"),
        include_str!("../migrations/20260923010000_product_state.sql"),
        include_str!("../migrations/20260923020000_tickets.sql"),
        include_str!("../migrations/20260923030000_execution.sql"),
        include_str!("../migrations/20260923040000_conversation_sessions.sql"),
        include_str!("../migrations/20260924000000_automations.sql"),
        include_str!("../migrations/20260924010000_workspaces.sql"),
    ];
    for sql in migrations {
        sqlx::raw_sql(sql).execute(&pool).await.unwrap();
    }
    sqlx::query("INSERT INTO tickets(id,title,status) VALUES (1,'Old','to_do'),(2,'Newer','to_do'),(3,'Shipped','done')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!(
        "../migrations/20260926000000_ticket_board.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let rows: Vec<(i64, String, i64, String)> =
        sqlx::query_as("SELECT id,status,position,priority FROM tickets ORDER BY status,position")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        rows,
        [
            (3, "done".into(), 1, "none".into()),
            (2, "to_do".into(), 1, "none".into()),
            (1, "to_do".into(), 2, "none".into()),
        ]
    );
    let created: Vec<(i64, String)> = sqlx::query_as(
        "SELECT ticket_id,to_value FROM ticket_activity WHERE kind='created' ORDER BY ticket_id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        created,
        [
            (1, "Old".into()),
            (2, "Newer".into()),
            (3, "Shipped".into())
        ]
    );
    sqlx::query("UPDATE tickets SET status='blocked' WHERE id=1")
        .execute(&pool)
        .await
        .unwrap();
}
