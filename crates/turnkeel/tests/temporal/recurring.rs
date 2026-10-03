//! Recurring actions: a retained rule fires through the real server, a caller can
//! request an occurrence directly, and the action worker sees each one once.

use futures::future::BoxFuture;
use serde_json::json;
use std::{sync::Arc, time::Duration};
use turnkeel::{Occurrence, RecurringAction, RecurringRule, Runtime, testing::Server};

struct Record(tokio::sync::mpsc::UnboundedSender<Occurrence>);

impl RecurringAction for Record {
    fn execute(&self, occurrence: Occurrence) -> BoxFuture<'static, Result<(), turnkeel::Error>> {
        let _ = self.0.send(occurrence);
        Box::pin(async { Ok(()) })
    }
}

#[tokio::test]
async fn rule_fires_and_direct_occurrences_run_once() -> anyhow::Result<()> {
    let server = Server::start().await?;
    let (tx, mut seen) = tokio::sync::mpsc::unbounded_channel();
    let runtime = Runtime::recurring(server.config(), Arc::new(Record(tx))).await?;
    let rule = RecurringRule {
        id: "daily-review".into(),
        every: Duration::from_secs(3600),
        paused: false,
        input: json!({"rule": "daily-review"}),
    };
    runtime.apply_rule(rule.clone()).await?;
    assert!(
        runtime.apply_rule(rule.clone()).await.is_ok(),
        "applying the same rule again is a no-op"
    );
    server.fire_rule(&rule.id).await?;
    let fired = seen.recv().await.expect("fired occurrence");
    assert_eq!(fired.input, rule.input);
    assert!(fired.id.contains("daily-review"), "{}", fired.id);

    let manual = Occurrence {
        id: "manual-1".into(),
        input: json!({"rule": "daily-review", "manual": true}),
    };
    runtime.run_occurrence(manual.clone()).await?;
    runtime.run_occurrence(manual.clone()).await?;
    let ran = seen.recv().await.expect("manual occurrence");
    assert_eq!(ran.id, "manual-1");
    assert_eq!(ran.input, manual.input);

    let state = runtime.recurring_state(&rule.id).await?;
    assert!(!state.paused);
    assert!(state.recent.iter().any(|o| o.id == fired.id));

    runtime
        .apply_rule(RecurringRule {
            paused: true,
            ..rule.clone()
        })
        .await?;
    assert!(runtime.recurring_state(&rule.id).await?.paused);
    assert!(
        matches!(
            runtime
                .apply_rule(RecurringRule {
                    every: Duration::from_secs(1),
                    ..rule
                })
                .await,
            Err(turnkeel::Error::Connection(_))
        ),
        "rules under a minute are refused"
    );
    runtime.shutdown().await?;
    server.shutdown().await?;
    Ok(())
}
