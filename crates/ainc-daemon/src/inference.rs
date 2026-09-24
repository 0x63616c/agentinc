//! Personal subscription model steps. OAuth remains owned by the official Codex
//! sign-in client; the Responses request and agent loop are ours.
use futures::{StreamExt, future::BoxFuture};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc};
use turnkeel::{Content, Model, ModelError, ModelRequest, ModelResponse, Role, StopReason};

const ENDPOINT: &str = "https://chatgpt.com/backend-api/codex/responses";
const PROVIDER: &str = "codex-responses-v1";
const MAX_RESPONSE: usize = 4 * 1024 * 1024;

#[derive(Clone)]
pub struct CodexModel {
    model: String,
    profile: PathBuf,
    http: reqwest::Client,
    // Serialize refreshes: the official client rotates the profile's credentials.
    auth_lock: Arc<tokio::sync::Mutex<()>>,
}

#[derive(Deserialize)]
struct AuthFile {
    tokens: Tokens,
}
#[derive(Deserialize)]
struct Tokens {
    access_token: String,
    account_id: String,
}

impl CodexModel {
    pub fn new(model: String, profile: PathBuf) -> Result<Self, ModelError> {
        if model.trim().is_empty() {
            return Err(ModelError::fatal("Select a model before starting work."));
        }
        Ok(Self {
            model,
            profile,
            http: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|_| ModelError::fatal("Could not create model transport."))?,
            auth_lock: Arc::default(),
        })
    }

    async fn credentials(&self) -> Result<Tokens, ModelError> {
        let _guard = self.auth_lock.lock().await;
        let profile = self.profile.clone();
        tokio::task::spawn_blocking(move || {
            let mut client = crate::codex::Client::start_at(&profile).map_err(|_| {
                ModelError::fatal("Codex sign-in is unavailable. Refresh the Connection.")
            })?;
            client
                .call("account/read", json!({"refreshToken":true}))
                .map_err(|_| ModelError::fatal("Refresh the ChatGPT Connection in Settings."))?;
            let data = std::fs::read(profile.join("auth.json")).map_err(|_| {
                ModelError::fatal("Sign in with a Codex file credential store for this profile.")
            })?;
            let auth: AuthFile = serde_json::from_slice(&data).map_err(|_| {
                ModelError::fatal("ChatGPT Connection credentials are unavailable.")
            })?;
            if auth.tokens.access_token.is_empty() || auth.tokens.account_id.is_empty() {
                return Err(ModelError::fatal("Sign in to ChatGPT in Settings."));
            }
            Ok(auth.tokens)
        })
        .await
        .map_err(|_| ModelError::fatal("Connection refresh stopped."))?
    }

    async fn request(
        &self,
        endpoint: &str,
        tokens: Tokens,
        request: ModelRequest,
    ) -> Result<ModelResponse, ModelError> {
        let response = self
            .http
            .post(endpoint)
            .bearer_auth(tokens.access_token)
            .header("ChatGPT-Account-Id", tokens.account_id)
            .header("originator", "agentinc")
            .header("accept", "text/event-stream")
            .json(&request_body(&self.model, request)?)
            .send()
            .await
            .map_err(|_| ModelError::retryable("Model transport is unavailable."))?;
        let status = response.status();
        if !status.is_success() {
            // Never return provider bodies or request headers (which may contain secrets).
            return Err(if status.as_u16() == 429 || status.is_server_error() {
                ModelError::retryable(format!("Model service returned {status}."))
            } else {
                ModelError::fatal(format!(
                    "Model request refused ({status}); check the Connection and model."
                ))
            });
        }
        let mut stream = response.bytes_stream();
        let mut data = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk =
                chunk.map_err(|_| ModelError::retryable("Model response was interrupted."))?;
            if data.len() + chunk.len() > MAX_RESPONSE {
                return Err(ModelError::fatal(
                    "Model response exceeded the supported size.",
                ));
            }
            data.extend_from_slice(&chunk);
        }
        parse_stream(&data)
    }
}

impl Model for CodexModel {
    fn id(&self) -> &str {
        &self.model
    }
    fn complete(
        &self,
        request: ModelRequest,
    ) -> BoxFuture<'static, Result<ModelResponse, ModelError>> {
        let model = self.clone();
        Box::pin(async move {
            let tokens = model.credentials().await?;
            model.request(ENDPOINT, tokens, request).await
        })
    }
}

