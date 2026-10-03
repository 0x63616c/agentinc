//! The SQL behind each Conversation command, inside the receipt's transaction.
use super::ConversationCommand;
use crate::{api::CommandError, pg::coordination};
use sqlx::{Postgres, Transaction};

pub(super) async fn apply(
    tx: &mut Transaction<'_, Postgres>,
    workspace: &str,
    command: ConversationCommand,
) -> Result<Option<i64>, CommandError> {
    let result_id = match command {
        ConversationCommand::Create => Some(
            sqlx::query_scalar!(
                "INSERT INTO conversations(workspace_id,title) VALUES ($1,'New conversation') RETURNING id",
                workspace
            )
            .fetch_one(&mut **tx)
            .await?,
        ),
        ConversationCommand::Rename { id, title } => {
            changed(
                sqlx::query!(
                    "UPDATE conversations SET title=$2 WHERE id=$1 AND workspace_id=$3",
                    id,
                    title.trim(),
                    workspace
                )
                .execute(&mut **tx)
                .await?
                .rows_affected(),
            )?;
            Some(id)
        }
        ConversationCommand::Delete { id } => {
            // Lock the parent against send, retry and worker claims.
            let row = sqlx::query_scalar!(
                "SELECT id FROM conversations WHERE id=$1 AND workspace_id=$2 FOR UPDATE",
                id,
                workspace
            )
            .fetch_optional(&mut **tx)
            .await?;
            if row.is_none() {
                return Err(CommandError::NotFound);
            }
            let pending = sqlx::query_scalar!(
                r#"SELECT EXISTS(SELECT 1 FROM turns WHERE conversation_id=$1 AND state IN ('queued','running')) AS "pending!""#,
                id
            )
            .fetch_one(&mut **tx)
            .await?;
            if pending {
                return Err(CommandError::Conflict(
                    "Wait for the reply in progress.".into(),
                ));
            }
            sqlx::query!(
                "UPDATE conversation_sessions SET state='closed' WHERE conversation_id=$1 AND state='active'",
                id
            )
            .execute(&mut **tx)
            .await?;
            sqlx::query!("DELETE FROM conversations WHERE id=$1", id)
                .execute(&mut **tx)
                .await?;
            Some(id)
        }
        ConversationCommand::Select { id } => {
            let exists = sqlx::query_scalar!(
                r#"SELECT EXISTS(SELECT 1 FROM conversations WHERE id=$1 AND workspace_id=$2) AS "exists!""#,
                id,
                workspace
            )
            .fetch_one(&mut **tx)
            .await?;
            if !exists {
                return Err(CommandError::NotFound);
            }
            sqlx::query!(
                "INSERT INTO assistant_settings(workspace_id,key,value) VALUES ($1,'selected_conversation',$2) ON CONFLICT(workspace_id,key) DO UPDATE SET value=excluded.value",
                workspace,
                id.to_string()
            )
            .execute(&mut **tx)
            .await?;
            Some(id)
        }
        ConversationCommand::Send {
            conversation_id,
            prompt,
        } => {
            let parent = sqlx::query_scalar!(
                "SELECT id FROM conversations WHERE id=$1 AND workspace_id=$2 FOR UPDATE",
                conversation_id,
                workspace
            )
            .fetch_optional(&mut **tx)
            .await?;
            if parent.is_none() {
                return Err(CommandError::NotFound);
            }
            let id = sqlx::query_scalar!(
                "INSERT INTO turns(conversation_id,prompt,state,model) VALUES ($1,$2,'queued',(SELECT value FROM assistant_settings WHERE workspace_id=$3 AND key='model')) RETURNING id",
                conversation_id,
                prompt.trim(),
                workspace
            )
            .fetch_one(&mut **tx)
            .await?;
            sqlx::query!(
                "UPDATE conversations SET updated_at=extract(epoch FROM clock_timestamp())::bigint,title=CASE WHEN title='New conversation' THEN left($2,60) ELSE title END WHERE id=$1",
                conversation_id,
                prompt.trim()
            )
            .execute(&mut **tx)
            .await?;
            Some(id)
        }
        ConversationCommand::Retry { id } => {
            let parent = sqlx::query_scalar!(
                "SELECT c.id FROM conversations c JOIN turns t ON c.id=t.conversation_id WHERE t.id=$1 AND c.workspace_id=$2 FOR UPDATE OF c",
                id,
                workspace
            )
            .fetch_optional(&mut **tx)
            .await?;
            if parent.is_none() {
                return Err(CommandError::NotFound);
            }
            let retried = sqlx::query!(
                "UPDATE turns SET error=NULL,response=NULL,state='queued',attempt=attempt+1,session_id=NULL WHERE id=$1 AND state='failed'",
                id
            )
            .execute(&mut **tx)
            .await?
            .rows_affected();
            if retried == 0 {
                return Err(CommandError::Conflict(
                    "Only a failed reply can be retried.".into(),
                ));
            }
            Some(id)
        }
        ConversationCommand::SelectModel { model } => {
            sqlx::query!(
                "INSERT INTO assistant_settings(workspace_id,key,value) VALUES ($1,'model',$2) ON CONFLICT(workspace_id,key) DO UPDATE SET value=excluded.value",
                workspace,
                model
            )
            .execute(&mut **tx)
            .await?;
            None
        }
    };
    sqlx::query!("SELECT pg_notify($1,'')", coordination::TURNS)
        .execute(&mut **tx)
        .await?;
    Ok(result_id)
}

/// An UPDATE that matched no row: the record is gone.
fn changed(rows: u64) -> Result<(), CommandError> {
    if rows == 0 {
        Err(CommandError::NotFound)
    } else {
        Ok(())
    }
}
