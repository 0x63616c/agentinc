//! Order inside a status column. Positions are renumbered 0..n on every move,
//! so a column never runs out of room between two neighbours.
use super::{TicketStatus, invalid};
use crate::product::ApiError;
use sqlx::{Postgres, Transaction};

/// Serialize every writer of one workspace's board: statuses, order and
/// relationships. Take it before any Ticket row lock so writers never wait on
/// each other in opposite orders.
pub(crate) async fn lock(
    tx: &mut Transaction<'_, Postgres>,
    workspace: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("board/{workspace}"))
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// The position that puts a Ticket above everything in a column.
pub(super) async fn top(
    tx: &mut Transaction<'_, Postgres>,
    workspace: &str,
    status: TicketStatus,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT COALESCE(min(position),1)-1 FROM tickets WHERE workspace_id=$1 AND status=$2",
    )
    .bind(workspace)
    .bind(status)
    .fetch_one(&mut **tx)
    .await
}

/// Put a Ticket in `status` at the top of that column. Every status change
/// goes through here; revision and generation stay with the caller.
pub(crate) async fn enter_column(
    tx: &mut Transaction<'_, Postgres>,
    id: i64,
    status: TicketStatus,
) -> Result<(), sqlx::Error> {
    let workspace: String = sqlx::query_scalar("SELECT workspace_id FROM tickets WHERE id=$1")
        .bind(id)
        .fetch_one(&mut **tx)
        .await?;
    let position = top(tx, &workspace, status).await?;
    sqlx::query("UPDATE tickets SET status=$2,position=$3 WHERE id=$1")
        .bind(id)
        .bind(status)
        .bind(position)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// `column` without `id`, with `id` inserted directly below `after` (or first).
/// `None` when `after` is not in the column.
fn insert_after(column: &[i64], id: i64, after: Option<i64>) -> Option<Vec<i64>> {
    let mut order: Vec<i64> = column
        .iter()
        .copied()
        .filter(|&other| other != id)
        .collect();
    let index = match after {
        None => 0,
        Some(after) => order.iter().position(|&other| other == after)? + 1,
    };
    order.insert(index, id);
    Some(order)
}

/// Place a Ticket already in `status` directly below `after`. The caller holds
/// the workspace's board lock.
pub(super) async fn place(
    tx: &mut Transaction<'_, Postgres>,
    workspace: &str,
    id: i64,
    status: TicketStatus,
    after: Option<i64>,
) -> Result<(), ApiError> {
    if after == Some(id) {
        return Err(invalid("A Ticket cannot follow itself."));
    }
    let column: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM tickets WHERE workspace_id=$1 AND status=$2 ORDER BY position,id DESC",
    )
    .bind(workspace)
    .bind(status)
    .fetch_all(&mut **tx)
    .await?;
    // A neighbour that moved away since the client read the board is a stale view.
    let order = insert_after(&column, id, after).ok_or_else(ApiError::conflict)?;
    let positions: Vec<i64> = (0..order.len() as i64).collect();
    sqlx::query("UPDATE tickets t SET position=v.position FROM unnest($1::bigint[],$2::bigint[]) AS v(id,position) WHERE t.id=v.id AND t.position<>v.position")
        .bind(&order)
        .bind(&positions)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::insert_after;

    #[test]
    fn inserts_below_a_neighbour_or_first_and_refuses_a_missing_one() {
        assert_eq!(insert_after(&[1, 2, 3], 9, None), Some(vec![9, 1, 2, 3]));
        assert_eq!(insert_after(&[1, 2, 3], 9, Some(2)), Some(vec![1, 2, 9, 3]));
        assert_eq!(insert_after(&[1, 2, 3], 9, Some(3)), Some(vec![1, 2, 3, 9]));
        // Reordering inside the same column removes the old slot first.
        assert_eq!(insert_after(&[1, 2, 3], 1, Some(3)), Some(vec![2, 3, 1]));
        assert_eq!(insert_after(&[1, 2, 3], 3, None), Some(vec![3, 1, 2]));
        assert_eq!(insert_after(&[1, 2, 3], 9, Some(7)), None);
        assert_eq!(insert_after(&[], 9, None), Some(vec![9]));
    }
}
