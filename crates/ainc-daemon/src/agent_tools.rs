//! Evee's outward-facing tools: HTTP requests under an explicit host policy and
//! a read-only view of durable runs. Request and response bodies are recorded
//! as Conversation steps so the user sees exactly what was sent and received.
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use std::{net::IpAddr, time::Duration};
use turnkeel::{RuntimeConfig, Tool, ToolCtx, ToolError};

const TIMEOUT: Duration = Duration::from_secs(20);
const MAX_BODY: usize = 64 * 1024;
const MAX_REQUEST_BODY: usize = 256 * 1024;

/// Which hosts the agent may call. Patterns are exact hosts or `*.suffix`;
/// `*` allows every public host. Loopback, private and link-local addresses
/// are always refused, whatever the policy says.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct HttpPolicy {
    pub allow: Vec<String>,
    pub deny: Vec<String>,
}
impl Default for HttpPolicy {
    fn default() -> Self {
        Self {
            allow: vec!["*".into()],
            deny: Vec::new(),
        }
    }
}
impl HttpPolicy {
    pub async fn load(pool: &PgPool, workspace: &str) -> Result<Self, sqlx::Error> {
        let stored: Option<String> = sqlx::query_scalar(
            "SELECT value FROM assistant_settings WHERE workspace_id=$1 AND key='http_policy'",
        )
        .bind(workspace)
        .fetch_optional(pool)
        .await?;
        Ok(stored
            .and_then(|value| serde_json::from_str(&value).ok())
            .unwrap_or_default())
    }
    pub fn normalized(mut self) -> Self {
        for list in [&mut self.allow, &mut self.deny] {
            list.retain(|p| !p.trim().is_empty());
            for pattern in list.iter_mut() {
                *pattern = pattern.trim().to_ascii_lowercase();
            }
        }
        self
    }
    pub fn permits(&self, host: &str) -> Result<(), String> {
        let host = host.to_ascii_lowercase();
        if self.deny.iter().any(|pattern| matches(pattern, &host)) {
            return Err(format!("{host} is denied by the HTTP policy."));
        }
        if self.allow.iter().any(|pattern| matches(pattern, &host)) {
            return Ok(());
        }
        Err(format!("{host} is not in the HTTP allow list."))
    }
}
fn matches(pattern: &str, host: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(suffix) = pattern.strip_prefix("*.") {
        return host == suffix || host.ends_with(&format!(".{suffix}"));
    }
    pattern == host
}
fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            !(v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_multicast()
                || v4.is_unspecified()
                || v4.is_documentation()
                || v4.octets()[0] == 100 && (64..=127).contains(&v4.octets()[1])
                || v4.octets()[0] == 0)
        }
        IpAddr::V6(v6) => {
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
                || v6
                    .to_ipv4_mapped()
                    .is_some_and(|v4| !is_public(IpAddr::V4(v4))))
        }
    }
}

