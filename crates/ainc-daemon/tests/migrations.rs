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
