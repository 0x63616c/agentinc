use crate::{Tool, ToolCtx, ToolError};
use futures::future::BoxFuture;
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::Notify;

/// A tool that blocks until the test releases it. Holds a turn open at a known point so the
/// test can act mid-turn without timing assumptions.
///
/// ```ignore
/// let gate = Gate::new("wait");
/// let agent = Agent::builder("bot").model(model).tool(gate.clone()).build();
/// session.send("start").await?;   // model calls `wait`
/// gate.entered().await;            // the tool is now running
/// session.send("actually").await?;
/// gate.release();
/// ```
#[derive(Clone)]
pub struct Gate {
    inner: Arc<Inner>,
}

struct Inner {
    name: String,
    entered: Notify,
    release: Notify,
}

impl Gate {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            inner: Arc::new(Inner {
                name: name.into(),
                entered: Notify::new(),
                release: Notify::new(),
            }),
        }
    }

    /// Resolves once the model has called this tool and it is waiting.
    pub async fn entered(&self) {
        self.inner.entered.notified().await;
    }

    /// Let the waiting tool call return.
    pub fn release(&self) {
        self.inner.release.notify_one();
    }
}

impl Tool for Gate {
    fn name(&self) -> &str {
        &self.inner.name
    }

    fn description(&self) -> &str {
        "Waits until released."
    }

    fn schema(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }

    /// One attempt only: a second call would block forever waiting for a second release.
    fn idempotent(&self) -> bool {
        false
    }

    fn call(&self, _ctx: ToolCtx, _args: Value) -> BoxFuture<'static, Result<Value, ToolError>> {
        let inner = self.inner.clone();
        Box::pin(async move {
            inner.entered.notify_one();
            inner.release.notified().await;
            Ok(Value::String("released".into()))
        })
    }
}