#[derive(Clone)]
pub struct HttpTool {
    pub pool: PgPool,
    pub workspace: String,
}
#[derive(Deserialize)]
struct HttpArgs {
    method: String,
    url: String,
    #[serde(default)]
    headers: serde_json::Map<String, Value>,
    #[serde(default)]
    body: Option<String>,
}
impl Tool for HttpTool {
    fn name(&self) -> &str {
        "http_request"
    }
    fn description(&self) -> &str {
        "Send one HTTP request to a public web API or page and read the response. Use it for anything on the web: JSON APIs, RSS, status pages. Requests follow the workspace HTTP policy, time out after 20 seconds, never follow redirects, and return at most 64 KB of body text."
    }
    fn schema(&self) -> Value {
        json!({
            "type":"object",
            "properties":{
                "method":{"type":"string","enum":["GET","POST","PUT","PATCH","DELETE","HEAD"]},
                "url":{"type":"string","description":"Absolute http(s) URL"},
                "headers":{"type":"object","additionalProperties":{"type":"string"},"description":"Request headers such as Accept or Content-Type"},
                "body":{"type":"string","description":"Request body for POST, PUT and PATCH"}
            },
            "required":["method","url"],
            "additionalProperties":false
        })
    }
    fn idempotent(&self) -> bool {
        false
    }
    fn call(&self, _ctx: ToolCtx, args: Value) -> BoxFuture<'static, Result<Value, ToolError>> {
        let this = self.clone();
        Box::pin(async move {
            let args: HttpArgs = serde_json::from_value(args)
                .map_err(|e| ToolError::InvalidArguments(format!("Invalid request: {e}")))?;
            let policy = HttpPolicy::load(&this.pool, &this.workspace)
                .await
                .map_err(|_| ToolError::Failed("HTTP policy is unavailable.".into()))?;
            request(&policy, args, false).await
        })
    }
}
/// `allow_private` exists only so tests can reach a loopback fixture; the tool
/// always passes `false`.
async fn request(
    policy: &HttpPolicy,
    args: HttpArgs,
    allow_private: bool,
) -> Result<Value, ToolError> {
    let method = reqwest::Method::from_bytes(args.method.to_ascii_uppercase().as_bytes())
        .map_err(|_| ToolError::InvalidArguments("Unsupported HTTP method.".into()))?;
    if !matches!(
        method,
        reqwest::Method::GET
            | reqwest::Method::POST
            | reqwest::Method::PUT
            | reqwest::Method::PATCH
            | reqwest::Method::DELETE
            | reqwest::Method::HEAD
    ) {
        return Err(ToolError::InvalidArguments(
            "Unsupported HTTP method.".into(),
        ));
    }
    let url = reqwest::Url::parse(args.url.trim())
        .map_err(|_| ToolError::InvalidArguments("Use an absolute http(s) URL.".into()))?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(ToolError::InvalidArguments(
            "Use an http(s) URL without embedded credentials.".into(),
        ));
    }
    let host = url
        .host_str()
        .ok_or_else(|| ToolError::InvalidArguments("The URL has no host.".into()))?
        .trim_matches(['[', ']'])
        .to_owned();
    policy.permits(&host).map_err(ToolError::InvalidArguments)?;
    let port = url.port_or_known_default().unwrap_or(443);
    let addresses: Vec<std::net::SocketAddr> = match host.parse::<IpAddr>() {
        Ok(ip) => vec![(ip, port).into()],
        Err(_) => tokio::time::timeout(
            Duration::from_secs(5),
            tokio::net::lookup_host((host.as_str(), port)),
        )
        .await
        .map_err(|_| ToolError::Failed(format!("{host} did not resolve in time.")))?
        .map_err(|_| ToolError::Failed(format!("{host} could not be resolved.")))?
        .collect(),
    };
    if addresses.is_empty() {
        return Err(ToolError::Failed(format!("{host} has no addresses.")));
    }
    if let Some(private) = addresses
        .iter()
        .find(|a| !allow_private && !is_public(a.ip()))
    {
        return Err(ToolError::InvalidArguments(format!(
            "{host} resolves to a private address ({}), which agents may not call.",
            private.ip()
        )));
    }
    if args
        .body
        .as_ref()
        .is_some_and(|b| b.len() > MAX_REQUEST_BODY)
    {
        return Err(ToolError::InvalidArguments(
            "Request body is larger than 256 KB.".into(),
        ));
    }
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(TIMEOUT)
        .user_agent(format!("AgentInc/{}", ainc_release::VERSION))
        .resolve_to_addrs(&host, &addresses)
        .build()
        .map_err(|_| ToolError::Failed("HTTP client is unavailable.".into()))?;
    let mut builder = client.request(method.clone(), url.clone());
    for (name, value) in &args.headers {
        let lower = name.to_ascii_lowercase();
        if matches!(
            lower.as_str(),
            "host" | "content-length" | "transfer-encoding" | "connection"
        ) {
            continue;
        }
        let Some(value) = value.as_str() else {
            return Err(ToolError::InvalidArguments(format!(
                "Header {name} must be a string."
            )));
        };
        builder = builder.header(name, value);
    }
    if let Some(body) = args.body.clone()
        && !matches!(method, reqwest::Method::GET | reqwest::Method::HEAD)
    {
        builder = builder.body(body);
    }
    let started = std::time::Instant::now();
    let response = builder.send().await.map_err(|error| {
        ToolError::Failed(if error.is_timeout() {
            format!("{host} did not answer within 20 seconds.")
        } else {
            format!("Request to {host} failed: connection error.")
        })
    })?;
    let status = response.status();
    let mut headers = serde_json::Map::new();
    for name in ["content-type", "content-length", "location", "retry-after"] {
        if let Some(value) = response.headers().get(name).and_then(|v| v.to_str().ok()) {
            headers.insert(name.into(), json!(value));
        }
    }
    let bytes = read_capped(response).await?;
    let truncated = bytes.len() > MAX_BODY;
    let body = String::from_utf8_lossy(&bytes[..bytes.len().min(MAX_BODY)]).into_owned();
    Ok(json!({
        "request": {"method": method.as_str(), "url": url.as_str(), "headers": args.headers, "body": args.body},
        "status": status.as_u16(),
        "ok": status.is_success(),
        "headers": headers,
        "body": body,
        "truncated": truncated,
        "elapsed_ms": started.elapsed().as_millis() as u64
    }))
}
async fn read_capped(response: reqwest::Response) -> Result<Vec<u8>, ToolError> {
    use futures::StreamExt;
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| ToolError::Failed("Response was interrupted.".into()))?;
        bytes.extend_from_slice(&chunk);
        if bytes.len() > MAX_BODY {
            bytes.truncate(MAX_BODY + 1);
            break;
        }
    }
    Ok(bytes)
}

