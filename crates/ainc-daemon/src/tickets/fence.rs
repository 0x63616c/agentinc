//! The one answer to "is this agent's assignment still live?". An assignment
//! is live while its Ticket is still that agent's, still on the same
//! generation, still in an actionable status, and the generation's run is
//! queued or running. Every writer acting for an agent proves this inside its
//! own transaction, so a credential made stale by cancellation or reassignment
//! writes nothing.
use super::{Actor, AssigneeKind, TICKET_COLUMNS, Ticket, TicketStatus, denied};
use crate::product::ApiError;
use sqlx::{Postgres, Transaction};

/// Proof, inside one transaction, that the actor's assignment is live.
#[derive(Clone, Debug)]
pub(crate) struct LiveAssignment {
    pub ticket: Ticket,
}

impl LiveAssignment {
    /// Lock the assigned Ticket's row and prove the assignment is live. Takes
    /// the workspace's board lock first, like every Ticket writer. Denied when
    /// the actor has no assignment or it is no longer live.
    pub(crate) async fn lock(
        tx: &mut Transaction<'_, Postgres>,
        actor: &Actor,
    ) -> Result<Self, ApiError> {
        Self::try_lock(tx, actor).await?.ok_or_else(denied)
    }

    /// [`Self::lock`] without the refusal: `None` when the assignment is stale,
    /// for writers that step aside quietly.
    pub(crate) async fn try_lock(
        tx: &mut Transaction<'_, Postgres>,
        actor: &Actor,
    ) -> Result<Option<Self>, sqlx::Error> {
        super::board::lock(tx, &actor.workspace).await?;
        Self::find(tx, actor, true).await
    }

    /// The same proof without any lock, for read-only snapshots.
    pub(crate) async fn check(
        tx: &mut Transaction<'_, Postgres>,
        actor: &Actor,
    ) -> Result<Self, ApiError> {
        Self::find(tx, actor, false).await?.ok_or_else(denied)
    }

    async fn find(
        tx: &mut Transaction<'_, Postgres>,
        actor: &Actor,
        lock: bool,
    ) -> Result<Option<Self>, sqlx::Error> {
        let Some((ticket, generation)) = actor.assignment else {
            return Ok(None);
        };
        let columns = TICKET_COLUMNS
            .split(',')
            .map(|column| format!("t.{column}"))
            .collect::<Vec<_>>()
            .join(",");
        let locking = if lock { " FOR UPDATE OF t" } else { "" };
        let ticket: Option<Ticket> = sqlx::query_as(&format!(
            "SELECT {columns} FROM tickets t JOIN ticket_runs r ON r.ticket_id=t.id AND r.generation=t.generation \
             WHERE t.id=$1 AND t.workspace_id=$2 AND t.generation=$3 AND t.assignee_kind=$4 AND t.assignee_id=$5 AND r.agent_id=$5 \
             AND t.status=ANY($6) AND r.state IN ('queued','running'){locking}"
        ))
        .bind(ticket)
        .bind(&actor.workspace)
        .bind(generation)
        .bind(AssigneeKind::Agent)
        .bind(&actor.id)
        .bind(
            TicketStatus::ALL
                .into_iter()
                .filter(|status| status.actionable())
                .map(TicketStatus::as_str)
                .collect::<Vec<_>>(),
        )
        .fetch_optional(&mut **tx)
        .await?;
        Ok(ticket.map(|ticket| Self { ticket }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tickets::{self, TicketCommand, TicketCommandRequest};
    use sqlx::PgPool;

    async fn apply(pool: &PgPool, command: TicketCommand) -> Option<i64> {
        tickets::execute(
            pool,
            &Actor::owner(),
            TicketCommandRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                command,
            },
        )
        .await
        .unwrap()
        .result_id
    }

    #[sqlx::test]
    async fn only_the_current_generation_of_a_live_assignment_passes(pool: PgPool) {
        apply(
            &pool,
            TicketCommand::RegisterAgent {
                name: "Fixture".into(),
                instructions: "Prove liveness".into(),
                model: "fixture".into(),
            },
        )
        .await;
        let agent: String = sqlx::query_scalar("SELECT id FROM agents")
            .fetch_one(&pool)
            .await
            .unwrap();
        let id = apply(
            &pool,
            TicketCommand::Create {
                title: "Fenced".into(),
            },
        )
        .await
        .unwrap();
        apply(
            &pool,
            TicketCommand::Assign {
                id,
                revision: 0,
                assignee_kind: AssigneeKind::Agent,
                assignee_id: agent.clone(),
            },
        )
        .await;
        let actor = |generation| Actor {
            workspace: "local".into(),
            id: agent.clone(),
            assignment: Some((id, generation)),
            conversation: None,
        };
        let mut tx = pool.begin().await.unwrap();
        let live = LiveAssignment::lock(&mut tx, &actor(1)).await.unwrap();
        assert_eq!(live.ticket.id, id);
        assert_eq!(live.ticket.status, TicketStatus::ToDo);
        assert!(LiveAssignment::lock(&mut tx, &actor(0)).await.is_err());
        assert!(LiveAssignment::check(&mut tx, &actor(2)).await.is_err());
        assert!(
            LiveAssignment::lock(&mut tx, &Actor::owner())
                .await
                .is_err()
        );
        tx.commit().await.unwrap();
        // Cancelling makes generation 1 stale; nothing is live until the next assignment.
        apply(&pool, TicketCommand::Cancel { id, revision: 1 }).await;
        let mut tx = pool.begin().await.unwrap();
        assert!(
            LiveAssignment::try_lock(&mut tx, &actor(1))
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            LiveAssignment::try_lock(&mut tx, &actor(2))
                .await
                .unwrap()
                .is_none()
        );
    }
}
