//! OpenRouter chat completions. The API key lives in the Keychain and is read
//! for each request; it never appears in agent definitions, records or errors.
use super::{ProviderModel, secrets::SecretStore, sse::SseReader};
use crate::execution::DeltaSink;
use futures::{StreamExt, future::BoxFuture};
use serde_json::{Value, json};
use std::sync::Arc;
use turnkeel::{Content, Model, ModelError, ModelRequest, ModelResponse, Role, StopReason};

const ENDPOINT: &str = "https://openrouter.ai/api/v1";
pub const SECRET: &str = "openrouter";
const MAX_RESPONSE: usize = 4 * 1024 * 1024;
/// Jev is the featured, inexpensive model used for real end-to-end checks.
pub const FEATURED: &[(&str, &str)] = &[
    ("typesafe/jev-router", "Jev Router"),
    ("typesafe/jev-latest", "Jev Latest"),
];
const VENDORS: &[&str] = &[
    "typesafe/",
    "openai/",
    "anthropic/",
    "google/",
    "x-ai/",
    "deepseek/",
    "moonshotai/",
    "mistralai/",
    "meta-llama/",
    "qwen/",
];

pub struct OpenRouter {
    endpoint: String,
    http: reqwest::Client,
    secrets: Arc<dyn SecretStore>,
}
#[derive(Clone, Debug)]
pub struct Account {
    pub label: String,
}
/// The key itself was refused, as opposed to the service being unreachable.
#[derive(Debug)]
pub struct Rejected;
impl std::fmt::Display for Rejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OpenRouter rejected the API key.")
    }
}
impl std::error::Error for Rejected {}
impl OpenRouter {
    pub fn new(secrets: Arc<dyn SecretStore>) -> Result<Self, ModelError> {
        Self::with_endpoint(ENDPOINT, secrets)
    }
    pub fn with_endpoint(
        endpoint: &str,
        secrets: Arc<dyn SecretStore>,
    ) -> Result<Self, ModelError> {
        Ok(Self {
            endpoint: endpoint.trim_end_matches('/').to_owned(),
            http: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(std::time::Duration::from_secs(600))
                .build()
                .map_err(|_| ModelError::fatal("Could not create model transport."))?,
            secrets,
        })
    }
    pub fn key(&self) -> Result<Option<String>, ModelError> {
        self.secrets
            .get(SECRET)
            .map_err(|error| ModelError::fatal(error.to_string()))
    }
    pub fn store_key(&self, key: &str) -> anyhow::Result<()> {
        let key = key.trim();
        anyhow::ensure!(
            key.len() >= 20 && key.chars().all(|c| c.is_ascii_graphic()),
            "Enter a valid OpenRouter API key."
        );
        self.secrets.set(SECRET, key)
    }
    pub fn forget_key(&self) -> anyhow::Result<()> {
        self.secrets.delete(SECRET)
    }
    /// Verify the stored key against the account endpoint.
    pub async fn account(&self) -> anyhow::Result<Option<Account>> {
        let Some(key) = self.key()? else {
            return Ok(None);
        };
        let response = self
            .http
            .get(format!("{}/key", self.endpoint))
            .bearer_auth(key)
            .send()
            .await
            .map_err(|_| anyhow::anyhow!("OpenRouter is unreachable."))?;
        if matches!(
            response.status(),
            reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN
        ) {
            return Err(Rejected.into());
        }
        if !response.status().is_success() {
            anyhow::bail!("OpenRouter returned {}.", response.status());
        }
        let value: Value = response
            .json()
            .await
            .map_err(|_| anyhow::anyhow!("OpenRouter sent an unreadable account."))?;
        let data = &value["data"];
        let label = data["label"].as_str().unwrap_or("API key").to_owned();
        let usage = data["usage"].as_f64().unwrap_or(0.0);
        let label = match data["limit"].as_f64() {
            Some(limit) => format!("{label} · ${usage:.2} of ${limit:.2} used"),
            None => format!("{label} · ${usage:.2} used"),
        };
        Ok(Some(Account { label }))
    }
    /// Curated model list: Jev first, then well-known vendors.
    pub async fn models(&self) -> anyhow::Result<Vec<ProviderModel>> {
        let response = self
            .http
            .get(format!("{}/models", self.endpoint))
            .send()
            .await
            .map_err(|_| anyhow::anyhow!("OpenRouter is unreachable."))?;
        if !response.status().is_success() {
            anyhow::bail!("OpenRouter returned {}.", response.status());
        }
        let value: Value = response
            .json()
            .await
            .map_err(|_| anyhow::anyhow!("OpenRouter sent an unreadable model list."))?;
        let mut models: Vec<ProviderModel> = value["data"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| {
                        let id = item["id"].as_str()?;
                        if !VENDORS.iter().any(|vendor| id.starts_with(vendor)) || id.contains(':')
                        {
                            return None;
                        }
                        Some(ProviderModel {
                            id: id.to_owned(),
                            name: item["name"].as_str().unwrap_or(id).to_owned(),
                            featured: FEATURED.iter().any(|(featured, _)| *featured == id),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        for (id, name) in FEATURED.iter().rev() {
            if !models.iter().any(|model| model.id == *id) {
                models.insert(
                    0,
                    ProviderModel {
                        id: (*id).into(),
                        name: (*name).into(),
                        featured: true,
                    },
                );
            }
        }
        let rank = |model: &ProviderModel| {
            FEATURED
                .iter()
                .position(|(id, _)| *id == model.id)
                .unwrap_or(FEATURED.len())
        };
        models.sort_by(|a, b| rank(a).cmp(&rank(b)).then_with(|| a.name.cmp(&b.name)));
        Ok(models)
    }
    pub fn model(
        self: &Arc<Self>,
        id: &str,
        sink: Option<DeltaSink>,
    ) -> Result<OpenRouterModel, ModelError> {
        if id.trim().is_empty() {
            return Err(ModelError::fatal("Select a model before starting work."));
        }
        Ok(OpenRouterModel {
            model: id.into(),
            provider: self.clone(),
            sink,
        })
    }
}

#[derive(Clone)]
pub struct OpenRouterModel {
    model: String,
    provider: Arc<OpenRouter>,
    sink: Option<DeltaSink>,
}
impl Model for OpenRouterModel {
    fn id(&self) -> &str {
        &self.model
    }
    fn complete(
        &self,
        request: ModelRequest,
    ) -> BoxFuture<'static, Result<ModelResponse, ModelError>> {
        let this = self.clone();
        Box::pin(async move {
            let key = this
                .provider
                .key()?
                .ok_or_else(|| ModelError::fatal("Connect OpenRouter in Settings."))?;
            this.request(&key, request).await
        })
    }
}
impl OpenRouterModel {
    async fn request(&self, key: &str, request: ModelRequest) -> Result<ModelResponse, ModelError> {
        let response = self
            .provider
            .http
            .post(format!("{}/chat/completions", self.provider.endpoint))
            .bearer_auth(key)
            .header("X-Title", "AgentInc")
            .header("accept", "text/event-stream")
            .json(&request_body(&self.model, request)?)
            .send()
            .await
            .map_err(|_| ModelError::retryable("Model transport is unavailable."))?;
        let status = response.status();
        if !status.is_success() {
            return Err(match status.as_u16() {
                401 | 403 => {
                    ModelError::fatal("OpenRouter rejected the API key. Reconnect it in Settings.")
                }
                402 => ModelError::fatal("OpenRouter credits are exhausted."),
                404 => ModelError::fatal("OpenRouter does not offer this model."),
                429 | 500..=599 => {
                    ModelError::retryable(format!("Model service returned {status}."))
                }
                _ => ModelError::fatal(format!(
                    "Model request refused ({status}); check the model."
                )),
            });
        }
        let mut stream = response.bytes_stream();
        let mut reader = SseReader::new(MAX_RESPONSE);
        let mut text = String::new();
        let mut calls: Vec<(String, String, String)> = Vec::new();
        let mut finish: Option<String> = None;
        while let Some(chunk) = stream.next().await {
            let chunk =
                chunk.map_err(|_| ModelError::retryable("Model response was interrupted."))?;
            for data in reader.push(&chunk)? {
                let value: Value = serde_json::from_str(&data)
                    .map_err(|_| ModelError::fatal("Model sent an invalid event."))?;
                if let Some(error) = value.get("error") {
                    let code = error["code"].as_u64().unwrap_or(0);
                    return Err(if matches!(code, 429 | 500..=599) {
                        ModelError::retryable("Model service is temporarily unavailable.")
                    } else {
                        ModelError::fatal("Model could not finish the response.")
                    });
                }
                let Some(choice) = value["choices"].as_array().and_then(|c| c.first()) else {
                    continue;
                };
                if let Some(delta) = choice["delta"]["content"].as_str()
                    && !delta.is_empty()
                {
                    text.push_str(delta);
                    if let Some(sink) = &self.sink {
                        sink(delta);
                    }
                }
                if let Some(parts) = choice["delta"]["tool_calls"].as_array() {
                    for part in parts {
                        let index = part["index"].as_u64().unwrap_or(calls.len() as u64) as usize;
                        while calls.len() <= index {
                            calls.push((String::new(), String::new(), String::new()));
                        }
                        let call = &mut calls[index];
                        if let Some(id) = part["id"].as_str() {
                            call.0 = id.into();
                        }
                        if let Some(name) = part["function"]["name"].as_str() {
                            call.1.push_str(name);
                        }
                        if let Some(arguments) = part["function"]["arguments"].as_str() {
                            call.2.push_str(arguments);
                        }
                    }
                }
                if let Some(reason) = choice["finish_reason"].as_str() {
                    finish = Some(reason.into());
                }
            }
        }
        let finish =
            finish.ok_or_else(|| ModelError::retryable("Model stream ended before completion."))?;
        if finish == "length" {
            return Err(ModelError::fatal("Model reply exceeded its output limit."));
        }
        if finish == "content_filter" {
            return Err(ModelError::fatal("Model declined to answer."));
        }
        let mut content = Vec::new();
        if !text.is_empty() {
            content.push(Content::Text { text });
        }
        for (index, (id, name, arguments)) in calls.into_iter().enumerate() {
            if name.is_empty() {
                return Err(ModelError::fatal("Model tool call is incomplete."));
            }
            let input = if arguments.trim().is_empty() {
                json!({})
            } else {
                serde_json::from_str(&arguments)
                    .map_err(|_| ModelError::fatal("Model tool arguments are invalid."))?
            };
            content.push(Content::ToolUse {
                id: if id.is_empty() {
                    format!("call_{index}")
                } else {
                    id
                },
                name,
                input,
            });
        }
        let stop_reason = if content.iter().any(|c| matches!(c, Content::ToolUse { .. })) {
            StopReason::ToolUse
        } else {
            StopReason::EndTurn
        };
        if content.is_empty() {
            return Err(ModelError::fatal("Model returned no reply or tool call."));
        }
        Ok(ModelResponse {
            content,
            stop_reason,
        })
    }
}

fn tool_result_text(content: &Value, is_error: bool) -> String {
    let body = match content {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    };
    if is_error {
        format!("Error: {body}")
    } else {
        body
    }
}

fn request_body(model: &str, request: ModelRequest) -> Result<Value, ModelError> {
    let mut messages = vec![json!({"role":"system","content":request.instructions})];
    for message in request.messages {
        match message.role {
            Role::User => {
                let mut text = String::new();
                for block in &message.content {
                    match block {
                        Content::Text { text: part } => text.push_str(part),
                        Content::ToolResult { tool_use_id, content, is_error } => messages.push(json!({"role":"tool","tool_call_id":tool_use_id,"content":tool_result_text(content,*is_error)})),
                        Content::ToolUse { .. } => return Err(ModelError::fatal("Conversation history is malformed.")),
                        Content::ModelContext { .. } => {}
                    }
                }
                if !text.is_empty() {
                    messages.push(json!({"role":"user","content":text}));
                }
            }
            Role::Assistant => {
                let mut text = String::new();
                let mut calls = Vec::new();
                for block in &message.content {
                    match block {
                        Content::Text { text: part } => text.push_str(part),
                        Content::ToolUse { id, name, input } => calls.push(json!({"id":id,"type":"function","function":{"name":name,"arguments":input.to_string()}})),
                        Content::ToolResult { .. } => return Err(ModelError::fatal("Conversation history is malformed.")),
                        Content::ModelContext { .. } => {}
                    }
                }
                let mut entry = json!({"role":"assistant","content": if text.is_empty() { Value::Null } else { Value::String(text) }});
                if !calls.is_empty() {
                    entry["tool_calls"] = Value::Array(calls);
                }
                messages.push(entry);
            }
        }
    }
    let tools: Vec<Value> = request
        .tools
        .into_iter()
        .map(|tool| json!({"type":"function","function":{"name":tool.name,"description":tool.description,"parameters":tool.input_schema}}))
        .collect();
    let mut body = json!({"model":model,"messages":messages,"stream":true});
    if !tools.is_empty() {
        body["tools"] = Value::Array(tools);
        body["tool_choice"] = json!("auto");
        body["parallel_tool_calls"] = json!(false);
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::secrets::MemoryStore;
    use axum::{Json, Router, extract::State, http::HeaderMap, routing::post};
    use std::sync::Mutex;
    use turnkeel::{Agent, Runtime, tool};

    fn chunk(delta: Value, finish: Option<&str>) -> String {
        format!(
            "data: {}\n\n",
            json!({"choices":[{"delta":delta,"finish_reason":finish}]})
        )
    }
    #[tool]
    async fn fixture_echo(value: String) -> anyhow::Result<String> {
        Ok(value)
    }

    async fn provider(
        app: Router,
    ) -> anyhow::Result<(Arc<OpenRouter>, tokio::task::JoinHandle<()>)> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("http://{}", listener.local_addr()?);
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let secrets = Arc::new(MemoryStore::default());
        secrets.set(SECRET, "sk-or-fixture-key-0123456789")?;
        Ok((
            Arc::new(OpenRouter::with_endpoint(&endpoint, secrets)?),
            server,
        ))
    }

    #[tokio::test]
    async fn streams_text_deltas_runs_tools_in_our_sdk_and_maps_history() -> anyhow::Result<()> {
        let seen = Arc::new(Mutex::new(Vec::<Value>::new()));
        let app = Router::new().route("/chat/completions", post(|State(seen): State<Arc<Mutex<Vec<Value>>>>, headers: HeaderMap, Json(body): Json<Value>| async move {
            assert_eq!(headers["authorization"], "Bearer sk-or-fixture-key-0123456789");
            let mut seen = seen.lock().unwrap();
            seen.push(body);
            let stream = if seen.len() == 1 {
                chunk(json!({"content":"Let me "}), None)
                    + &chunk(json!({"content":"check."}), None)
                    + &chunk(json!({"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"fixture_echo","arguments":"{\"val"}}]}), None)
                    + &chunk(json!({"tool_calls":[{"index":0,"function":{"arguments":"ue\":\"tool evidence\"}"}}]}), Some("tool_calls"))
                    + "data: [DONE]\n\n"
            } else {
                chunk(json!({"content":"Finished with tool evidence"}), Some("stop"))
            };
            ([("content-type", "text/event-stream")], stream)
        })).with_state(seen.clone());
        let (provider, server) = provider(app).await?;
        let deltas = Arc::new(Mutex::new(String::new()));
        let sink_target = deltas.clone();
        let sink: DeltaSink =
            Arc::new(move |delta: &str| sink_target.lock().unwrap().push_str(delta));
        let model = provider.model("typesafe/jev-router", Some(sink))?;
        let agent = Agent::builder("openrouter-fixture-v1")
            .model(model)
            .tool(fixture_echo)
            .build();
        let runtime = Runtime::test().await?;
        let run = runtime.start(&agent, "use the fixture").await?;
        assert_eq!(run.result().await?, "Finished with tool evidence");
        runtime.shutdown().await?;
        server.abort();
        assert_eq!(
            deltas.lock().unwrap().as_str(),
            "Let me check.Finished with tool evidence"
        );
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0]["model"], "typesafe/jev-router");
        assert_eq!(seen[0]["stream"], true);
        assert_eq!(seen[0]["messages"][0]["role"], "system");
        assert_eq!(seen[0]["tools"][0]["function"]["name"], "fixture_echo");
        assert_eq!(seen[1]["messages"][2]["role"], "assistant");
        assert_eq!(seen[1]["messages"][2]["content"], "Let me check.");
        assert_eq!(seen[1]["messages"][2]["tool_calls"][0]["id"], "call_1");
        assert_eq!(seen[1]["messages"][3]["role"], "tool");
        assert_eq!(seen[1]["messages"][3]["tool_call_id"], "call_1");
        assert!(
            seen[1]["messages"][3]["content"]
                .as_str()
                .unwrap()
                .contains("tool evidence")
        );
        Ok(())
    }

    #[tokio::test]
    async fn transport_errors_are_typed_and_never_carry_the_key() -> anyhow::Result<()> {
        let app = Router::new()
            .route(
                "/unauthorized/chat/completions",
                post(|| async {
                    (
                        reqwest::StatusCode::UNAUTHORIZED,
                        "sk-or-fixture-key-0123456789",
                    )
                }),
            )
            .route(
                "/limited/chat/completions",
                post(|| async { (reqwest::StatusCode::TOO_MANY_REQUESTS, "slow down") }),
            )
            .route(
                "/truncated/chat/completions",
                post(|| async {
                    (
                        [("content-type", "text/event-stream")],
                        chunk(json!({"content":"partial"}), Some("length")),
                    )
                }),
            )
            .route(
                "/silent/chat/completions",
                post(|| async {
                    (
                        [("content-type", "text/event-stream")],
                        chunk(json!({"content":"partial"}), None),
                    )
                }),
            );
        let (provider, server) = provider(app).await?;
        let key = provider.key()?.unwrap();
        for (path, retryable) in [
            ("unauthorized", false),
            ("limited", true),
            ("truncated", false),
            ("silent", true),
        ] {
            let scoped = Arc::new(OpenRouter::with_endpoint(
                &format!("{}/{path}", provider.endpoint),
                Arc::new(MemoryStore::default()),
            )?);
            let model = scoped.model("fixture", None)?;
            let error = model
                .request(
                    &key,
                    ModelRequest {
                        instructions: "t".into(),
                        messages: vec![turnkeel::Message::user("hi")],
                        tools: vec![],
                    },
                )
                .await
                .unwrap_err();
            assert_eq!(error.retryable, retryable, "{path}");
            assert!(!error.message.contains("fixture-key"));
        }
        server.abort();
        Ok(())
    }

    #[test]
    fn key_validation_and_model_curation() {
        let secrets = Arc::new(MemoryStore::default());
        let provider = OpenRouter::new(secrets.clone()).unwrap();
        assert!(provider.store_key("short").is_err());
        assert!(
            provider
                .store_key("  sk-or-v1-0123456789abcdef0123  ")
                .is_ok()
        );
        assert_eq!(
            secrets.get(SECRET).unwrap().as_deref(),
            Some("sk-or-v1-0123456789abcdef0123")
        );
        provider.forget_key().unwrap();
        assert!(provider.key().unwrap().is_none());
    }
}