/// Read-only view of every durable run: Conversation turns, Ticket work and Automations.
#[derive(Clone)]
pub struct RunsTool {
    pub config: RuntimeConfig,
}
impl Tool for RunsTool {
    fn name(&self) -> &str {
        "list_runs"
    }
    fn description(&self) -> &str {
        "List durable runs, newest first: Conversation turns, assigned Ticket work and Automation occurrences, with status and timing. Filter by status (Running, Completed, Failed, Canceled, Terminated, TimedOut)."
    }
    fn schema(&self) -> Value {
        json!({"type":"object","properties":{"status":{"type":"string","enum":["Running","Completed","Failed","Canceled","Terminated","TimedOut"]},"limit":{"type":"integer","minimum":1,"maximum":50}},"additionalProperties":false})
    }
    fn call(&self, _ctx: ToolCtx, args: Value) -> BoxFuture<'static, Result<Value, ToolError>> {
        let config = self.config.clone();
        Box::pin(async move {
            let status = args["status"].as_str().map(str::to_owned);
            let limit = args["limit"].as_u64().unwrap_or(20).clamp(1, 50) as usize;
            let page = turnkeel::Runtime::run_history(&config, status.as_deref(), None)
                .await
                .map_err(|_| ToolError::Failed("Run history is unavailable.".into()))?;
            let runs: Vec<Value> = page
                .runs
                .into_iter()
                .take(limit)
                .map(|run| json!({"id":run.id,"kind":run.kind,"status":run.status,"started_at":run.started_at,"closed_at":run.closed_at}))
                .collect();
            Ok(json!({"runs":runs}))
        })
    }
}

