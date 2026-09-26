//! A thin client for World Wide Webb's control center, which owns Home Assistant.
//! It speaks the control center's tRPC wire format: queries are `GET
//! /trpc/<procedure>`, mutations `POST` the raw JSON input, and both answer
//! `{"result":{"data":…}}`.
use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(4);

/// A Cloudflare Access service token for a control center behind Access.
#[derive(Clone)]
pub struct AccessToken {
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Clone)]
pub struct ControlCenter {
    http: reqwest::Client,
    base: String,
    access: Option<AccessToken>,
}

/// One switchable group's state.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct Group {
    pub on: bool,
    #[serde(default)]
    pub pending: bool,
}
/// The groups AgentInc exposes, as `controls.list` reports them.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Controls {
    pub all: Group,
    pub lamps: Group,
    pub bedroom_lamps: Group,
    pub other_lamps: Group,
    pub ceiling: Group,
    pub cabinet: Group,
}
/// `climate.get`: the thermostat's mode, setpoints and indoor reading (°F).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Climate {
    pub mode: String,
    pub ambient: Option<f64>,
    pub action: Option<String>,
    pub target: Option<i64>,
    pub target_low: Option<i64>,
    pub target_high: Option<i64>,
}

impl ControlCenter {
    pub fn new(http: reqwest::Client, base: &str, access: Option<AccessToken>) -> Self {
        Self {
            http,
            base: base.trim_end_matches('/').to_owned(),
            access,
        }
    }
    /// A client that never follows redirects: Access answers a refused token
    /// with a redirect to its login page, which must surface as an error.
    pub fn http() -> reqwest::Client {
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(TIMEOUT)
            .build()
            .expect("control center HTTP client")
    }

    pub async fn controls(&self) -> Result<Controls> {
        self.query("controls.list").await
    }
    pub async fn climate(&self) -> Result<Climate> {
        self.query("climate.get").await
    }
    pub async fn toggle(&self, key: &str, on: bool) -> Result<()> {
        self.mutate("controls.toggle", json!({"key": key, "on": on}))
            .await
    }
    pub async fn set_mode(&self, mode: &str) -> Result<()> {
        self.mutate("climate.setMode", json!(mode)).await
    }
    pub async fn set_target(&self, target: i64) -> Result<()> {
        self.mutate("climate.setTarget", json!(target)).await
    }
    pub async fn set_range(&self, low: i64, high: i64) -> Result<()> {
        self.mutate("climate.setRange", json!({"low": low, "high": high}))
            .await
    }

    async fn query<T: DeserializeOwned>(&self, procedure: &str) -> Result<T> {
        let request = self.http.get(format!("{}/trpc/{procedure}", self.base));
        let data = self.send(request).await?;
        serde_json::from_value(data).with_context(|| {
            format!("The control center answered {procedure} in an unknown shape.")
        })
    }
    async fn mutate(&self, procedure: &str, input: Value) -> Result<()> {
        let request = self
            .http
            .post(format!("{}/trpc/{procedure}", self.base))
            .json(&input);
        self.send(request).await.map(drop)
    }
    async fn send(&self, mut request: reqwest::RequestBuilder) -> Result<Value> {
        if let Some(access) = &self.access {
            request = request
                .header("CF-Access-Client-Id", &access.client_id)
                .header("CF-Access-Client-Secret", &access.client_secret);
        }
        let response = request
            .send()
            .await
            .map_err(|_| anyhow!("The control center is unreachable."))?;
        let status = response.status();
        if status.is_redirection() || status == reqwest::StatusCode::FORBIDDEN {
            bail!("Cloudflare Access refused the service token.");
        }
        let body: Value = response
            .json()
            .await
            .map_err(|_| anyhow!("The control center answered with HTTP {status}."))?;
        if !status.is_success() {
            let message = body
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or("request failed");
            bail!("The control center refused the request: {message}");
        }
        body.pointer("/result/data")
            .cloned()
            .ok_or_else(|| anyhow!("The control center answered without data."))
    }
}
