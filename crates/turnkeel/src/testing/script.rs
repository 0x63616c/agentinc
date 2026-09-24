use crate::{Model, ModelError, ModelRequest, ModelResponse, Tool, ToolCtx, ToolError};
use futures::future::BoxFuture;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot};

/// Drives a model and its tools from the test, one call at a time.
///
/// Every model call and every tool call blocks until the test answers it, so the test
/// controls order, timing, and outcome without any timers.
#[derive(Clone, Default)]
pub struct Script {
    inner: Arc<Inner>,
}

#[derive(Default)]
struct Inner {
    model_calls: Channel<ModelCall>,
    tool_calls: Channel<ToolCall>,
    /// Results already given, by idempotency key. The test runtime calls idempotent tools
    /// twice; the second call gets the same answer without asking the test again.
    answered: Mutex<HashMap<String, Value>>,
}

struct Channel<T> {
    tx: mpsc::UnboundedSender<T>,
    rx: tokio::sync::Mutex<mpsc::UnboundedReceiver<T>>,
}

impl<T> Default for Channel<T> {
    fn default() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            tx,
            rx: tokio::sync::Mutex::new(rx),
        }
    }
}

impl<T> Channel<T> {
    async fn next(&self) -> T {
        self.rx
            .lock()
            .await
            .recv()
            .await
            .expect("Script dropped while a test was waiting on it")
    }
}

impl Script {
    pub fn new() -> Self {
        Self::default()
    }

    /// A model whose every reply the test provides through [`Script::next_model_call`].
    pub fn model(&self) -> ScriptModel {
        ScriptModel {
            inner: self.inner.clone(),
        }
    }

    /// A tool whose every result the test provides through [`Script::next_tool_call`].
    pub fn tool(&self, name: impl Into<String>) -> ScriptTool {
        ScriptTool {
            name: name.into(),
            inner: self.inner.clone(),
        }
    }

    /// Wait for the agent to call the model.
    pub async fn next_model_call(&self) -> ModelCall {
        self.inner.model_calls.next().await
    }

    /// Wait for the agent to call any scripted tool.
    pub async fn next_tool_call(&self) -> ToolCall {
        self.inner.tool_calls.next().await
    }
}

/// A model call waiting for its reply.
pub struct ModelCall {
    request: ModelRequest,
    reply: oneshot::Sender<ModelResponse>,
}

impl ModelCall {
    /// Wait until cancellation drops this model call's receiver.
    pub async fn cancelled(&mut self) {
        self.reply.closed().await;
    }

    pub fn request(&self) -> &ModelRequest {
        &self.request
    }

    pub fn reply(self, response: ModelResponse) {
        let _ = self.reply.send(response);
    }
}

/// A tool call waiting for its result.
pub struct ToolCall {
    name: String,
    args: Value,
    key: String,
    reply: oneshot::Sender<Value>,
    inner: Arc<Inner>,
}

impl ToolCall {
    /// Wait until cancellation drops this tool call's receiver.
    pub async fn cancelled(&mut self) {
        self.reply.closed().await;
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn args(&self) -> &Value {
        &self.args
    }

    pub fn succeed(self, result: Value) {
        self.inner
            .answered
            .lock()
            .unwrap()
            .insert(self.key, result.clone());
        let _ = self.reply.send(result);
    }
}

#[derive(Clone)]
pub struct ScriptModel {
    inner: Arc<Inner>,
}

impl Model for ScriptModel {
    fn id(&self) -> &str {
        "script"
    }

    fn complete(
        &self,
        request: ModelRequest,
    ) -> BoxFuture<'static, Result<ModelResponse, ModelError>> {
        let (tx, rx) = oneshot::channel();
        let _ = self
            .inner
            .model_calls
            .tx
            .send(ModelCall { request, reply: tx });
        Box::pin(async move {
            rx.await
                .map_err(|_| ModelError::fatal("Script dropped before replying"))
        })
    }
}

#[derive(Clone)]
pub struct ScriptTool {
    name: String,
    inner: Arc<Inner>,
}

impl Tool for ScriptTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "Scripted by the test."
    }

    fn schema(&self) -> Value {
        json!({ "type": "object" })
    }

    fn call(&self, ctx: ToolCtx, args: Value) -> BoxFuture<'static, Result<Value, ToolError>> {
        let key = ctx.idempotency_key().to_owned();
        if let Some(answer) = self.inner.answered.lock().unwrap().get(&key) {
            let answer = answer.clone();
            return Box::pin(async move { Ok(answer) });
        }
        let (tx, rx) = oneshot::channel();
        let _ = self.inner.tool_calls.tx.send(ToolCall {
            name: self.name.clone(),
            args,
            key,
            reply: tx,
            inner: self.inner.clone(),
        });
        Box::pin(async move {
            rx.await
                .map_err(|_| ToolError::Failed("Script dropped before answering".into()))
        })
    }
}