/// The tools every Conversation agent receives, built once per turn.
pub struct Toolbox {
    pub pool: PgPool,
    pub config: RuntimeConfig,
}
impl Toolbox {
    pub fn http(&self, workspace: &str) -> HttpTool {
        HttpTool {
            pool: self.pool.clone(),
            workspace: workspace.into(),
        }
    }
    pub fn runs(&self) -> RunsTool {
        RunsTool {
            config: self.config.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, routing::get};

    #[test]
    fn policy_matches_hosts_and_refuses_private_ranges() {
        let policy = HttpPolicy {
            allow: vec!["*.example.com".into(), "api.test".into()],
            deny: vec!["private.example.com".into()],
        };
        assert!(policy.permits("api.example.com").is_ok());
        assert!(policy.permits("example.com").is_ok());
        assert!(policy.permits("API.TEST").is_ok());
        assert!(policy.permits("private.example.com").is_err());
        assert!(policy.permits("other.test").is_err());
        assert!(HttpPolicy::default().permits("anything.test").is_ok());
        for ip in [
            "127.0.0.1",
            "10.1.2.3",
            "192.168.0.1",
            "169.254.169.254",
            "100.64.0.1",
            "::1",
            "fd00::1",
            "::ffff:10.0.0.1",
        ] {
            assert!(!is_public(ip.parse().unwrap()), "{ip}");
        }
        assert!(is_public("1.1.1.1".parse().unwrap()));
        assert!(is_public("2606:4700::1111".parse().unwrap()));
    }

    #[tokio::test]
    async fn requests_are_recorded_capped_and_private_addresses_refused() -> anyhow::Result<()> {
        use axum::{body::Bytes, http::HeaderMap, routing::post};
        let app = Router::new()
            .route(
                "/ok",
                get(|| async { ([("content-type", "text/plain")], "hello") }),
            )
            .route(
                "/big",
                get(|| async { ([("content-type", "text/plain")], "x".repeat(70 * 1024)) }),
            )
            .route(
                "/echo",
                post(|headers: HeaderMap, body: Bytes| async move {
                    format!(
                        "{}|{}",
                        headers["x-fixture"].to_str().unwrap(),
                        String::from_utf8_lossy(&body)
                    )
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let policy = HttpPolicy::default();
        let args = |method: &str, path: &str, body: Option<&str>| HttpArgs {
            method: method.into(),
            url: format!("http://{address}/{path}"),
            headers: serde_json::from_value(json!({"x-fixture": "seen"})).unwrap(),
            body: body.map(str::to_owned),
        };
        let refused = request(&policy, args("GET", "ok", None), false)
            .await
            .unwrap_err();
        assert!(refused.to_string().contains("private address"));
        let refused = request(
            &policy,
            HttpArgs {
                method: "GET".into(),
                url: format!("http://localhost:{}/ok", address.port()),
                headers: Default::default(),
                body: None,
            },
            false,
        )
        .await
        .unwrap_err();
        assert!(refused.to_string().contains("private address"));
        let ok = request(&policy, args("GET", "ok", None), true).await?;
        assert_eq!(ok["status"], 200);
        assert_eq!(ok["body"], "hello");
        assert_eq!(ok["truncated"], false);
        assert_eq!(ok["request"]["url"], format!("http://{address}/ok"));
        let echoed = request(&policy, args("POST", "echo", Some("payload")), true).await?;
        assert_eq!(echoed["body"], "seen|payload");
        let big = request(&policy, args("GET", "big", None), true).await?;
        assert_eq!(big["truncated"], true);
        assert_eq!(big["body"].as_str().unwrap().len(), MAX_BODY);
        for url in ["ftp://example.test/x", "http://user:pw@example.test/x"] {
            let refused = request(
                &policy,
                HttpArgs {
                    method: "GET".into(),
                    url: url.into(),
                    headers: Default::default(),
                    body: None,
                },
                true,
            )
            .await
            .unwrap_err();
            assert!(matches!(refused, ToolError::InvalidArguments(_)));
        }
        let denied = HttpPolicy {
            allow: vec![],
            deny: vec![],
        };
        let refused = request(
            &denied,
            HttpArgs {
                method: "GET".into(),
                url: "https://example.test/".into(),
                headers: Default::default(),
                body: None,
            },
            true,
        )
        .await
        .unwrap_err();
        assert!(refused.to_string().contains("allow list"));
        server.abort();
        Ok(())
    }

    #[tokio::test]
    async fn runs_tool_reads_history_from_the_configured_runtime() -> anyhow::Result<()> {
        use turnkeel::{
            Agent, Runtime,
            testing::{ScriptedModel, Server, text},
        };
        let server = Server::start().await?;
        let agent = Agent::builder("runs-tool-fixture")
            .model(ScriptedModel::new().otherwise(text("done")))
            .build();
        let runtime = Runtime::configured(server.config(), std::slice::from_ref(&agent)).await?;
        runtime.run(&agent, "hello").await?;
        let tool = RunsTool {
            config: server.config(),
        };
        let mut listed = Value::Null;
        for _ in 0..100 {
            listed = tool
                .call(ToolCtx::new("fixture"), json!({"limit":5}))
                .await?;
            if !listed["runs"].as_array().unwrap().is_empty() {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert!(
            listed["runs"]
                .as_array()
                .unwrap()
                .iter()
                .any(|run| run["status"] == "Completed")
        );
        runtime.shutdown().await?;
        server.shutdown().await?;
        Ok(())
    }
}
