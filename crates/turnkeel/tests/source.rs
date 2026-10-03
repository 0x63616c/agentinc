use futures::future::BoxFuture;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use turnkeel::{
    Agent, AgentSource, Error, ModelResponse, Runtime,
    testing::{Script, Server},
};

/// Knows one agent, `lazy-v1`, and counts how often it is asked for anything.
#[derive(Clone)]
struct Source {
    script: Script,
    lookups: Arc<AtomicUsize>,
}

impl AgentSource for Source {
    fn resolve(&self, name: &str) -> BoxFuture<'static, Result<Option<Agent>, Error>> {
        self.lookups.fetch_add(1, Ordering::SeqCst);
        let agent = (name == "lazy-v1")
            .then(|| Agent::builder("lazy-v1").model(self.script.model()).build());
        Box::pin(async move { Ok(agent) })
    }
}

#[tokio::test]
async fn worker_resolves_unknown_agents_from_its_source_once() -> anyhow::Result<()> {
    let server = Server::start().await?;
    let script = Script::new();
    let lookups = Arc::new(AtomicUsize::new(0));
    let worker = Runtime::configured_with(
        server.config(),
        Source {
            script: script.clone(),
            lookups: lookups.clone(),
        },
    )
    .await?;
    // Started elsewhere: the worker has never seen this agent and must look it up.
    let control = Runtime::observer(server.config()).await?;
    let agent = Agent::builder("lazy-v1").model(script.model()).build();
    let run = control.start(&agent, "first").await?;
    script
        .next_model_call()
        .await
        .reply(ModelResponse::text("one"));
    assert_eq!(run.result().await?, "one");
    let again = control.start(&agent, "second").await?;
    script
        .next_model_call()
        .await
        .reply(ModelResponse::text("two"));
    assert_eq!(again.result().await?, "two");
    assert_eq!(
        lookups.load(Ordering::SeqCst),
        1,
        "resolved once, then cached"
    );

    let unknown = Agent::builder("unknown-v1").model(script.model()).build();
    let error = control
        .start(&unknown, "hello")
        .await?
        .result()
        .await
        .unwrap_err();
    match error {
        Error::RunFailed(message) => {
            assert!(message.contains("unknown-v1"), "{message}");
            assert!(message.contains("no definition"), "{message}");
        }
        other => panic!("expected a failed run, got {other:?}"),
    }

    control.shutdown().await?;
    worker.shutdown().await?;
    server.shutdown().await?;
    Ok(())
}
