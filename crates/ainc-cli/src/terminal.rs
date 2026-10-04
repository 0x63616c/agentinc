//! `ainc terminals attach`: the reconnecting viewer the Mac app runs inside each Ghostty pane
//! for a daemon-owned Terminal. It keeps trying while the daemon restarts or updates.
use ainc_client::{ClientError, types::CreateTerminal, types::ErrorCode};
use anyhow::{Context, Result};
use futures::{SinkExt, StreamExt};
use std::{io::IsTerminal, os::fd::AsRawFd, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    signal::unix::{SignalKind, signal},
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, client::IntoClientRequest, http::HeaderValue},
};
use uuid::Uuid;

const RETRY: Duration = Duration::from_millis(500);
const REFUSED: &[u8] = b"\r\n[Terminal daemon authentication or version failed]\r\n";

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

/// The resize message: tag 1, then rows and columns as big-endian `u16`.
fn size() -> [u8; 5] {
    let mut size: libc::winsize = unsafe { std::mem::zeroed() };
    unsafe { libc::ioctl(std::io::stdin().as_raw_fd(), libc::TIOCGWINSZ, &mut size) };
    let rows = size.ws_row.max(1).to_be_bytes();
    let cols = size.ws_col.max(1).to_be_bytes();
    [1, rows[0], rows[1], cols[0], cols[1]]
}

/// The daemon's address and owner credential, read afresh so a restarted daemon is found.
fn connection() -> Result<(String, String)> {
    let (url, token_path) = crate::configuration()?;
    let token =
        std::fs::read_to_string(token_path).context("daemon owner credential unavailable")?;
    Ok((url, token.trim().into()))
}

/// Whether the daemon refused this client for good (credential or version), not a passing outage.
fn refused(error: &ClientError) -> bool {
    match error {
        ClientError::UpdateRequired => true,
        ClientError::Rejected(body) => body.code == ErrorCode::Unauthorized,
        _ => false,
    }
}

/// Attach to Terminal `id`, creating it first unless `existing`, until it ends or stdin closes.
pub async fn attach(id: Uuid, mut existing: bool) -> Result<()> {
    let _raw = RawMode::new()?;
    let mut stdout = tokio::io::stdout();
    let mut stdin = tokio::io::stdin();
    let mut input = [0u8; 8192];
    let mut resize = signal(SignalKind::window_change())?;
    stdout
        .write_all(b"\r\n[Connecting to AgentInc terminal...]\r\n")
        .await?;
    loop {
        let Ok((url, token)) = connection() else {
            tokio::time::sleep(RETRY).await;
            continue;
        };
        if !existing {
            let created = ainc_client::connect(&url, &token)?
                .terminals_create()
                .body(CreateTerminal { id: id.to_string() })
                .send()
                .await;
            match created.map_err(ainc_client::classify) {
                Ok(_) => existing = true,
                Err(error) if refused(&error) => {
                    stdout.write_all(REFUSED).await?;
                    return Ok(());
                }
                Err(_) => {
                    tokio::time::sleep(RETRY).await;
                    continue;
                }
            }
        }
        let ws_url = url.replacen("http://", "ws://", 1);
        let mut request = format!("{ws_url}/v1/terminals/{id}/attach").into_client_request()?;
        request.headers_mut().insert(
            "authorization",
            HeaderValue::from_str(&format!("Bearer {token}"))?,
        );
        request.headers_mut().insert(
            ainc_identity::CLIENT_HEADER,
            HeaderValue::from_str(&ainc_identity::client_header_as("cli"))?,
        );
        let mut socket = match connect_async(request).await {
            Ok((socket, _)) => socket,
            Err(tokio_tungstenite::tungstenite::Error::Http(response))
                if response.status().as_u16() == 404 =>
            {
                // Saved pane IDs can outlive the daemon after an update.
                existing = false;
                continue;
            }
            Err(tokio_tungstenite::tungstenite::Error::Http(response))
                if response.status().as_u16() == 401 || response.status().as_u16() == 426 =>
            {
                stdout.write_all(REFUSED).await?;
                return Ok(());
            }
            Err(_) => {
                tokio::time::sleep(RETRY).await;
                continue;
            }
        };
        stdout.write_all(b"\x1bc").await?;
        stdout.flush().await?;
        let _ = socket.send(Message::Binary(size().to_vec().into())).await;
        loop {
            tokio::select! {
                read = stdin.read(&mut input) => match read {
                    Ok(0) => return Ok(()),
                    Ok(count) => {
                        let mut bytes = Vec::with_capacity(count + 1);
                        bytes.push(0);
                        bytes.extend_from_slice(&input[..count]);
                        if socket.send(Message::Binary(bytes.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(error) => return Err(error.into()),
                },
                received = socket.next() => match received {
                    Some(Ok(Message::Binary(bytes))) => {
                        stdout.write_all(&bytes).await?;
                        stdout.flush().await?;
                    }
                    Some(Ok(Message::Text(text))) if text == "ended" => {
                        stdout.write_all(b"\r\n[Terminal session ended]\r\n").await?;
                        return Ok(());
                    }
                    _ => break,
                },
                _ = resize.recv() => {
                    let _ = socket.send(Message::Binary(size().to_vec().into())).await;
                }
            }
        }
        stdout
            .write_all(b"\x1bc\r\n[Reconnecting terminal...]\r\n")
            .await?;
        tokio::time::sleep(RETRY).await;
    }
}
