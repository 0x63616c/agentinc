//! OpenAI Responses transport; no shell execution, tools, or device access.
use crate::storage::Turn;
use anyhow::{Result, bail};
use reqwest::{blocking::Client, redirect::Policy};
use serde_json::{Value, json};
use std::{io::Read, time::Duration};

pub const MODEL: &str = "gpt-5-mini";
pub const KEYCHAIN_SERVICE: &str = "com.agentinc.os.openai";
const ENDPOINT: &str = "https://api.openai.com/v1/responses";
const INSTRUCTIONS: &str = "You are Evee, the personal assistant in Agentinc OS. Be concise, warm and practical. You can converse and help plan, but cannot access or change tasks, files, devices, calendars or other apps. Never claim to have performed an action. The user manages to-dos in the Tasks space. Use plain text suitable for a narrow chat panel.";

fn request_body(turns: &[Turn], current: &Turn, model: &str) -> Value {
    // ponytail: send the last 20 completed turns; add summarization only when needed.
    let mut history: Vec<_> = turns
        .iter()
        .filter(|t| t.id < current.id && t.response.is_some())
        .rev()
        .take(20)
        .collect();
    history.reverse();
    let mut input = Vec::new();
    for turn in history {
        input.push(json!({"role":"user", "content":turn.prompt}));
        input.push(json!({"role":"assistant", "content":turn.response}));
    }
    input.push(json!({"role":"user", "content":current.prompt}));
    json!({"model":model, "instructions":INSTRUCTIONS, "input":input,
        "store":false, "max_output_tokens":4096})
}
pub fn respond(key: &str, model: &str, turns: &[Turn], current: &Turn) -> Result<String> {
    request(
        ENDPOINT,
        key,
        request_body(turns, current, model),
        Duration::from_secs(90),
    )
}
fn request(endpoint: &str, key: &str, body: Value, timeout: Duration) -> Result<String> {
    // No redirects: a credential must never follow a redirect to another host.
    let client = Client::builder()
        .redirect(Policy::none())
        .timeout(timeout)
        .build()
        .map_err(|_| anyhow::anyhow!("Could not initialize the secure connection."))?;
    let response = client
        .post(endpoint)
        .bearer_auth(key)
        .json(&body)
        .send()
        .map_err(|error| {
            anyhow::anyhow!(if error.is_timeout() {
                "The reply timed out. You can retry."
            } else {
                "Could not reach OpenAI. Check your connection and retry."
            })
        })?;
    let status = response.status();
    if !status.is_success() {
        // Do not display/log raw service errors, which can echo keys or message text.
        bail!(match status.as_u16() {
            401 | 403 => "OpenAI rejected the API key or access. Check setup.",
            429 => "OpenAI rate or usage limit reached. Check your account, then retry.",
            400 | 404 => "OpenAI could not accept this request. Check the model in setup.",
            _ => "OpenAI is unavailable. Please retry later.",
        });
    }
    let mut bytes = Vec::new();
    response
        .take(1_048_577)
        .read_to_end(&mut bytes)
        .map_err(|_| anyhow::anyhow!("Could not read the reply. Please retry."))?;
    if bytes.len() > 1_048_576 {
        bail!("The reply was too large. Please try a shorter request.");
    }
    let data: Value = serde_json::from_slice(&bytes)
        .map_err(|_| anyhow::anyhow!("OpenAI returned an unreadable reply. Please retry."))?;
    parse_response(&data)
}
fn parse_response(data: &Value) -> Result<String> {
    if data["status"] != "completed" {
        bail!("OpenAI did not finish the reply. Try a shorter request.");
    }
    let mut parts = Vec::new();
    for output in data["output"].as_array().into_iter().flatten() {
        if output["type"] != "message" || output["role"] != "assistant" {
            continue;
        }
        for part in output["content"].as_array().into_iter().flatten() {
            let text = match part["type"].as_str() {
                Some("output_text") => part["text"].as_str(),
                Some("refusal") => part["refusal"].as_str(),
                _ => None,
            };
            if let Some(text) = text.filter(|t| !t.trim().is_empty()) {
                parts.push(text);
            }
        }
    }
    if parts.is_empty() {
        bail!("OpenAI returned no text. Please retry.");
    }
    Ok(parts.join("\n\n"))
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader, Write},
        net::TcpListener,
        thread,
    };
    fn server(status: &str, body: &str) -> (String, thread::JoinHandle<Value>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/v1/responses", listener.local_addr().unwrap());
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(&stream);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert!(line.starts_with("POST /v1/responses "));
            let mut length = 0;
            let mut auth = false;
            loop {
                line.clear();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                let lower = line.to_ascii_lowercase();
                if lower.starts_with("content-length:") {
                    length = line.split(':').nth(1).unwrap().trim().parse().unwrap();
                }
                if lower.starts_with("authorization:") {
                    auth = line.trim().ends_with("Bearer test-only");
                }
            }
            assert!(auth);
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let value = serde_json::from_slice(&body).unwrap();
            stream.write_all(response.as_bytes()).unwrap();
            value
        });
        (endpoint, handle)
    }
    #[test]
    fn http_round_trip_preserves_roles_and_collects_all_text() {
        let prior = Turn {
            id: 1,
            prompt: "Hello 👋".into(),
            response: Some("Hi".into()),
            error: None,
        };
        let current = Turn {
            id: 2,
            prompt: "Next".into(),
            response: None,
            error: None,
        };
        let body = request_body(&[prior], &current, MODEL);
        let (endpoint, server) = server(
            "200 OK",
            r#"{"status":"completed","output":[{"type":"reasoning"},{"type":"message","role":"assistant","content":[{"type":"output_text","text":"One"},{"type":"output_text","text":"Two"}]}]}"#,
        );
        assert_eq!(
            request(&endpoint, "test-only", body, Duration::from_secs(5)).unwrap(),
            "One\n\nTwo"
        );
        let received = server.join().unwrap();
        assert_eq!(received["input"][0]["content"], "Hello 👋");
        assert_eq!(received["input"][1]["role"], "assistant");
        assert_eq!(received["input"][2]["content"], "Next");
        assert_eq!(received["store"], false);
    }
    #[test]
    fn service_and_protocol_failures_are_safe_and_retryable() {
        for (status, body, expected) in [
            ("401 Unauthorized", "secret echoed", "API key"),
            ("429 Too Many Requests", "private text", "limit"),
            ("500 Server Error", "secret", "unavailable"),
            ("302 Found", "secret", "unavailable"),
            ("200 OK", "not json", "unreadable"),
            ("200 OK", r#"{"status":"incomplete","output":[]}"#, "finish"),
            ("200 OK", r#"{"status":"completed","output":[]}"#, "no text"),
        ] {
            let (endpoint, server) = server(status, body);
            let error = request(&endpoint, "test-only", json!({}), Duration::from_secs(5))
                .unwrap_err()
                .to_string();
            assert!(error.contains(expected), "{error}");
            assert!(!error.contains("secret"));
            server.join().unwrap();
        }
    }
    #[test]
    fn failed_request_retries_same_persisted_turn_then_restores_reply() -> Result<()> {
        use crate::storage::Store;
        let path = std::env::current_dir()?.join(format!(
            "target/chat-transport-{}.sqlite3",
            std::process::id()
        ));
        let store = Store::open(&path)?;
        let mut turn = store.begin_turn("[Fixture] Help plan my afternoon.")?;
        let (endpoint, failed_server) = server("429 Too Many Requests", "private service detail");
        turn.error = Some(
            request(
                &endpoint,
                "test-only",
                request_body(&store.turns()?, &turn, MODEL),
                Duration::from_secs(5),
            )
            .unwrap_err()
            .to_string(),
        );
        failed_server.join().unwrap();
        store.save_turn(&turn)?;
        drop(store);
        let store = Store::open(&path)?;
        let mut retry = store.turns()?.last().unwrap().clone();
        assert_eq!(retry.id, turn.id);
        assert!(retry.error.as_ref().unwrap().contains("limit"));
        retry.error = None;
        store.save_turn(&retry)?;
        let (endpoint, success_server) = server(
            "200 OK",
            r#"{"status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"[Fixture] Start with the most important task."}]}]}"#,
        );
        retry.response = Some(request(
            &endpoint,
            "test-only",
            request_body(&store.turns()?, &retry, MODEL),
            Duration::from_secs(5),
        )?);
        success_server.join().unwrap();
        store.save_turn(&retry)?;
        drop(store);
        let store = Store::open(&path)?;
        assert_eq!(store.turns()?.last(), Some(&retry));
        assert!(
            store
                .turns()?
                .iter()
                .all(|t| t.id != turn.id || t.error.is_none())
        );
        drop(store);
        std::fs::remove_file(path)?;
        Ok(())
    }
    #[test]
    fn timeout_and_refusal_are_handled() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/v1/responses", listener.local_addr().unwrap());
        let peer = thread::spawn(move || {
            let (_stream, _) = listener.accept().unwrap();
            thread::sleep(Duration::from_millis(250));
        });
        assert!(
            request(
                &endpoint,
                "test-only",
                json!({}),
                Duration::from_millis(100)
            )
            .unwrap_err()
            .to_string()
            .contains("timed out")
        );
        peer.join().unwrap();
        assert_eq!(parse_response(&json!({"status":"completed", "output":[{"type":"message","role":"assistant","content":[{"type":"refusal","refusal":"I cannot help with that."}]}]})).unwrap(), "I cannot help with that.");
    }
    #[test]
    fn history_excludes_failed_future_and_unbounded_old_turns() {
        let mut history: Vec<_> = (1..=30)
            .map(|id| Turn {
                id,
                prompt: id.to_string(),
                response: Some("Reply".into()),
                error: None,
            })
            .collect();
        history[28].response = None;
        let current = Turn {
            id: 30,
            prompt: "Retry".into(),
            response: None,
            error: None,
        };
        let body = request_body(&history, &current, MODEL);
        assert_eq!(body["input"].as_array().unwrap().len(), 41);
        assert_eq!(body["input"][0]["content"], "9");
        assert_eq!(body["input"][40]["content"], "Retry");
    }
}
