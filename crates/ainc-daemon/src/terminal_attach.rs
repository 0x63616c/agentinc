//! Ghostty child process: reconnecting viewer for a daemon-owned PTY.
use anyhow::{Context, Result};
use futures::{SinkExt, StreamExt};
use std::{io::IsTerminal, os::fd::AsRawFd, path::PathBuf, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    signal::unix::{SignalKind, signal},
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, client::IntoClientRequest, http::HeaderValue},
};
use uuid::Uuid;

struct RawMode(Option<libc::termios>);
impl RawMode {
    fn new() -> Result<Self> {
        if !std::io::stdin().is_terminal() {
            return Ok(Self(None));
        }
        let fd = std::io::stdin().as_raw_fd();
        let mut original = unsafe { std::mem::zeroed() };
        if unsafe { libc::tcgetattr(fd, &mut original) } != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let mut raw = original;
        unsafe { libc::cfmakeraw(&mut raw) };
        if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &raw) } != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(Self(Some(original)))
    }
}
impl Drop for RawMode {
    fn drop(&mut self) {
        if let Some(original) = &self.0 {
            unsafe { libc::tcsetattr(std::io::stdin().as_raw_fd(), libc::TCSANOW, original) };
        }
    }
}
fn size() -> [u8; 5] {
    let mut size: libc::winsize = unsafe { std::mem::zeroed() };
    unsafe { libc::ioctl(std::io::stdin().as_raw_fd(), libc::TIOCGWINSZ, &mut size) };
    let rows = size.ws_row.max(1).to_be_bytes();
    let cols = size.ws_col.max(1).to_be_bytes();
    [1, rows[0], rows[1], cols[0], cols[1]]
}
fn connection() -> Result<(String, String)> {
    let discovery = std::env::var_os("AINC_DISCOVERY_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| ainc_release::identity::support_dir().join("daemon/api-url"));
    let url = std::fs::read_to_string(&discovery).context("daemon unavailable")?;
    let token_path = std::env::var_os("AINC_TOKEN_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| discovery.with_file_name("owner-token"));
    let token =
        std::fs::read_to_string(token_path).context("daemon owner credential unavailable")?;
    Ok((url.trim().trim_end_matches('/').into(), token.trim().into()))
}
pub async fn run(id: Uuid, mut existing: bool) -> Result<()> {
    let _raw = RawMode::new()?;
    let mut stdout = tokio::io::stdout();
    let mut stdin = tokio::io::stdin();
    let mut input = [0u8; 8192];
    let mut resize = signal(SignalKind::window_change())?;
    stdout
        .write_all(b"\r\n[Connecting to AgentInc terminal...]\r\n")
        .await?;
    loop {
        let (url, token) = match connection() {
            Ok(value) => value,
            Err(_) => {
                tokio::time::sleep(Duration::from_millis(500)).await;
                continue;
            }
        };
        if !existing {
            let response = reqwest::Client::new()
                .post(format!("{url}/v1/terminal/sessions"))
                .header("authorization", format!("Bearer {token}"))
                .header("agent-inc-client", ainc_release::client_header())
                .json(&serde_json::json!({"id": id.to_string()}))
                .send()
                .await;
            match response {
                Ok(response) if response.status().is_success() => {
                    existing = true;
                }
                Ok(response)
                    if response.status().as_u16() == 401 || response.status().as_u16() == 426 =>
                {
                    stdout
                        .write_all(b"\r\n[Terminal daemon authentication or version failed]\r\n")
                        .await?;
                    return Ok(());
                }
                _ => {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    continue;
                }
            }
        }
        let ws_url = url.replacen("http://", "ws://", 1);
        let mut request =
            format!("{ws_url}/v1/terminal/sessions/{id}/attach").into_client_request()?;
        request.headers_mut().insert(
            "authorization",
            HeaderValue::from_str(&format!("Bearer {token}"))?,
        );
        request.headers_mut().insert(
            "agent-inc-client",
            HeaderValue::from_str(&ainc_release::client_header())?,
        );
        let socket = connect_async(request).await;
        let mut socket = match socket {
            Ok((socket, _)) => socket,
            Err(tokio_tungstenite::tungstenite::Error::Http(response))
                if response.status().as_u16() == 404 =>
            {
                stdout
                    .write_all(b"\r\n[Terminal session ended]\r\n")
                    .await?;
                return Ok(());
            }
            Err(tokio_tungstenite::tungstenite::Error::Http(response))
                if response.status().as_u16() == 401 || response.status().as_u16() == 426 =>
            {
                stdout
                    .write_all(b"\r\n[Terminal daemon authentication or version failed]\r\n")
                    .await?;
                return Ok(());
            }
            Err(_) => {
                tokio::time::sleep(Duration::from_millis(500)).await;
                continue;
            }
        };
        let _ = socket.send(Message::Binary(size().to_vec().into())).await;
        loop {
            tokio::select! {
                read = stdin.read(&mut input) => match read {
                    Ok(0) => return Ok(()),
                    Ok(count) => { let mut bytes = Vec::with_capacity(count + 1); bytes.push(0); bytes.extend_from_slice(&input[..count]); if socket.send(Message::Binary(bytes.into())).await.is_err() { break; } },
                    Err(error) => return Err(error.into()),
                },
                received = socket.next() => match received {
                    Some(Ok(Message::Binary(bytes))) => stdout.write_all(&bytes).await?,
                    Some(Ok(Message::Text(text))) if text == "ended" => { stdout.write_all(b"\r\n[Terminal session ended]\r\n").await?; return Ok(()); },
                    _ => break,
                },
                _ = resize.recv() => { let _ = socket.send(Message::Binary(size().to_vec().into())).await; }
            }
        }
        stdout
            .write_all(b"\x1bc\r\n[Reconnecting terminal...]\r\n")
            .await?;
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

pub async fn close(id: Uuid) -> Result<()> {
    let (url, token) = connection()?;
    reqwest::Client::new()
        .delete(format!("{url}/v1/terminal/sessions/{id}"))
        .header("authorization", format!("Bearer {token}"))
        .header("agent-inc-client", ainc_release::client_header())
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}