fn request_body(model: &str, request: ModelRequest) -> Result<Value, ModelError> {
    let mut input = Vec::new();
    for message in request.messages {
        for block in message.content {
            input.push(match block {
                Content::Text { text } => json!({"role":match message.role { Role::User => "user", Role::Assistant => "assistant" }, "content":[{"type":if message.role == Role::User { "input_text" } else { "output_text" }, "text":text}]}),
                Content::ToolUse { id, name, input } => json!({"type":"function_call","call_id":id,"name":name,"arguments":input.to_string()}),
                Content::ToolResult { tool_use_id, content, is_error } => json!({"type":"function_call_output","call_id":tool_use_id,"output":json!({"content":content,"is_error":is_error}).to_string()}),
                Content::ModelContext { provider, value } if provider == PROVIDER => value,
                Content::ModelContext { .. } => return Err(ModelError::fatal("Conversation context belongs to another model provider.")),
            });
        }
    }
    let tools: Vec<_> = request.tools.into_iter().map(|tool| json!({"type":"function","name":tool.name,"description":tool.description,"parameters":tool.input_schema,"strict":false})).collect();
    Ok(
        json!({"model":model,"instructions":request.instructions,"input":input,"tools":tools,"tool_choice":"auto","parallel_tool_calls":false,"store":false,"stream":true,"include":["reasoning.encrypted_content"]}),
    )
}

fn parse_stream(bytes: &[u8]) -> Result<ModelResponse, ModelError> {
    let body = std::str::from_utf8(bytes)
        .map_err(|_| ModelError::fatal("Model response was not UTF-8."))?;
    let normalized = body.replace("\r\n", "\n");
    for event in normalized.split("\n\n") {
        let data = event
            .lines()
            .filter_map(|line| line.strip_prefix("data:").map(str::trim_start))
            .collect::<Vec<_>>()
            .join("\n");
        if data.is_empty() || data == "[DONE]" {
            continue;
        }
        let value: Value = serde_json::from_str(&data)
            .map_err(|_| ModelError::fatal("Model sent an invalid event."))?;
        match value["type"].as_str() {
            Some("response.completed") => return parse_output(&value["response"]),
            Some("response.failed" | "response.incomplete" | "error") => {
                return Err(ModelError::fatal("Model could not finish the response."));
            }
            _ => {}
        }
    }
    Err(ModelError::retryable(
        "Model stream ended before completion.",
    ))
}

