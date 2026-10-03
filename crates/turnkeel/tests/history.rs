//! Retained work is reported in agent vocabulary: kind and status, no attempt ids.

use turnkeel::{
    Agent, Error, RunKind, RunStatus, Runtime, WorkRecord,
    testing::{Script, ScriptedModel, Server, text},
};

async fn work(
    observer: &Runtime,
    status: Option<RunStatus>,
    expected: usize,
) -> anyhow::Result<Vec<WorkRecord>> {
    // Visibility lags the history store by a moment; no timers, just keep asking.
    loop {
        let page = observer.work_history(status, None).await?;
        if page.work.len() >= expected {
            return Ok(page.work);
        }
        tokio::task::yield_now().await;
    }
}

#[tokio::test]
async fn work_history_reports_kind_and_status() -> anyhow::Result<()> {
    let server = Server::start().await?;
    let script = Script::new();
    let scripted = Agent::builder("history-v1")
        .model(ScriptedModel::new().otherwise(text("done")))
        .build();
    let gated = Agent::builder("history-gated-v1")
        .model(script.model())
        .build();
    let runtime = Runtime::configured(server.config(), &[scripted.clone(), gated.clone()]).await?;
    let completed = runtime.start(&scripted, "hello").await?;
    assert_eq!(completed.result().await?, "done");
    let cancelled = runtime.start(&gated, "wait").await?;
    let mut call = script.next_model_call().await;
    cancelled.cancel().await?;
    assert!(matches!(cancelled.result().await, Err(Error::Cancelled)));
    call.cancelled().await;
    let session = runtime.session(&gated).await?;

    let observer = Runtime::observer(server.config()).await?;
    let all = work(&observer, None, 3).await?;
    let find = |id: &str| {
        all.iter()
            .find(|w| w.id == id)
            .unwrap_or_else(|| panic!("{id} missing from {all:?}"))
            .clone()
    };
    let done = find(completed.id().as_str());
    assert_eq!(
        (done.kind, done.status),
        (RunKind::Run, RunStatus::Completed)
    );
    assert!(done.closed_at.is_some());
    let stopped = find(cancelled.id().as_str());
    assert_eq!(
        (stopped.kind, stopped.status),
        (RunKind::Run, RunStatus::Cancelled)
    );
    let open = find(session.id().as_str());
    assert_eq!(
        (open.kind, open.status),
        (RunKind::Session, RunStatus::Running)
    );
    assert_eq!(open.closed_at, None);

    let running = work(&observer, Some(RunStatus::Running), 1).await?;
    assert!(running.iter().all(|w| w.status == RunStatus::Running));
    assert!(running.iter().any(|w| w.id == session.id().as_str()));
    let only_completed = work(&observer, Some(RunStatus::Completed), 1).await?;
    assert_eq!(only_completed.len(), 1);
    assert_eq!(only_completed[0].id, completed.id().as_str());
    assert!(matches!(
        observer.work_history(None, Some("not-a-token")).await,
        Err(Error::InvalidInput(_))
    ));

    observer.shutdown().await?;
    runtime.shutdown().await?;
    server.shutdown().await?;
    Ok(())
}
