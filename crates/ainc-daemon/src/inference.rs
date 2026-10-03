//! Personal subscription model steps. Credentials come from the Connection; the
//! Responses request and agent loop are ours.
use crate::connection::{Connection, Tokens};
use eventsource_stream::Eventsource;
use futures::{Stream, StreamExt, future::BoxFuture};
use serde_json::{Value, json};
use std::sync::Arc;
use turnkeel::{Content, Model, ModelError, ModelRequest, ModelResponse, Role, StopReason};

const ENDPOINT: &str = "https://chatgpt.com/backend-api/codex/responses";
const PROVIDER: &str = "codex-responses-v1";
const MAX_RESPONSE: usize = 4 * 1024 * 1024;

#[derive(Clone)]
pub struct CodexModel {
    model: String,
    shared: CodexModels,
}

/// One personal Connection shared across selected models and agent definitions.
#[derive(Clone)]
pub struct CodexModels {
    connection: Connection,
    endpoint: String,
    http: reqwest::Client,
}
impl CodexModels {
    pub fn new(connection: Connection) -> Result<Self, ModelError> {
        let endpoint = ENDPOINT.to_owned();
        #[cfg(debug_assertions)]
        let endpoint = if let Ok(value) = std::env::var("AINC_TEST_RESPONSES_URL") {
            let url = reqwest::Url::parse(&value)
                .map_err(|_| ModelError::fatal("Invalid fixture URL."))?;
            if url.scheme() != "http"
                || url.host_str() != Some("127.0.0.1")
                || !url.username().is_empty()
                || url.password().is_some()
            {
                return Err(ModelError::fatal(
                    "Fixture transport must use loopback HTTP.",
                ));
            }
            value
        } else {
            endpoint
        };
        Ok(Self {
            connection,
            endpoint,
            http: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|_| ModelError::fatal("Could not create model transport."))?,
        })
    }
    fn model(&self, id: &str) -> Result<CodexModel, ModelError> {
        if id.trim().is_empty() {
            return Err(ModelError::fatal("Select a model before starting work."));
        }
        Ok(CodexModel {
            model: id.into(),
            shared: self.clone(),
        })
    }
}
impl crate::execution::ModelCatalog for CodexModels {
    fn resolve(&self, id: &str) -> Result<Arc<dyn Model>, ModelError> {
        Ok(Arc::new(self.model(id)?))
    }
}

