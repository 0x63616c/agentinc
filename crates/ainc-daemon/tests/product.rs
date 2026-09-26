use ainc_daemon::{
    legacy,
    product::{self, Command, CommandRequest, Product},
};
use sqlx::PgPool;

fn request(command: Command) -> CommandRequest {
    CommandRequest {
        operation_id: uuid::Uuid::new_v4().to_string(),
        command,
    }
}
async fn apply(pool: &PgPool, command: Command) -> i64 {
    product::execute(pool, request(command))
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
    let ack = product::execute(&pool, command.clone()).await.unwrap();
    assert_eq!(
        product::execute(&pool, command.clone())
            .await
            .unwrap()
            .result_id,
        ack.result_id
    );
    let mut conflicting = command;
    conflicting.command = Command::CreateTodo {
        title: "different".into(),
    };
    assert!(product::execute(&pool, conflicting).await.is_err());
    assert!(
        product::execute(&pool, request(Command::CreateTodo { title: "  ".into() }))
            .await
            .is_err()
    );
    let state = product::snapshot(&pool).await.unwrap();
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
    assert!(product::snapshot(&pool).await.unwrap().todos[0].completed);
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
        command: None,
    });
    let response = app
        .clone()
        .oneshot(
            Request::post("/v1/commands")
                .header("agent-inc-client", ainc_release::client_header())
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
    let state = product::snapshot(&pool).await.unwrap();
    assert_eq!(state.turns[0].state, "queued");
    let id = state.turns[0].id;
    sqlx::query("UPDATE turns SET state='running' WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    ainc_daemon::conversations::save_result(&pool, id, Ok("Saved without a window".into()))
        .await
        .unwrap();
    let state = product::snapshot(&pool).await.unwrap();
    assert_eq!(
        state.turns[0].response.as_deref(),
        Some("Saved without a window")
    );
    assert_eq!(state.turns[0].state, "completed");
    assert_eq!(
        product::execute(&pool, command).await.unwrap().result_id,
        Some(id)
    );
    assert_eq!(product::snapshot(&pool).await.unwrap().turns.len(), 1);
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
            command: None,
        },
    )
    .await;
    assert!(
        product::execute(
            &pool,
            request(Command::Send {
                conversation_id: first,
                prompt: "overlap".into(),
                command: None,
            })
        )
        .await
        .is_err()
    );
    assert!(
        product::execute(&pool, request(Command::DeleteConversation { id: first }))
            .await
            .is_err()
    );
    apply(
        &pool,
        Command::Send {
            conversation_id: second,
            prompt: "independent".into(),
            command: None,
        },
    )
    .await;
    apply(&pool, Command::SelectConversation { id: second }).await;
    assert_eq!(
        product::snapshot(&pool)
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
                .header("agent-inc-client", ainc_release::client_header())
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
    let state = product::snapshot(&pool).await.unwrap();
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
    assert_eq!(
        product::snapshot(&pool).await.unwrap().conversations.len(),
        1
    );
}

#[sqlx::test]
async fn failed_import_rolls_back_and_future_schema_is_untouched(pool: PgPool) {
    let directory = tempfile::tempdir().unwrap();
    legacy_fixture(directory.path());
    let db = rusqlite::Connection::open(directory.path().join("assistant.sqlite3")).unwrap();
    db.execute("UPDATE todos SET title=''", []).unwrap();
    assert!(legacy::import(&pool, directory.path()).await.is_err());
    assert!(
        product::snapshot(&pool)
            .await
            .unwrap()
            .conversations
            .is_empty()
    );
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

#[sqlx::test]
async fn stopping_a_queued_reply_records_the_outcome_and_allows_retry(pool: PgPool) {
    let conversation = apply(&pool, Command::CreateConversation).await;
    let turn = apply(
        &pool,
        Command::Send {
            conversation_id: conversation,
            prompt: "Take your time".into(),
            command: Some("/http https://example.test".into()),
        },
    )
    .await;
    apply(&pool, Command::StopTurn { id: turn }).await;
    let state = product::snapshot(&pool).await.unwrap();
    let stopped = state.turns.iter().find(|t| t.id == turn).unwrap();
    assert_eq!(stopped.state, "failed");
    assert_eq!(stopped.error.as_deref(), Some("Stopped."));
    assert_eq!(
        stopped.command.as_deref(),
        Some("/http https://example.test")
    );
    assert!(stopped.finished_at.is_some());
    assert_eq!(state.conversations[0].title, "/http https://example.test");
    // Stopping twice is a conflict, and a stopped reply can be retried.
    assert!(
        product::execute(&pool, request(Command::StopTurn { id: turn }))
            .await
            .is_err()
    );
    apply(&pool, Command::Retry { id: turn }).await;
    let state = product::snapshot(&pool).await.unwrap();
    assert_eq!(
        state.turns.iter().find(|t| t.id == turn).unwrap().state,
        "queued"
    );
}
