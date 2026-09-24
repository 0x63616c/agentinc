//! The phase-2 text runner. Pending rows are a durable queue; UI lifetimes have
//! no bearing on execution. Phase 3 replaces this runner with SDK sessions.
use crate::{codex, product::Turn};
use anyhow::Result;
use sqlx::{PgPool, postgres::PgListener};
use std::time::Duration;

pub struct Runner {
    pool: PgPool,
    owner: sqlx::pool::PoolConnection<sqlx::Postgres>,
    listener: PgListener,
}
impl Runner {
    pub async fn start(pool: PgPool) -> Result<Self> {
        // Hold a session lock for the entire runner lifetime. A second daemon must
        // never "recover" work owned by a live daemon.
        let mut owner = pool.acquire().await?;
        let owned: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock(7358710202)")
            .fetch_one(&mut *owner)
            .await?;
        anyhow::ensure!(owned, "another daemon already owns conversation execution");
        let mut listener = PgListener::connect_with(&pool).await?;
        listener.listen("agentinc_turns").await?;
        sqlx::query("UPDATE turns SET state='failed',error='Reply interrupted by daemon restart; outcome unknown. Retry when ready.' WHERE state='running'").execute(&pool).await?;
        Ok(Self {
            pool,
            owner,
            listener,
        })
    }
    pub async fn run(self) -> Result<()> {
        let Self {
            pool,
            mut owner,
            mut listener,
        } = self;
        let mut active = tokio::task::JoinSet::new();
        loop {
            while let Some(turn) = claim(&pool).await? {
                let pool = pool.clone();
                active.spawn(async move { complete(&pool, turn).await });
            }
            tokio::select! {
                result = listener.recv() => { result?; }
                Some(result) = active.join_next(), if !active.is_empty() => { result??; }
                // Detect a lost owner connection before a replacement can overlap
                // with this runner. The database records remain conservatively unknown.
                _ = tokio::time::sleep(Duration::from_secs(1)) => {
                    sqlx::query("SELECT 1").execute(&mut *owner).await?;
                }
            }
        }
    }
}

async fn claim(pool: &PgPool) -> Result<Option<Turn>> {
    Ok(sqlx::query_as("UPDATE turns SET state='running' WHERE id=(SELECT id FROM turns WHERE state='queued' ORDER BY id FOR UPDATE SKIP LOCKED LIMIT 1) RETURNING id,conversation_id,prompt,response,error,state").fetch_optional(pool).await?)
}

async fn complete(pool: &PgPool, turn: Turn) -> Result<()> {
    let history = sqlx::query_as("SELECT id,conversation_id,prompt,response,error,state FROM turns WHERE conversation_id=$1 ORDER BY id").bind(turn.conversation_id).fetch_all(pool).await?;
    let model: Option<String> = sqlx::query_scalar("SELECT model FROM turns WHERE id=$1")
        .bind(turn.id)
        .fetch_one(pool)
        .await?;
    let id = turn.id;
    let result = tokio::task::spawn_blocking(move || {
        codex::respond(model.as_deref().filter(|s| !s.is_empty()), &history, &turn)
    })
    .await?;
    save_result(pool, id, result).await
}

pub async fn save_result(pool: &PgPool, id: i64, result: Result<String>) -> Result<()> {
    let (response, error, state) = match result {
        Ok(text) => (Some(text), None, "completed"),
        Err(error) => (None, Some(error.to_string()), "failed"),
    };
    // Retain the completed result while Postgres is unavailable; saving again
    // must not call the provider a second time.
    loop {
        let result = sqlx::query(
            "WITH saved AS (UPDATE turns SET response=$2,error=$3,state=$4 WHERE id=$1 AND state='running' RETURNING id) SELECT pg_notify('agentinc_results',id::text) FROM saved",
        )
        .bind(id)
        .bind(&response)
        .bind(&error)
        .bind(state)
        .execute(pool)
        .await;
        match result {
            Ok(_) => return Ok(()),
            Err(error) if pool.is_closed() => return Err(error.into()),
            Err(error) => {
                tracing::warn!(turn_id=id, %error, "reply retained; retrying persistence");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}
