//! Postgres transaction helpers shared by every module that owns product state.
pub mod coordination;

use sqlx::{PgPool, Postgres, Transaction};

/// A consistent read of several tables: one REPEATABLE READ, read-only
/// transaction. Every snapshot endpoint reads through one of these.
pub(crate) async fn snapshot_tx(
    pool: &PgPool,
) -> Result<Transaction<'static, Postgres>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}
