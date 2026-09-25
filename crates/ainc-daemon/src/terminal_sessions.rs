//! Owner-scoped terminal PTYs. A disconnected viewer does not own the shell.
use crate::product::{ApiError, ErrorBody, Product};
use axum::{
    Json, Router,
    extract::{
        Path, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    fs::File,
    os::fd::{AsRawFd, FromRawFd},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::{
    io::unix::AsyncFd,
    process::Command,
    sync::{broadcast, mpsc, watch},
};
use utoipa::ToSchema;
use uuid::Uuid;

const SCROLLBACK_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone)]
struct Service {
    product: Product,
    sessions: Arc<Mutex<HashMap<Uuid, Arc<Session>>>>,
}
struct Session {
    pid: i32,
    master: File,
    input: mpsc::UnboundedSender<Vec<u8>>,
    output: broadcast::Sender<Vec<u8>>,
    scrollback: Mutex<VecDeque<u8>>,
    ended: AtomicBool,
    ending: watch::Sender<bool>,
}
impl Drop for Session {
    fn drop(&mut self) {
        if !self.ended.load(Ordering::SeqCst) {
            // The shell is a session leader; its children share this process group.
            unsafe { libc::kill(-self.pid, libc::SIGHUP) };
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateTerminalSession {
    pub id: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct TerminalSession {
    pub id: String,
    pub workspace_id: String,
    pub state: String,
}
fn api_error(error: impl std::fmt::Display) -> ApiError {
    tracing::error!(%error, "terminal session failed");
    ApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "terminal_unavailable",
        "Terminal unavailable. Try again.",
    )
}
fn missing() -> ApiError {
    ApiError::new(
        StatusCode::NOT_FOUND,
        "session_ended",
        "This terminal session has ended.",
    )
}
fn bad_id() -> ApiError {
    ApiError::new(StatusCode::BAD_REQUEST, "invalid", "Use a UUID session ID.")
}
pub fn router(product: Product) -> Router {
    let service = Service {
        product,
        sessions: Arc::new(Mutex::new(HashMap::new())),
    };
    Router::new()
        .route("/v1/terminal/sessions", get(list).post(create))
        .route("/v1/terminal/sessions/{id}", axum::routing::delete(close))
        .route("/v1/terminal/sessions/{id}/attach", get(attach))
        .with_state(service)
}
#[utoipa::path(get, path = "/v1/terminal/sessions", operation_id = "terminal_sessions_list", responses((status = 200, body = Vec<TerminalSession>), (status = 401, body = ErrorBody)))]
async fn list(
    State(service): State<Service>,
    headers: HeaderMap,
) -> Result<Json<Vec<TerminalSession>>, ApiError> {
    service.product.authorize(&headers)?;
    let sessions = service
        .sessions
        .lock()
        .unwrap()
        .iter()
        .map(|(id, session)| TerminalSession {
            id: id.to_string(),
            workspace_id: "local".into(),
            state: if session.ended.load(Ordering::SeqCst) {
                "ended"
            } else {
                "running"
            }
            .into(),
        })
        .collect();
    Ok(Json(sessions))
}
#[utoipa::path(post, path = "/v1/terminal/sessions", operation_id = "terminal_sessions_create", request_body = CreateTerminalSession, responses((status = 200, body = TerminalSession), (status = 401, body = ErrorBody), (status = 503, body = ErrorBody)))]
async fn create(
    State(service): State<Service>,
    headers: HeaderMap,
    Json(request): Json<CreateTerminalSession>,
) -> Result<Json<TerminalSession>, ApiError> {
    service.product.authorize(&headers)?;
    let id = Uuid::parse_str(&request.id).map_err(|_| bad_id())?;
    if id.is_nil() {
        return Err(bad_id());
    }
    let mut sessions = service.sessions.lock().unwrap();
    if let std::collections::hash_map::Entry::Vacant(entry) = sessions.entry(id) {
        let session = spawn_shell().map_err(api_error)?;
        entry.insert(session);
    }
    let session = &sessions[&id];
    Ok(Json(TerminalSession {
        id: request.id,
        workspace_id: "local".into(),
        state: if session.ended.load(Ordering::SeqCst) {
            "ended"
        } else {
            "running"
        }
        .into(),
    }))
}
#[utoipa::path(delete, path = "/v1/terminal/sessions/{id}", operation_id = "terminal_sessions_close", params(("id" = String, Path)), responses((status = 204), (status = 400, body = ErrorBody), (status = 401, body = ErrorBody), (status = 404, body = ErrorBody)))]
async fn close(
    State(service): State<Service>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    service.product.authorize(&headers)?;
    let id = Uuid::parse_str(&id).map_err(|_| bad_id())?;
    let session = service
        .sessions
        .lock()
        .unwrap()
        .remove(&id)
        .ok_or_else(missing)?;
    session.ended.store(true, Ordering::SeqCst);
    let _ = session.ending.send(true);
    unsafe { libc::kill(-session.pid, libc::SIGHUP) };
    Ok(StatusCode::NO_CONTENT)
}
async fn attach(
    State(service): State<Service>,
    headers: HeaderMap,
    Path(id): Path<String>,
    upgrade: WebSocketUpgrade,
) -> Result<impl IntoResponse, ApiError> {
    service.product.authorize(&headers)?;
    let id = Uuid::parse_str(&id).map_err(|_| bad_id())?;
    let session = service
        .sessions
        .lock()
        .unwrap()
        .get(&id)
        .cloned()
        .ok_or_else(missing)?;
    Ok(upgrade.on_upgrade(move |socket| attached(socket, session)))
}
async fn attached(socket: WebSocket, session: Arc<Session>) {
    let (mut sink, mut source) = socket.split();
    let (mut output, replay) = {
        let history = session.scrollback.lock().unwrap();
        let output = session.output.subscribe();
        let replay: Vec<u8> = history.iter().copied().collect();
        (output, replay)
    };
    let mut ending = session.ending.subscribe();
    if !replay.is_empty() && sink.send(Message::Binary(replay.into())).await.is_err() {
        return;
    }
    if *ending.borrow() {
        let _ = sink.send(Message::Text("ended".into())).await;
        return;
    }
    loop {
        tokio::select! {
            incoming = source.next() => match incoming {
                Some(Ok(Message::Binary(data))) if !data.is_empty() => match data[0] {
                    0 => { let _ = session.input.send(data[1..].to_vec()); }
                    1 if data.len() == 5 => {
                        let rows = u16::from_be_bytes([data[1], data[2]]);
                        let cols = u16::from_be_bytes([data[3], data[4]]);
                        let size = libc::winsize { ws_row: rows, ws_col: cols, ws_xpixel: 0, ws_ypixel: 0 };
                        unsafe { libc::ioctl(session.master.as_raw_fd(), libc::TIOCSWINSZ as libc::c_ulong, &size); libc::kill(-session.pid, libc::SIGWINCH); }
                    }
                    _ => {}
                },
                _ => break,
            },
            received = output.recv() => match received {
                Ok(bytes) => { if sink.send(Message::Binary(bytes.into())).await.is_err() { break; } },
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    // Repaint from the bounded buffer after a slow viewer falls behind.
                    let bytes: Vec<u8> = session.scrollback.lock().unwrap().iter().copied().collect();
                    if sink.send(Message::Binary(b"\x1bc".to_vec().into())).await.is_err() { break; }
                    if sink.send(Message::Binary(bytes.into())).await.is_err() { break; }
                }
                Err(_) => break,
            },
            _ = ending.changed() => {
                let _ = sink.send(Message::Text("ended".into())).await;
                break;
            }
        }
    }
}
fn spawn_shell() -> anyhow::Result<Arc<Session>> {
    let mut master = -1;
    let mut slave = -1;
    let mut size = libc::winsize {
        ws_row: 24,
        ws_col: 80,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    anyhow::ensure!(
        unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut size,
            )
        } == 0,
        "openpty: {}",
        std::io::Error::last_os_error()
    );
    let master = unsafe { File::from_raw_fd(master) };
    let slave = unsafe { File::from_raw_fd(slave) };
    let shell = std::env::var_os("SHELL")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/bin/zsh"));
    let home = std::env::var_os("HOME").ok_or_else(|| anyhow::anyhow!("HOME is missing"))?;
    let mut command = Command::new(&shell);
    command
        .arg("-l")
        .current_dir(home)
        .env("TERM", "xterm-256color")
        .env("COLORTERM", "truecolor");
    let slave_fd = slave.as_raw_fd();
    // SAFETY: only async-signal-safe libc calls run between fork and exec.
    unsafe {
        command.pre_exec(move || {
            if libc::setsid() < 0 || libc::ioctl(slave_fd, libc::TIOCSCTTY as libc::c_ulong, 0) < 0
            {
                return Err(std::io::Error::last_os_error());
            }
            for fd in 0..=2 {
                if libc::dup2(slave_fd, fd) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            if slave_fd > 2 {
                libc::close(slave_fd);
            }
            Ok(())
        });
    }
    let mut child = command.spawn()?;
    drop(slave);
    let pid = child
        .id()
        .ok_or_else(|| anyhow::anyhow!("shell has no pid"))? as i32;
    let (input, mut received) = mpsc::unbounded_channel::<Vec<u8>>();
    let (output, _) = broadcast::channel(128);
    let (ending, _) = watch::channel(false);
    let session = Arc::new(Session {
        pid,
        master: master.try_clone()?,
        input,
        output,
        scrollback: Mutex::new(VecDeque::new()),
        ended: AtomicBool::new(false),
        ending,
    });
    let writer = master.try_clone()?;
    unsafe {
        libc::fcntl(master.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK);
    }
    let reader = AsyncFd::new(master)?;
    let writer = AsyncFd::new(writer)?;
    tokio::spawn(async move {
        while let Some(bytes) = received.recv().await {
            let mut offset = 0;
            while offset < bytes.len() {
                let Ok(mut ready) = writer.writable().await else {
                    return;
                };
                match ready.try_io(|fd| {
                    let count = unsafe {
                        libc::write(
                            fd.as_raw_fd(),
                            bytes[offset..].as_ptr().cast(),
                            bytes.len() - offset,
                        )
                    };
                    if count < 0 {
                        Err(std::io::Error::last_os_error())
                    } else {
                        Ok(count as usize)
                    }
                }) {
                    Ok(Ok(count)) if count > 0 => offset += count,
                    Ok(_) => return,
                    Err(_) => {}
                }
            }
        }
    });
    let reader_session = session.clone();
    tokio::spawn(async move {
        let mut bytes = [0u8; 8192];
        loop {
            let Ok(mut ready) = reader.readable().await else {
                break;
            };
            match ready.try_io(|fd| {
                let count =
                    unsafe { libc::read(fd.as_raw_fd(), bytes.as_mut_ptr().cast(), bytes.len()) };
                if count < 0 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(count as usize)
                }
            }) {
                Ok(Ok(0)) | Ok(Err(_)) => break,
                Ok(Ok(count)) => {
                    let chunk = bytes[..count].to_vec();
                    let mut history = reader_session.scrollback.lock().unwrap();
                    history.extend(&chunk);
                    let excess = history.len().saturating_sub(SCROLLBACK_BYTES);
                    history.drain(..excess);
                    let _ = reader_session.output.send(chunk);
                    drop(history);
                }
                Err(_) => {}
            }
        }
        reader_session.ended.store(true, Ordering::SeqCst);
        let _ = reader_session.ending.send(true);
    });
    tokio::spawn(async move {
        let _ = child.wait().await;
    });
    Ok(session)
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{SinkExt, StreamExt};
    use std::{ffi::CString, io::Write};
    use tokio_tungstenite::{
        connect_async,
        tungstenite::{Message as ClientMessage, client::IntoClientRequest},
    };

    #[tokio::test]
    async fn disconnected_viewer_keeps_shell_and_replays_output() {
        let pool = sqlx::PgPool::connect_lazy("postgres://unused:unused@127.0.0.1/unused").unwrap();
        let app = router(Product::new(pool, "owner".into()).unwrap());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let id = Uuid::new_v4();
        let client = reqwest::Client::new();
        client
            .post(format!("http://{address}/v1/terminal/sessions"))
            .bearer_auth("owner")
            .json(&serde_json::json!({"id": id.to_string()}))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
        let temp = tempfile::tempdir().unwrap();
        let fifo = temp.path().join("gate");
        let c_path = CString::new(fifo.to_string_lossy().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0);
        let uri = format!("ws://{address}/v1/terminal/sessions/{id}/attach");
        let mut request = uri.as_str().into_client_request().unwrap();
        request
            .headers_mut()
            .insert("authorization", "Bearer owner".parse().unwrap());
        let (mut first, _) = connect_async(request.clone()).await.unwrap();
        let command = format!(
            "IFS= read -r line < {}; printf '__AFTER__:%s\\n' \"$line\"\n",
            fifo.display()
        );
        let mut input = vec![0];
        input.extend_from_slice(command.as_bytes());
        first
            .send(ClientMessage::Binary(input.into()))
            .await
            .unwrap();
        // The echoed command is an explicit gate: the PTY has accepted it.
        let mut accepted = Vec::new();
        loop {
            if let Some(Ok(ClientMessage::Binary(bytes))) = first.next().await {
                accepted.extend_from_slice(&bytes);
                if String::from_utf8_lossy(&accepted).contains("__AFTER__") {
                    break;
                }
            }
        }
        drop(first);
        let gate = tokio::task::spawn_blocking(move || {
            let mut file = std::fs::OpenOptions::new().write(true).open(fifo).unwrap();
            file.write_all(b"survived\n").unwrap();
        });
        gate.await.unwrap();
        let (mut second, _) = connect_async(request).await.unwrap();
        let mut replay = Vec::new();
        loop {
            if let Some(Ok(ClientMessage::Binary(bytes))) = second.next().await {
                replay.extend_from_slice(&bytes);
                if String::from_utf8_lossy(&replay).contains("__AFTER__:survived") {
                    break;
                }
            }
        }
        second.close(None).await.unwrap();
        client
            .delete(format!("http://{address}/v1/terminal/sessions/{id}"))
            .bearer_auth("owner")
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
        server.abort();
    }
}
