use crate::{
    Error, RuntimeConfig,
    runtime::{RunPage, RunRecord},
};
use temporalio_client::{
    Client, ClientOptions, ConnectionOptions, NamespacedClient, grpc::WorkflowService,
    tonic::IntoRequest,
};
use temporalio_common::protos::temporal::api::workflowservice::v1::ListWorkflowExecutionsRequest;

const PAGE_SIZE: i32 = 40;

#[derive(serde::Serialize, serde::Deserialize)]
struct Cursor {
    started_ns: i64,
    run_id: String,
}

fn token_bytes(token: &str) -> Result<Vec<u8>, Error> {
    if !token.len().is_multiple_of(2) || token.len() > 8192 {
        return Err(Error::Connection("invalid page token".into()));
    }
    (0..token.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&token[i..i + 2], 16)
                .map_err(|_| Error::Connection("invalid page token".into()))
        })
        .collect()
}

fn token_string(bytes: &[u8]) -> Option<String> {
    if bytes.is_empty() {
        None
    } else {
        Some(bytes.iter().map(|b| format!("{b:02x}")).collect())
    }
}

pub(crate) fn status_query(status: Option<&str>) -> Result<String, Error> {
    let query = match status.unwrap_or("All") {
        "All" => String::new(),
        "Running" | "Completed" | "Failed" | "Canceled" | "Terminated" | "TimedOut"
        | "ContinuedAsNew" | "Paused" => {
            format!("ExecutionStatus = '{}'", status.unwrap_or_default())
        }
        _ => return Err(Error::Connection("invalid execution status".into())),
    };
    Ok(query)
}

pub(crate) async fn list_workflows(
    config: &RuntimeConfig,
    status: Option<&str>,
    page: Option<&str>,
) -> Result<RunPage, Error> {
    let query = status_query(status)?;
    let cursor = page
        .map(|page| {
            serde_json::from_slice::<Cursor>(&token_bytes(page)?)
                .map_err(|_| Error::Connection("invalid page token".into()))
        })
        .transpose()?;
    let target: temporalio_client::Url = config
        .endpoint
        .parse()
        .map_err(|e| Error::Connection(format!("invalid runtime endpoint: {e}")))?;
    let mut client = Client::connect(
        ConnectionOptions::new(target)
            .identity("turnkeel-visibility")
            .build(),
        ClientOptions::new(config.scope.clone()).build(),
    )
    .await
    .map_err(|e| Error::Connection(e.to_string()))?;
    let namespace = client.namespace();
    let mut temporal_page = Vec::new();
    let mut rows = Vec::new();
    // Temporal's default order groups open executions first. A retained-history
    // scan lets one cursor page include the newest starts across every status.
    // ponytail: add a visibility snapshot if a large personal history makes this slow.
    loop {
        let response = client
            .list_workflow_executions(
                ListWorkflowExecutionsRequest {
                    namespace: namespace.clone(),
                    page_size: PAGE_SIZE,
                    next_page_token: temporal_page,
                    query: query.clone(),
                }
                .into_request(),
            )
            .await
            .map_err(|e| Error::Connection(e.to_string()))?
            .into_inner();
        rows.extend(response.executions.into_iter().filter_map(|info| {
            let execution = info.execution?;
            let start = info.start_time?;
            let started_ns = start
                .seconds
                .checked_mul(1_000_000_000)?
                .checked_add(i64::from(start.nanos))?;
            let started_at = start
                .seconds
                .checked_mul(1000)?
                .checked_add(i64::from(start.nanos) / 1_000_000)?;
            let closed_at = info.close_time.and_then(|t| {
                t.seconds
                    .checked_mul(1000)?
                    .checked_add(i64::from(t.nanos) / 1_000_000)
            });
            let status = match info.status {
                1 => "Running",
                2 => "Completed",
                3 => "Failed",
                4 => "Canceled",
                5 => "Terminated",
                6 => "ContinuedAsNew",
                7 => "TimedOut",
                8 => "Paused",
                _ => "Unknown",
            };
            Some((
                started_ns,
                RunRecord {
                    id: execution.workflow_id,
                    run_id: execution.run_id,
                    kind: info
                        .r#type
                        .map_or_else(|| "Workflow".to_owned(), |t| t.name),
                    status: status.into(),
                    started_at,
                    closed_at,
                },
            ))
        }));
        temporal_page = response.next_page_token;
        if temporal_page.is_empty() {
            break;
        }
    }
    rows.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.run_id.cmp(&a.1.run_id)));
    if let Some(cursor) = cursor {
        rows.retain(|(started_ns, execution)| {
            (*started_ns, execution.run_id.as_str()) < (cursor.started_ns, cursor.run_id.as_str())
        });
    }
    let next_page = if rows.len() > PAGE_SIZE as usize {
        let (started_ns, execution) = &rows[PAGE_SIZE as usize - 1];
        let bytes = serde_json::to_vec(&Cursor {
            started_ns: *started_ns,
            run_id: execution.run_id.clone(),
        })
        .map_err(|e| Error::Connection(e.to_string()))?;
        token_string(&bytes)
    } else {
        None
    };
    rows.truncate(PAGE_SIZE as usize);
    Ok(RunPage {
        runs: rows.into_iter().map(|(_, execution)| execution).collect(),
        next_page,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filters_and_tokens_are_bounded() {
        assert_eq!(
            status_query(Some("Failed")).unwrap(),
            "ExecutionStatus = 'Failed'"
        );
        assert!(status_query(Some("Failed' OR 1=1")).is_err());
        assert_eq!(
            token_bytes(&token_string(&[0, 255, 16]).unwrap()).unwrap(),
            [0, 255, 16]
        );
        assert!(token_bytes("zz").is_err());
    }
}
