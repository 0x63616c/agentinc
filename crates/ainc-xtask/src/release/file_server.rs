//! A static file server on 127.0.0.1 for the upgrade gate's local feed
//! (`http.server` in the Python it replaces).
use anyhow::Result;
use std::{
    fs,
    io::{Read, Write},
    net::{Shutdown, SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

/// Serves files from a directory on 127.0.0.1 until dropped or `shutdown` is called.
pub struct FileServer {
    address: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    requests: Arc<Mutex<Vec<ServedRequest>>>,
}

/// Completed responses, used to prove which update payload the native app downloaded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServedRequest {
    pub path: String,
    pub bytes: usize,
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("json") => "application/json",
        Some("gz") => "application/gzip",
        Some("md") => "text/markdown",
        Some("xml") => "application/rss+xml",
        _ => "application/octet-stream",
    }
}

fn serve(mut stream: TcpStream, directory: &Path, requests: &Mutex<Vec<ServedRequest>>) {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    while !buffer.windows(4).any(|w| w == b"\r\n\r\n") {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(n) => buffer.extend_from_slice(&chunk[..n]),
        }
    }
    let request = String::from_utf8_lossy(&buffer).into_owned();
    let mut parts = request.lines().next().unwrap_or("").split_whitespace();
    let (method, target) = (parts.next().unwrap_or(""), parts.next().unwrap_or("/"));
    let relative = target
        .split(['?', '#'])
        .next()
        .unwrap_or("")
        .trim_start_matches('/');
    let safe = !relative.is_empty()
        && Path::new(relative)
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)));
    let file: Option<PathBuf> = safe
        .then(|| directory.join(relative))
        .filter(|p| p.is_file());
    let (status, body, kind) = match (method, file) {
        ("GET" | "HEAD", Some(path)) => match fs::read(&path) {
            Ok(body) => ("200 OK", body, content_type(&path)),
            Err(_) => ("404 Not Found", Vec::new(), "text/plain"),
        },
        _ => ("404 Not Found", Vec::new(), "text/plain"),
    };
    let head = format!(
        "HTTP/1.0 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    if stream.write_all(head.as_bytes()).is_ok()
        && method == "GET"
        && stream.write_all(&body).is_ok()
        && status == "200 OK"
    {
        requests.lock().unwrap().push(ServedRequest {
            path: relative.to_owned(),
            bytes: body.len(),
        });
    }
    let _ = stream.shutdown(Shutdown::Both);
}

impl FileServer {
    pub fn start(directory: &Path) -> Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let address = listener.local_addr()?;
        let stop = Arc::new(AtomicBool::new(false));
        let directory = directory.to_path_buf();
        let flag = stop.clone();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let served = requests.clone();
        let thread = thread::spawn(move || {
            for stream in listener.incoming() {
                if flag.load(Ordering::SeqCst) {
                    break;
                }
                if let Ok(stream) = stream {
                    let directory = directory.clone();
                    let served = served.clone();
                    thread::spawn(move || serve(stream, &directory, &served));
                }
            }
        });
        Ok(Self {
            address,
            stop,
            thread: Some(thread),
            requests,
        })
    }

    pub fn port(&self) -> u16 {
        self.address.port()
    }

    pub fn requests(&self) -> Vec<ServedRequest> {
        self.requests.lock().unwrap().clone()
    }

    pub fn shutdown(&mut self) {
        if let Some(thread) = self.thread.take() {
            self.stop.store(true, Ordering::SeqCst);
            // Wake the blocked accept so the loop sees the flag.
            let _ = TcpStream::connect(self.address);
            let _ = thread.join();
        }
    }
}

impl Drop for FileServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::release::http::request;
    use std::time::Duration;

    #[test]
    fn serves_files_not_outside_paths_and_shuts_down() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("feed.json"), b"{\"ok\":true}").unwrap();
        let mut server = FileServer::start(dir.path()).unwrap();
        let base = format!("http://127.0.0.1:{}", server.port());
        let get = |path: &str| {
            request(
                &format!("{base}{path}"),
                "GET",
                &[],
                None,
                Duration::from_secs(10),
            )
        };
        assert_eq!(get("/feed.json").unwrap(), b"{\"ok\":true}");
        assert!(
            request(
                &format!("{base}/feed.json"),
                "HEAD",
                &[],
                None,
                Duration::from_secs(10)
            )
            .unwrap()
            .is_empty()
        );
        assert!(get("/missing").is_err());
        assert!(get("/../x").is_err());
        assert_eq!(
            server.requests(),
            vec![ServedRequest {
                path: "feed.json".into(),
                bytes: 11
            }]
        );
        server.shutdown();
        assert!(get("/feed.json").is_err());
    }
}
