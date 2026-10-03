//! Retained history, read one page at a time through the engine's own client.

use super::legacy;
#[allow(deprecated)]
use crate::runtime::{RunPage, RunRecord};
use crate::{
    Error,
    runtime::{RunKind, RunStatus, WorkPage, WorkRecord},
};
use temporalio_client::{Client, NamespacedClient, grpc::WorkflowService, tonic::IntoRequest};
use temporalio_common::protos::temporal::api::workflowservice::v1::ListWorkflowExecutionsRequest;

const PAGE_SIZE: i32 = 40;

/// One retained execution, before it is spoken of in agent vocabulary.
struct Row {
    id: String,
    attempt_id: String,
    workflow_type: String,
    status: i32,
    started_at: i64,
    closed_at: Option<i64>,
}

fn millis(seconds: i64, nanos: i32) -> Option<i64> {
    seconds
        .checked_mul(1000)?
        .checked_add(i64::from(nanos) / 1_000_000)
}

/// The page token is the service's own continuation token, hex-encoded so it survives a
/// URL. Anything else is the caller's mistake, not an outage.
fn token_bytes(token: &str) -> Result<Vec<u8>, Error> {
    let invalid = || Error::InvalidInput("invalid page token".into());
    if token.is_empty() || !token.len().is_multiple_of(2) || token.len() > 8192 {
        return Err(invalid());
    }
    (0..token.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&token[i..i + 2], 16).map_err(|_| invalid()))
        .collect()
}

fn token_string(bytes: &[u8]) -> Option<String> {
    if bytes.is_empty() {
        None
    } else {
        Some(bytes.iter().map(|b| format!("{b:02x}")).collect())
    }
}

/// Only the statuses the service knows; everything else is rejected before it reaches a query.
fn legacy_query(status: Option<&str>) -> Result<String, Error> {
    let query = match status.unwrap_or("All") {
        "All" => String::new(),
        "Running" | "Completed" | "Failed" | "Canceled" | "Terminated" | "TimedOut"
        | "ContinuedAsNew" | "Paused" => {
            format!("ExecutionStatus = '{}'", status.unwrap_or_default())
        }
        _ => return Err(Error::InvalidInput("invalid execution status".into())),
    };
    Ok(query)
}

/// Every service status that maps onto one [`RunStatus`]. See [`status_of`].
fn work_query(status: Option<RunStatus>) -> String {
    let names: &[&str] = match status {
        None => return String::new(),
        Some(RunStatus::Running) => &["Running", "ContinuedAsNew", "Paused"],
        Some(RunStatus::Completed) => &["Completed"],
        Some(RunStatus::Failed) => &["Failed", "TimedOut"],
        Some(RunStatus::Cancelled) => &["Canceled", "Terminated"],
    };
    names
        .iter()
        .map(|name| format!("ExecutionStatus = '{name}'"))
        .collect::<Vec<_>>()
        .join(" OR ")
}

fn legacy_status(code: i32) -> &'static str {
    match code {
        1 => "Running",
        2 => "Completed",
        3 => "Failed",
        4 => "Canceled",
        5 => "Terminated",
        6 => "ContinuedAsNew",
        7 => "TimedOut",
        8 => "Paused",
        _ => "Unknown",
    }
}

/// Continued-as-new and paused executions are still the same run, so they are running.
/// A timeout is a failure the run did not choose. Termination is an operator stopping the
/// run on purpose, which is what cancellation means to a caller.
fn status_of(code: i32) -> RunStatus {
    match code {
        2 => RunStatus::Completed,
        3 | 7 => RunStatus::Failed,
        4 | 5 => RunStatus::Cancelled,
        _ => RunStatus::Running,
    }
}

fn kind_of(workflow_type: &str) -> Option<RunKind> {
    match workflow_type {
        "turnkeel.run" | legacy::RUN => Some(RunKind::Run),
        "turnkeel.session" | legacy::SESSION => Some(RunKind::Session),
        "turnkeel.occurrence" => Some(RunKind::Occurrence),
        _ => None,
    }
}

