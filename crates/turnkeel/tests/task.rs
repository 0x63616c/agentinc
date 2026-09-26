//! Durable tasks: retried until they succeed, stopped by a permanent error,
//! and started at most once per ID.
use futures::future::BoxFuture;
use serde_json::json;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use turnkeel::{Runtime, Task, TaskError, TaskHandler, testing::Server};

/// Records every attempt and answers from a script keyed by task ID.
struct Handler {
    attempts: mpsc::UnboundedSender<(String, usize)>,
    counts: Mutex<std::collections::HashMap<String, usize>>,
}
impl TaskHandler for Handler {
    fn run(&self, task: Task) -> BoxFuture<'static, Result<(), TaskError>> {
        let attempt = {
            let mut counts = self.counts.lock().unwrap();
            let count = counts.entry(task.id.clone()).or_default();
            *count += 1;
            *count
        };
        let _ = self.attempts.send((task.id.clone(), attempt));
        let outcome = match (task.input["behaviour"].as_str(), attempt) {
            (Some("flaky"), 1) => Err(TaskError::Retry("not yet".into())),
            (Some("broken"), _) => Err(TaskError::Permanent("never".into())),
            _ => Ok(()),
        };
        Box::pin(async move { outcome })
    }
}

async fn status(config: &turnkeel::RuntimeConfig, id: &str) -> Option<String> {
    Runtime::run_history(config, None, None)
        .await
        .ok()?
        .runs
        .into_iter()
        .find(|run| run.id == id)
        .map(|run| run.status)
}

#[tokio::test]
async fn tasks_retry_until_done_stop_on_permanent_errors_and_start_once() {
    let server = Server::start().await.unwrap();
    let (sender, mut attempts) = mpsc::unbounded_channel();
    let runtime = Runtime::tasks(
        server.config(),
        Arc::new(Handler {
            attempts: sender,
            counts: Mutex::default(),
        }),
    )
    .await
    .unwrap();

    let flaky = Task {
        id: "task-flaky".into(),
        input: json!({"behaviour": "flaky"}),
    };
    runtime.start_task(flaky.clone()).await.unwrap();
    // Starting the same ID again attaches instead of running it twice.
    runtime.start_task(flaky).await.unwrap();
    assert_eq!(attempts.recv().await, Some(("task-flaky".into(), 1)));
    assert_eq!(attempts.recv().await, Some(("task-flaky".into(), 2)));

    runtime
        .start_task(Task {
            id: "task-broken".into(),
            input: json!({"behaviour": "broken"}),
        })
        .await
        .unwrap();
    assert_eq!(attempts.recv().await, Some(("task-broken".into(), 1)));
    let config = server.config();
    loop {
        match status(&config, "task-broken").await.as_deref() {
            Some("Failed") => break,
            _ => tokio::task::yield_now().await,
        }
    }
    loop {
        match status(&config, "task-flaky").await.as_deref() {
            Some("Completed") => break,
            _ => tokio::task::yield_now().await,
        }
    }
    assert!(
        attempts.try_recv().is_err(),
        "a permanent error is not retried"
    );
    runtime.shutdown().await.unwrap();
    server.shutdown().await.unwrap();
}
