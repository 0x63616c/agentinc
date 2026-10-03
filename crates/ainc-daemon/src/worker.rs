//! The one skeleton behind every leased background runner: hold an advisory lock, wake on
//! NOTIFY or a tick, reconcile, and drain in-flight tasks on shutdown.
use anyhow::Result;
use sqlx::{PgConnection, PgPool, postgres::PgListener};
use std::{collections::HashSet, future::Future, time::Duration};
use tokio::task::JoinSet;

/// What a runner does on each wake-up.
pub(crate) trait Reconcile: Send {
    /// Start whatever is due. Long work is spawned into `tasks`, keyed so it starts once.
    fn reconcile(&mut self, tasks: &mut Tasks) -> impl Future<Output = Result<()>> + Send;
    /// Release the runner's resources after its tasks were stopped.
    fn drain(self) -> impl Future<Output = Result<()>> + Send;
}

/// In-flight work, at most one task per key.
#[derive(Default)]
pub(crate) struct Tasks {
    set: JoinSet<Result<String>>,
    active: HashSet<String>,
}
impl Tasks {
    /// Spawn `work` unless a task with this key is already running.
    pub(crate) fn spawn(
        &mut self,
        key: impl Into<String>,
        work: impl Future<Output = Result<()>> + Send + 'static,
    ) {
        let key = key.into();
        if self.active.insert(key.clone()) {
            self.set.spawn(async move {
                work.await?;
                Ok(key)
            });
        }
    }
}

/// Exclusive ownership of one kind of background work, held by an advisory lock on a
/// connection that lives as long as the runner.
pub(crate) struct Leased {
    owner: PgConnection,
    listener: PgListener,
    tick: Duration,
}
impl Leased {
    pub(crate) async fn acquire(
        pool: &PgPool,
        lock: i64,
        channels: &[&str],
        tick: Duration,
        owned_elsewhere: &'static str,
    ) -> Result<Self> {
        let mut owner = pool.acquire().await?.detach();
        let owned = sqlx::query_scalar!(r#"SELECT pg_try_advisory_lock($1) AS "owned!""#, lock)
            .fetch_one(&mut owner)
            .await?;
        anyhow::ensure!(owned, owned_elsewhere);
        let mut listener = PgListener::connect_with(pool).await?;
        listener.listen_all(channels.iter().copied()).await?;
        Ok(Self {
            owner,
            listener,
            tick,
        })
    }

    pub(crate) async fn run_until<W: Reconcile>(
        mut self,
        mut work: W,
        shutdown: impl Future<Output = ()>,
    ) -> Result<()> {
        tokio::pin!(shutdown);
        let mut tasks = Tasks::default();
        loop {
            // Notices loss of exclusive ownership.
            sqlx::query!("SELECT 1 AS alive")
                .fetch_one(&mut self.owner)
                .await?;
            work.reconcile(&mut tasks).await?;
            tokio::select! {
                _ = &mut shutdown => {
                    tasks.set.shutdown().await;
                    return work.drain().await;
                }
                // The tick also reconciles a lost NOTIFY.
                notification = self.listener.recv() => { notification?; }
                Some(done) = tasks.set.join_next(), if !tasks.set.is_empty() => {
                    tasks.active.remove(&done??);
                }
                _ = tokio::time::sleep(self.tick) => {}
            }
        }
    }
}