fn parse_output(response: &Value) -> Result<ModelResponse, ModelError> {
    if response["status"] != "completed" {
        return Err(ModelError::fatal("Model response is not complete."));
    }
    let output = response["output"]
        .as_array()
        .ok_or_else(|| ModelError::fatal("Model output is missing."))?;
    let mut content = Vec::new();
    let mut stop_reason = StopReason::EndTurn;
    for item in output {
        match item["type"].as_str() {
            Some("message") => {
                let blocks = item["content"]
                    .as_array()
                    .ok_or_else(|| ModelError::fatal("Model message content is missing."))?;
                for block in blocks {
                    let text = match block["type"].as_str() {
                        Some("output_text") => block["text"].as_str(),
                        Some("refusal") => block["refusal"].as_str(),
                        _ => None,
                    }
                    .ok_or_else(|| ModelError::fatal("Model message content is unsupported."))?;
                    content.push(Content::Text { text: text.into() });
                }
            }
            Some("function_call") => {
                let field = |name| {
                    item[name]
                        .as_str()
                        .filter(|v| !v.is_empty())
                        .ok_or_else(|| ModelError::fatal("Model tool call is incomplete."))
                };
                content.push(Content::ToolUse {
                    id: field("call_id")?.into(),
                    name: field("name")?.into(),
                    input: serde_json::from_str(field("arguments")?)
                        .map_err(|_| ModelError::fatal("Model tool arguments are invalid."))?,
                });
                stop_reason = StopReason::ToolUse;
            }
            Some("reasoning") => content.push(Content::ModelContext {
                provider: PROVIDER.into(),
                value: item.clone(),
            }),
            _ => {
                return Err(ModelError::fatal(
                    "Model returned an unsupported output item.",
                ));
            }
        }
    }
    if !content
        .iter()
        .any(|c| matches!(c, Content::Text { .. } | Content::ToolUse { .. }))
    {
        return Err(ModelError::fatal("Model returned no reply or tool call."));
    }
    Ok(ModelResponse {
        content,
        stop_reason,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Json, Router,
        extract::State,
        http::{HeaderMap, StatusCode},
        routing::post,
    };
    use std::sync::Mutex;
    use turnkeel::{Agent, Message, Runtime, tool};

    fn tokens() -> Tokens {
        Tokens {
            access_token: "fixture-secret".into(),
            account_id: "fixture-account".into(),
        }
    }
    fn request() -> ModelRequest {
        ModelRequest {
            instructions: "Fixture instructions".into(),
            messages: vec![Message::user("hello")],
            tools: vec![],
        }
    }
    fn completed(output: Value) -> String {
        format!(
            "event: response.completed\ndata: {}\n\n",
            json!({"type":"response.completed","response":{"status":"completed","output":output}})
        )
    }
    fn text_output(text: &str) -> Value {
        json!([{"type":"message","content":[{"type":"output_text","text":text}]}])
    }

    #[test]
    fn stream_failures_are_typed_and_do_not_expose_provider_secrets() {
        assert!(!parse_stream(b"data: not-json\n\n").unwrap_err().retryable);
        assert!(
            parse_stream(b"data: {\"type\":\"response.created\"}\n\n")
                .unwrap_err()
                .retryable
        );
        let error =
            parse_stream(b"data: {\"type\":\"response.failed\",\"error\":\"fixture-secret\"}\n\n")
                .unwrap_err();
        assert!(!error.message.contains("fixture-secret"));
        assert!(!error.retryable);
        assert!(parse_output(&json!({"status":"completed","output":[{"type":"function_call","call_id":"c","name":"t","arguments":"{broken"}]})).is_err());
        assert!(
            parse_output(&json!({"status":"incomplete","output":text_output("partial")})).is_err()
        );
        assert_eq!(
            parse_stream(
                completed(text_output("héllo"))
                    .replace('\n', "\r\n")
                    .as_bytes()
            )
            .unwrap()
            .content,
            vec![Content::Text {
                text: "héllo".into()
            }]
        );
    }

    #[test]
    fn context_cannot_cross_provider_boundary() {
        let mut req = request();
        req.messages
            .push(Message::assistant(vec![Content::ModelContext {
                provider: "another".into(),
                value: json!({}),
            }]));
        assert!(!request_body("fixture", req).unwrap_err().retryable);
    }

    #[derive(Clone)]
    struct FixtureModel {
        model: CodexModel,
        endpoint: String,
    }
    impl Model for FixtureModel {
        fn id(&self) -> &str {
            "fixture"
        }
        fn complete(
            &self,
            request: ModelRequest,
        ) -> BoxFuture<'static, Result<ModelResponse, ModelError>> {
            let fixture = self.clone();
            Box::pin(async move {
                fixture
                    .model
                    .request(&fixture.endpoint, tokens(), request)
                    .await
            })
        }
    }
    /// Return a fixture result from the SDK's own tool loop.
    #[tool]
    async fn fixture_echo(value: String) -> anyhow::Result<String> {
        Ok(value)
    }

    #[tokio::test]
    async fn responses_adapter_runs_tools_in_our_sdk_and_preserves_context() -> anyhow::Result<()> {
        let seen = Arc::new(Mutex::new(Vec::<Value>::new()));
        let app = Router::new().route("/responses", post(|State(seen): State<Arc<Mutex<Vec<Value>>>>, headers: HeaderMap, Json(body): Json<Value>| async move {
            assert_eq!(headers["authorization"], "Bearer fixture-secret");
            assert_eq!(headers["chatgpt-account-id"], "fixture-account");
            let mut seen = seen.lock().unwrap();
            seen.push(body);
            let output = if seen.len() == 1 { json!([
                {"type":"reasoning","id":"r1","encrypted_content":"opaque-fixture","summary":[]},
                {"type":"function_call","call_id":"c1","name":"fixture_echo","arguments":"{\"value\":\"tool evidence\"}"}
            ]) } else { text_output("Finished with tool evidence") };
            ([("content-type", "text/event-stream")], completed(output))
        })).with_state(seen.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("http://{}/responses", listener.local_addr()?);
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let dir = tempfile::tempdir()?;
        let model = FixtureModel {
            model: CodexModel::new("fixture".into(), dir.path().into())?,
            endpoint,
        };
        let agent = Agent::builder("responses-fixture-v1")
            .model(model)
            .tool(fixture_echo)
            .build();
        let runtime = Runtime::test().await?;
        let run = runtime.start(&agent, "use the fixture").await?;
        assert_eq!(run.result().await?, "Finished with tool evidence");
        runtime.shutdown().await?;
        server.abort();
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0]["store"], false);
        assert_eq!(seen[0]["stream"], true);
        assert_eq!(seen[0]["tools"][0]["name"], "fixture_echo");
        assert_eq!(seen[1]["input"][1]["encrypted_content"], "opaque-fixture");
        assert_eq!(seen[1]["input"][2]["call_id"], "c1");
        assert_eq!(seen[1]["input"][3]["type"], "function_call_output");
        assert!(
            seen[1]["input"][3]["output"]
                .as_str()
                .unwrap()
                .contains("tool evidence")
        );
        Ok(())
    }

    #[tokio::test]
    async fn transport_redacts_errors_and_refuses_redirects() -> anyhow::Result<()> {
        let app = Router::new()
            .route(
                "/unauthorized",
                post(|| async { (StatusCode::UNAUTHORIZED, "fixture-secret") }),
            )
            .route(
                "/limited",
                post(|| async { (StatusCode::TOO_MANY_REQUESTS, "fixture-secret") }),
            )
            .route(
                "/redirect",
                post(|| async {
                    (
                        StatusCode::TEMPORARY_REDIRECT,
                        [("location", "https://example.invalid/leak")],
                        "",
                    )
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let url = format!("http://{}", listener.local_addr()?);
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let dir = tempfile::tempdir()?;
        let model = CodexModel::new("fixture".into(), dir.path().into())?;
        for (path, retryable) in [
            ("unauthorized", false),
            ("limited", true),
            ("redirect", false),
        ] {
            let error = model
                .request(&format!("{url}/{path}"), tokens(), request())
                .await
                .unwrap_err();
            assert_eq!(error.retryable, retryable);
            assert!(!error.message.contains("fixture-secret"));
        }
        server.abort();
        Ok(())
    }
}