impl CodexModel {
    async fn request(
        &self,
        endpoint: &str,
        tokens: Tokens,
        request: ModelRequest,
    ) -> Result<ModelResponse, ModelError> {
        let response = self
            .shared
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
        parse_stream(response.bytes_stream().map(|chunk| {
            chunk.map_err(|_| ModelError::retryable("Model response was interrupted."))
        }))
        .await
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
        let mut model = self.clone();
        Box::pin(async move {
            let connection = model.shared.connection.clone();
            let requested = model.model.clone();
            let (tokens, selected) =
                tokio::task::spawn_blocking(move || connection.credentials(&requested))
                    .await
                    .map_err(|_| ModelError::fatal("Connection refresh stopped."))??;
            model.model = selected;
            if model.shared.endpoint != ENDPOINT
                && (tokens.access_token != "fixture-access-token"
                    || tokens.account_id != "fixture-account")
            {
                return Err(ModelError::fatal(
                    "Fixture transport refuses real credentials.",
                ));
            }
            model.request(&model.shared.endpoint, tokens, request).await
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

/// Read a Responses SSE stream event by event until the response completes.
async fn parse_stream<B: AsRef<[u8]>>(
    body: impl Stream<Item = Result<B, ModelError>>,
) -> Result<ModelResponse, ModelError> {
    let mut seen = 0usize;
    let events = body
        .map(move |chunk| {
            let chunk = chunk?;
            seen += chunk.as_ref().len();
            if seen > MAX_RESPONSE {
                return Err(ModelError::fatal(
                    "Model response exceeded the supported size.",
                ));
            }
            Ok(chunk)
        })
        .eventsource();
    let mut events = std::pin::pin!(events);
    let mut output = Vec::new();
    while let Some(event) = events.next().await {
        let event = event.map_err(|error| match error {
            eventsource_stream::EventStreamError::Transport(error) => error,
            _ => ModelError::fatal("Model response was not a valid event stream."),
        })?;
        if event.data.is_empty() || event.data == "[DONE]" {
            continue;
        }
        let value: Value = serde_json::from_str(&event.data)
            .map_err(|_| ModelError::fatal("Model sent an invalid event."))?;
        match value["type"].as_str() {
            Some("response.output_item.done") => {
                let item = value["item"]
                    .as_object()
                    .ok_or_else(|| ModelError::fatal("Model output item is missing."))?;
                output.push(Value::Object(item.clone()));
            }
            Some("response.completed") => {
                // Codex delivers content in output_item.done events; the terminal
                // response may omit output or leave it empty. Do not append its
                // snapshot as well, which would duplicate replies and tool calls.
                let mut response = value["response"].clone();
                if !output.is_empty() {
                    let response = response
                        .as_object_mut()
                        .ok_or_else(|| ModelError::fatal("Model response is missing."))?;
                    response.insert("output".into(), Value::Array(output));
                }
                return parse_output(&response);
            }
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
    use turnkeel::{Agent, Event, Message, Runtime, tool};

    fn parsed(bytes: &[u8]) -> Result<ModelResponse, ModelError> {
        futures::executor::block_on(parse_stream(futures::stream::iter([Ok(bytes)])))
    }
    fn fixture_model(dir: &tempfile::TempDir) -> CodexModel {
        let connection = Connection::new(dir.path().into(), dir.path().join("absent"));
        CodexModels::new(connection)
            .unwrap()
            .model("fixture")
            .unwrap()
    }
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

    fn streamed(output: Value, response: Value) -> String {
        let mut stream = String::new();
        for item in output.as_array().unwrap() {
            stream.push_str(&format!(
                "data: {}\n\n",
                json!({"type":"response.output_item.done","item":item})
            ));
        }
        stream.push_str(&format!(
            "data: {}\n\n",
            json!({"type":"response.completed","response":response})
        ));
        stream
    }

    #[test]
    fn completed_stream_items_survive_an_empty_or_missing_terminal_output() {
        let output = json!([
            {"type":"reasoning","id":"r1","encrypted_content":"opaque-fixture","summary":[]},
            {"type":"message","content":[{"type":"output_text","text":"Checking your Tickets"}]},
            {"type":"function_call","call_id":"c1","name":"list_tickets","arguments":"{}"}
        ]);
        let expected = parse_output(&json!({"status":"completed","output":output})).unwrap();
        for response in [
            json!({"status":"completed","output":[]}),
            json!({"status":"completed"}),
            json!({"status":"completed","output":output}),
        ] {
            let actual = parsed(streamed(output.clone(), response).as_bytes()).unwrap();
            assert_eq!(actual.content, expected.content);
            assert_eq!(actual.stop_reason, StopReason::ToolUse);
        }
        let reply = parsed(
            streamed(
                text_output("héllo"),
                json!({"status":"completed","output":[]}),
            )
            .replace('\n', "\r\n")
            .as_bytes(),
        )
        .unwrap();
        assert_eq!(
            reply.content,
            vec![Content::Text {
                text: "héllo".into()
            }]
        );
        assert_eq!(reply.stop_reason, StopReason::EndTurn);
    }

    #[test]
    fn streamed_items_require_successful_completion() {
        let item = format!(
            "data: {}\n\n",
            json!({"type":"response.output_item.done","item":text_output("partial")[0]})
        );
        assert!(parsed(item.as_bytes()).unwrap_err().retryable);
        for kind in ["response.failed", "response.incomplete", "error"] {
            let stream = format!(
                "{item}data: {}\n\n",
                json!({"type":kind,"error":"fixture-secret"})
            );
            let error = parsed(stream.as_bytes()).unwrap_err();
            assert!(!error.retryable);
            assert!(!error.message.contains("fixture-secret"));
        }
        assert!(
            parsed(streamed(text_output("partial"), json!({"status":"incomplete"})).as_bytes())
                .is_err()
        );
        let added = format!(
            "data: {}\n\n{}",
            json!({"type":"response.output_item.added","item":text_output("partial")[0]}),
            completed(json!([]))
        );
        assert!(parsed(added.as_bytes()).is_err());
        assert!(parsed(b"data: {\"type\":\"response.output_item.done\"}\n\n").is_err());
    }

    #[test]
    fn stream_failures_are_typed_and_do_not_expose_provider_secrets() {
        assert!(!parsed(b"data: not-json\n\n").unwrap_err().retryable);
        assert!(
            parsed(b"data: {\"type\":\"response.created\"}\n\n")
                .unwrap_err()
                .retryable
        );
        let error =
            parsed(b"data: {\"type\":\"response.failed\",\"error\":\"fixture-secret\"}\n\n")
                .unwrap_err();
        assert!(!error.message.contains("fixture-secret"));
        assert!(!error.retryable);
        assert!(parse_output(&json!({"status":"completed","output":[{"type":"function_call","call_id":"c","name":"t","arguments":"{broken"}]})).is_err());
        assert!(
            parse_output(&json!({"status":"incomplete","output":text_output("partial")})).is_err()
        );
        assert_eq!(
            parsed(
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
            ([("content-type", "text/event-stream")], streamed(output, json!({"status":"completed","output":[]})))
        })).with_state(seen.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("http://{}/responses", listener.local_addr()?);
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let dir = tempfile::tempdir()?;
        let model = FixtureModel {
            model: fixture_model(&dir),
            endpoint,
        };
        let agent = Agent::builder("responses-fixture-v1")
            .model(model)
            .tool(fixture_echo)
            .build();
        let runtime = Runtime::test().await?;
        let session = runtime.session(&agent).await?;
        let mut events = session.events();
        for prompt in ["use the fixture", "follow up"] {
            session.send(prompt).await?;
            let mut reply = String::new();
            loop {
                match events.next().await.unwrap()? {
                    Event::Message(message) if message.role == Role::Assistant => {
                        reply.push_str(&message.text());
                    }
                    Event::TurnEnded => break,
                    _ => {}
                }
            }
            assert_eq!(reply, "Finished with tool evidence");
        }
        session.cancel().await?;
        runtime.shutdown().await?;
        server.abort();
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 3);
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
        assert_eq!(
            seen[2]["input"][4]["content"][0]["text"],
            "Finished with tool evidence"
        );
        assert_eq!(seen[2]["input"][5]["content"][0]["text"], "follow up");
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
        let model = fixture_model(&dir);
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
