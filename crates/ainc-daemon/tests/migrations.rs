//! Shape rules every migration must leave the schema in.
use sqlx::PgPool;

/// An epoch-second `bigint` time column defaults to `clock_timestamp()`, never `now()`, so
/// records written in one transaction keep their order (S5).
#[sqlx::test]
async fn epoch_time_columns_default_to_clock_timestamp(pool: PgPool) {
    let using_now: Vec<String> = sqlx::query_scalar(
        "SELECT table_name || '.' || column_name FROM information_schema.columns \
         WHERE table_schema = 'public' AND data_type = 'bigint' \
         AND column_default LIKE '%epoch%' AND column_default NOT LIKE '%clock_timestamp()%' \
         ORDER BY 1",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(using_now.is_empty(), "defaults using now(): {using_now:?}");
}

async fn occurrence(pool: &PgPool) -> String {
    sqlx::query(
        "INSERT INTO principals(workspace_id,id,kind,name) VALUES ('local','agent-1','agent','A')",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO agents(workspace_id,id,instructions,model) VALUES ('local','agent-1','i','m')",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO automations(id,workspace_id,name,prompt,agent_id) VALUES ('a','local','n','p','agent-1')")
        .execute(pool)
        .await
        .unwrap();
    "o1".into()
}

/// Expand step of occurrences.state -> status: a daemon writing either column leaves both equal.
#[sqlx::test]
async fn occurrence_state_and_status_stay_in_step_for_old_and_new_writers(pool: PgPool) {
    let id = occurrence(&pool).await;
    let both = |id: String| {
        let pool = pool.clone();
        async move {
            sqlx::query_as::<_, (String, String)>(
                "SELECT state,status FROM occurrences WHERE id=$1",
            )
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap()
        }
    };
    // New writer inserts and updates `status`.
    sqlx::query(
        "INSERT INTO occurrences(id,automation_id,revision,status) VALUES ($1,'a',0,'queued')",
    )
    .bind(&id)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(both(id.clone()).await, ("queued".into(), "queued".into()));
    sqlx::query("UPDATE occurrences SET status='superseded' WHERE id=$1")
        .bind(&id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        both(id.clone()).await,
        ("superseded".into(), "superseded".into())
    );
    // Old writer touches only `state`.
    sqlx::query("UPDATE occurrences SET state='paused' WHERE id=$1")
        .bind(&id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(both(id.clone()).await, ("paused".into(), "paused".into()));
    sqlx::query(
        "INSERT INTO occurrences(id,automation_id,revision,state) VALUES ('o2','a',0,'failed')",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(both("o2".into()).await, ("failed".into(), "failed".into()));
    // Neither written: the shared default.
    sqlx::query("INSERT INTO occurrences(id,automation_id,revision) VALUES ('o3','a',0)")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        both("o3".into()).await,
        ("waiting_for_worker".into(), "waiting_for_worker".into())
    );
}

/// Rows written before the migration get their `status` from `state`.
#[sqlx::test(migrations = false)]
async fn the_expand_migration_backfills_existing_occurrences(pool: PgPool) {
    let mut migrator = sqlx::migrate!();
    let all = migrator.migrations.to_vec();
    migrator.migrations = all
        .iter()
        .filter(|m| m.version < 20261004010000)
        .cloned()
        .collect::<Vec<_>>()
        .into();
    migrator.run(&pool).await.unwrap();
    occurrence(&pool).await;
    sqlx::query(
        "INSERT INTO occurrences(id,automation_id,revision,state) VALUES ('old','a',0,'failed')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    let status: String = sqlx::query_scalar("SELECT status FROM occurrences WHERE id='old'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "failed");
}
