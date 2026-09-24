//! Private Unix sessions and a one-request-per-connection client.
use crate::protocol::*;
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, DirBuilder, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    os::unix::{
        fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub version: u32,
    pub session: String,
    pub window: String,
    pub title: String,
    pub pid: u32,
    pub socket: PathBuf,
    pub token_file: PathBuf,
}

pub struct Session {
    pub manifest: Manifest,
    pub directory: PathBuf,
    token: String,
}
impl Session {
    /// Exclusively create a new directory. Never reclaim an existing endpoint.
    pub fn create(directory: &Path, title: String) -> Result<(Self, UnixListener)> {
        ensure!(
            directory.is_absolute(),
            "session directory must be absolute"
        );
        ensure!(
            directory.join("pilot.sock").as_os_str().len() < 104,
            "Unix socket path must be shorter than 104 bytes"
        );
        DirBuilder::new()
            .mode(0o700)
            .create(directory)
            .context("create fresh private pilot directory")?;
        let result = (|| {
            let directory = directory.canonicalize()?;
            let token = random_id()?;
            let manifest = Manifest {
                version: VERSION,
                session: random_id()?,
                window: "main".into(),
                title,
                pid: std::process::id(),
                socket: directory.join("pilot.sock"),
                token_file: directory.join("token"),
            };
            private_file(&manifest.token_file, token.as_bytes())?;
            let listener = UnixListener::bind(&manifest.socket)?;
            fs::set_permissions(&manifest.socket, fs::Permissions::from_mode(0o600))?;
            private_file(
                &directory.join("instance.json"),
                &serde_json::to_vec_pretty(&manifest)?,
            )?;
            Ok((
                Self {
                    directory,
                    manifest,
                    token,
                },
                listener,
            ))
        })();
        if result.is_err() {
            // Only this invocation created this directory and its known files.
            for name in ["token", "pilot.sock", "instance.json"] {
                let _ = fs::remove_file(directory.join(name));
            }
            let _ = fs::remove_dir(directory);
        }
        result
    }
    pub fn authenticate(&self, request: &Request) -> Result<(), Failure> {
        if request.token.as_bytes() != self.token.as_bytes() {
            return Err(Failure::new("unauthorized", "Invalid session token"));
        }
        if request.version != VERSION {
            return Err(Failure::new(
                "protocol_version",
                "Unsupported protocol version",
            ));
        }
        if request.session != self.manifest.session || request.window != self.manifest.window {
            return Err(Failure::new(
                "wrong_scope",
                "Session/window does not match this host",
            ));
        }
        request.command.validate()
    }
}
impl Session {
    /// Remove owned endpoints even when transport workers still hold session metadata.
    pub fn close(&self) {
        for name in ["pilot.sock", "token", "instance.json"] {
            let _ = fs::remove_file(self.directory.join(name));
        }
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.close();
    }
}

fn random_id() -> Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| anyhow::anyhow!("OS randomness: {e}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
fn private_file(path: &Path, bytes: &[u8]) -> Result<()> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?
        .write_all(bytes)?;
    Ok(())
}
pub fn peer_is_owner(stream: &UnixStream) -> Result<()> {
    use std::os::fd::AsRawFd;
    #[cfg(target_os = "macos")]
    let uid = {
        let (mut uid, mut gid) = (0, 0);
        // SAFETY: valid connected fd; writable uid/gid output pointers.
        ensure!(
            unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) } == 0,
            "peer credentials unavailable"
        );
        uid
    };
    #[cfg(target_os = "linux")]
    let uid = {
        let mut credentials: libc::ucred = unsafe { std::mem::zeroed() };
        let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        // SAFETY: correctly sized output buffer for SO_PEERCRED and a live fd.
        ensure!(
            unsafe {
                libc::getsockopt(
                    stream.as_raw_fd(),
                    libc::SOL_SOCKET,
                    libc::SO_PEERCRED,
                    (&mut credentials as *mut libc::ucred).cast(),
                    &mut len,
                )
            } == 0,
            "peer credentials unavailable"
        );
        credentials.uid
    };
    // SAFETY: geteuid has no preconditions.
    ensure!(
        uid == unsafe { libc::geteuid() },
        "peer belongs to another user"
    );
    Ok(())
}
pub fn read_message<T: serde::de::DeserializeOwned>(
    stream: &mut UnixStream,
    limit: usize,
) -> Result<T> {
    let mut bytes = Vec::new();
    BufReader::new(stream.take((limit + 1) as u64)).read_until(b'\n', &mut bytes)?;
    ensure!(
        bytes.len() <= limit && bytes.last() == Some(&b'\n'),
        "message missing newline or exceeds limit"
    );
    Ok(serde_json::from_slice(&bytes)?)
}
pub fn write_message<T: Serialize>(stream: &mut UnixStream, message: &T) -> Result<()> {
    serde_json::to_writer(&mut *stream, message)?;
    stream.write_all(b"\n")?;
    Ok(())
}