async fn list(
    client: &Client,
    query: String,
    page: Option<&str>,
) -> Result<(Vec<Row>, Option<String>), Error> {
    let next_page_token = page.map(token_bytes).transpose()?.unwrap_or_default();
    let mut client = client.clone();
    let response = client
        .list_workflow_executions(
            ListWorkflowExecutionsRequest {
                namespace: client.namespace(),
                page_size: PAGE_SIZE,
                next_page_token,
                query,
            }
            .into_request(),
        )
        .await
        .map_err(|e| match e.code() {
            temporalio_client::tonic::Code::InvalidArgument => {
                Error::InvalidInput(e.message().to_owned())
            }
            _ => Error::Connection(e.to_string()),
        })?
        .into_inner();
    let rows = response
        .executions
        .into_iter()
        .filter_map(|info| {
            let execution = info.execution?;
            let start = info.start_time?;
            Some(Row {
                id: execution.workflow_id,
                attempt_id: execution.run_id,
                workflow_type: info.r#type.map(|t| t.name).unwrap_or_default(),
                status: info.status,
                started_at: millis(start.seconds, start.nanos)?,
                closed_at: info.close_time.and_then(|t| millis(t.seconds, t.nanos)),
            })
        })
        .collect();
    Ok((rows, token_string(&response.next_page_token)))
}

pub(crate) async fn work_history(
    client: &Client,
    status: Option<RunStatus>,
    page: Option<&str>,
) -> Result<WorkPage, Error> {
    let (rows, next_page) = list(client, work_query(status), page).await?;
    Ok(WorkPage {
        work: rows
            .into_iter()
            .filter_map(|row| {
                Some(WorkRecord {
                    id: row.id,
                    kind: kind_of(&row.workflow_type)?,
                    status: status_of(row.status),
                    started_at: row.started_at,
                    closed_at: row.closed_at,
                })
            })
            .collect(),
        next_page,
    })
}

#[allow(deprecated)]
pub(crate) async fn run_history(
    client: &Client,
    status: Option<&str>,
    page: Option<&str>,
) -> Result<RunPage, Error> {
    let (rows, next_page) = list(client, legacy_query(status)?, page).await?;
    Ok(RunPage {
        runs: rows
            .into_iter()
            .map(|row| RunRecord {
                id: row.id,
                run_id: row.attempt_id,
                kind: row.workflow_type,
                status: legacy_status(row.status).into(),
                started_at: row.started_at,
                closed_at: row.closed_at,
            })
            .collect(),
        next_page,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_and_tokens_are_bounded() {
        assert_eq!(
            legacy_query(Some("Failed")).unwrap(),
            "ExecutionStatus = 'Failed'"
        );
        assert!(matches!(
            legacy_query(Some("Failed' OR 1=1")),
            Err(Error::InvalidInput(_))
        ));
        assert_eq!(
            work_query(Some(RunStatus::Cancelled)),
            "ExecutionStatus = 'Canceled' OR ExecutionStatus = 'Terminated'"
        );
        assert_eq!(
            token_bytes(&token_string(&[0, 255, 16]).unwrap()).unwrap(),
            [0, 255, 16]
        );
        assert!(matches!(token_bytes("zz"), Err(Error::InvalidInput(_))));
    }

    #[test]
    fn every_service_status_has_one_meaning() {
        for code in 0..=8 {
            let status = status_of(code);
            let query = work_query(Some(status));
            assert!(
                code == 0 || query.contains(legacy_status(code)),
                "{code} maps to {status:?} but its filter omits it"
            );
        }
        assert_eq!(kind_of(legacy::RUN), Some(RunKind::Run));
        assert_eq!(kind_of("turnkeel.session"), Some(RunKind::Session));
        assert_eq!(kind_of("something-else"), None);
    }
}