pub struct Client {
    pub manifest: Manifest,
    token: String,
    next_id: u64,
}
impl Client {
    pub fn connect(instance: &Path) -> Result<Self> {
        check_private(instance, false)?;
        let canonical = instance.canonicalize()?;
        let instance = canonical.as_path();
        check_private(instance.parent().context("instance parent")?, true)?;
        let manifest: Manifest = serde_json::from_slice(&fs::read(instance)?)?;
        ensure!(manifest.version == VERSION, "unsupported protocol version");
        ensure!(
            manifest.token_file.parent() == instance.parent()
                && manifest.socket.parent() == instance.parent(),
            "endpoint must be inside instance directory (use the absolute manifest path)"
        );
        check_private(&manifest.token_file, false)?;
        check_private(&manifest.socket, false)?;
        let token = fs::read_to_string(&manifest.token_file)?;
        Ok(Self {
            manifest,
            token,
            next_id: 1,
        })
    }
    pub fn request(&mut self, command: Command) -> Result<Response> {
        command.validate()?;
        let mut stream = UnixStream::connect(&self.manifest.socket)?;
        peer_is_owner(&stream)?;
        stream.set_read_timeout(Some(Duration::from_secs(15)))?;
        stream.set_write_timeout(Some(Duration::from_secs(2)))?;
        let id = self.next_id;
        self.next_id += 1;
        write_message(
            &mut stream,
            &Request {
                version: VERSION,
                token: self.token.clone(),
                id,
                session: self.manifest.session.clone(),
                window: self.manifest.window.clone(),
                command,
            },
        )?;
        let response: Response = read_message(&mut stream, 2 * 1024 * 1024)?;
        ensure!(
            response.id == id && response.version == VERSION,
            "mismatched reply"
        );
        Ok(response)
    }
    pub fn call(&mut self, command: Command) -> Result<Output> {
        match self.request(command)?.result {
            Reply::Ok { output } => Ok(output),
            Reply::Error { error, .. } => bail!(error),
        }
    }
}
fn check_private(path: &Path, directory: bool) -> Result<()> {
    let meta = fs::symlink_metadata(path)?;
    ensure!(
        !meta.file_type().is_symlink() && meta.is_dir() == directory,
        "invalid session path type"
    );
    ensure!(meta.mode() & 0o077 == 0, "session path is not private");
    // SAFETY: geteuid has no preconditions.
    ensure!(
        meta.uid() == unsafe { libc::geteuid() },
        "session path owner differs"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn session_ownership_authentication_socket_and_limits() {
        let parent = tempfile::tempdir().unwrap();
        let directory = parent.path().join("s");
        let (session, listener) = Session::create(&directory, "Fixture".into()).unwrap();
        assert_eq!(fs::metadata(&directory).unwrap().mode() & 0o777, 0o700);
        assert!(Session::create(&directory, "Other".into()).is_err());
        let mut client = Client::connect(&directory.join("instance.json")).unwrap();
        let thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            peer_is_owner(&stream).unwrap();
            let mut request: Request = read_message(&mut stream, MAX_MESSAGE).unwrap();
            session.authenticate(&request).unwrap();
            let token = request.token.clone();
            request.window = "wrong-window".into();
            assert_eq!(
                session.authenticate(&request).unwrap_err().code,
                "wrong_scope"
            );
            request.window = session.manifest.window.clone();
            request.version = VERSION + 1;
            assert_eq!(
                session.authenticate(&request).unwrap_err().code,
                "protocol_version"
            );
            request.version = VERSION;
            request.token = "bad".into();
            assert_eq!(
                session.authenticate(&request).unwrap_err().code,
                "unauthorized"
            );
            request.token = token;
            session.authenticate(&request).unwrap();
            write_message(
                &mut stream,
                &Response {
                    version: VERSION,
                    id: request.id,
                    result: Reply::Ok {
                        output: Output::Hello {
                            session: session.manifest.session.clone(),
                            window: "main".into(),
                            title: "Fixture".into(),
                            capabilities: vec![],
                        },
                    },
                },
            )
            .unwrap();
        });
        assert!(matches!(
            client.call(Command::Hello).unwrap(),
            Output::Hello { .. }
        ));
        thread.join().unwrap();
        assert!(!directory.join("pilot.sock").exists());
        let (mut a, mut b) = UnixStream::pair().unwrap();
        a.write_all(b"12345\n").unwrap();
        assert!(read_message::<serde_json::Value>(&mut b, 4).is_err());
    }
}
