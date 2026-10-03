//! App-owned settings and shutdown coordination for the custom Sparkle driver.
use crate::native_update::{self, NativeAction, Setting, State};
use crate::ui::*;
use gpui::{prelude::*, *};
use std::sync::atomic::{AtomicBool, Ordering};
use std::{fs::File, path::Path, rc::Rc, time::Duration};

static INSTALLATION_PENDING: AtomicBool = AtomicBool::new(false);
static COMPANION_LAUNCHES: tokio::sync::RwLock<()> = tokio::sync::RwLock::const_new(());

pub(crate) fn installation_pending() -> bool {
    INSTALLATION_PENDING.load(Ordering::Acquire) || native_update::installation_fenced()
}

/// Storage holds this across companion spawn and its readiness handshake. The
/// update drain waits for in-flight startup without blocking the AppKit thread.
pub(crate) async fn companion_launch_guard()
-> anyhow::Result<tokio::sync::RwLockReadGuard<'static, ()>> {
    let guard = COMPANION_LAUNCHES.read().await;
    anyhow::ensure!(
        !installation_pending(),
        "Update installation is preparing; companion startup is paused"
    );
    Ok(guard)
}

actions!(updates, [CheckForUpdates, ShowChangelog]);

#[derive(Clone)]
pub struct Updates(pub Entity<UpdateView>);
impl Global for Updates {}
type BeforeInstall = Rc<dyn Fn(&mut App) -> anyhow::Result<()>>;
pub struct UpdateView {
    state: State,
    draining: bool,
    // Prevent another companion from restarting between drain and app exit.
    shutdown_locks: Vec<File>,
    before_install: Option<BeforeInstall>,
}
impl UpdateView {
    #[cfg(any(test, feature = "fixtures"))]
    pub fn fixture_ready(ready: bool) -> Self {
        Self {
            state: State {
                ready,
                ..State::default()
            },
            draining: false,
            shutdown_locks: Vec::new(),
            before_install: None,
        }
    }
    fn new(cx: &mut Context<Self>) -> Self {
        let directory = std::env::var_os("AINC_SESSION_PATH")
            .map(std::path::PathBuf::from)
            .and_then(|p| p.parent().map(|p| p.join("updates")))
            .unwrap_or_else(|| ainc_release::identity::support_dir().join("updates"));
        native_update::start(&directory.join("preferences.json"));
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                if this.update(cx, |this, cx| this.poll_native(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        Self {
            state: native_update::state(),
            draining: false,
            shutdown_locks: Vec::new(),
            before_install: None,
        }
    }
    /// Flush the shell's state and drafts before permitting installation.
    pub fn on_before_install(&mut self, hook: impl Fn(&mut App) -> anyhow::Result<()> + 'static) {
        self.before_install = Some(Rc::new(hook));
    }
    fn flush(&self, cx: &mut App) -> anyhow::Result<()> {
        self.before_install
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("update host is unavailable"))?(cx)
    }
    pub fn is_ready(&self) -> bool {
        self.state.ready
    }
    fn poll_native(&mut self, cx: &mut Context<Self>) {
        while let Some((action, _)) = native_update::take_action() {
            match action {
                NativeAction::PrepareInstall => self.prepare_install(cx),
                NativeAction::ReleaseInstall => {
                    self.shutdown_locks.clear();
                    INSTALLATION_PENDING.store(false, Ordering::Release);
                }
                #[cfg(ainc_upgrade_test)]
                NativeAction::Downloaded => publish_downloaded_marker(cx),
                _ => {}
            }
        }
        let state = native_update::state();
        if self.state != state {
            self.state = state;
            cx.notify();
        }
    }
    fn prepare_install(&mut self, cx: &mut Context<Self>) {
        if self.draining {
            return;
        }
        let result = (|| -> anyhow::Result<_> {
            self.flush(cx)?;
            crate::daemon::discovery_path()
        })();
        let discovery = match result {
            Ok(path) => path,
            Err(error) => {
                native_update::prepared(Some(&format!("Update postponed: {error:#}")));
                return;
            }
        };
        if !self.shutdown_locks.is_empty() {
            // A canceled AppKit termination can be retried. We already own the
            // stopped profile; only the fresh UI flush above needs repeating.
            native_update::prepared(None);
            return;
        }
        self.draining = true;
        INSTALLATION_PENDING.store(true, Ordering::Release);
        let external = std::env::var_os("AINC_API_URL").is_some();
        let request = cx.background_executor().spawn(async move {
            crate::daemon::block_on(async move {
                if external {
                    let _launches = COMPANION_LAUNCHES.write().await;
                    Ok(Vec::new())
                } else {
                    drain_owned_runtime(&discovery).await
                }
            })
        });
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |this, cx| {
                this.draining = false;
                match result {
                    Ok(locks) => {
                        this.shutdown_locks = locks;
                        // Flush once more after the asynchronous drain: the user
                        // may have edited a draft while shutdown was in progress.
                        match this.flush(cx) {
                            Ok(()) => native_update::prepared(None),
                            result => {
                                this.shutdown_locks.clear();
                                INSTALLATION_PENDING.store(false, Ordering::Release);
                                native_update::prepared(Some(&format!(
                                    "Could not save work; update postponed: {result:?}"
                                )));
                            }
                        }
                    }
                    Err(error) => {
                        INSTALLATION_PENDING.store(false, Ordering::Release);
                        native_update::prepared(Some(&format!(
                            "Could not stop the local runtime; update postponed: {error:#}"
                        )));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub fn settings(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let ui = &mut Ui::new(window, cx);
        settings_section(
            "Software updates",
            column()
                .child(settings_row(
                    "Updates",
                    self.state.message.clone(),
                    Button::new("updates.check", "Check Now")
                        .secondary()
                        .icon(Icon::Refresh)
                        .enabled(self.state.can_check)
                        .build(ui, |_: &mut Self, _, _| native_update::check(false)),
                ))
                .child(settings_divider())
                .child(settings_row(
                    "Automatic checks",
                    "Check for new versions in the background.",
                    Toggle::new("updates.auto", "Automatic checks")
                        .on(self.state.automatic_checks)
                        .enabled(self.state.enabled)
                        .build(ui, |this: &mut Self, _, cx| {
                            native_update::setting(
                                Setting::AutomaticChecks,
                                !this.state.automatic_checks,
                            );
                            this.poll_native(cx);
                        }),
                ))
                .child(settings_divider())
                .child(settings_row(
                    "Check frequency",
                    "How often AgentInc checks for updates.",
                    Segmented::new("updates.frequency", ["Daily", "Weekly"])
                        .selected(usize::from(self.state.weekly))
                        .enabled(self.state.enabled)
                        .build(ui, |this: &mut Self, index, _, cx| {
                            native_update::setting(Setting::Weekly, index == 1);
                            this.poll_native(cx);
                        }),
                ))
                .child(settings_divider())
                .child(settings_row(
                    "Automatic download",
                    "Download new versions when they become available.",
                    Toggle::new("updates.download", "Automatic download")
                        .on(self.state.automatic_download)
                        .enabled(self.state.enabled)
                        .build(ui, |this: &mut Self, _, cx| {
                            native_update::setting(
                                Setting::AutomaticDownload,
                                !this.state.automatic_download,
                            );
                            this.poll_native(cx);
                        }),
                ))
                .child(settings_divider())
                .child(settings_row(
                    "Release notes",
                    "Browse the full release history.",
                    Button::new("updates.notes", "View Release Notes")
                        .secondary()
                        .build(ui, |_: &mut Self, _, cx| open_changelog(cx)),
                )),
        )
        .into_any_element()
    }
}

/// Drain only this profile's authenticated companion. Holding both ownership
/// locks proves its bundled Postgres/Temporal children have finished stopping.
async fn drain_owned_runtime(discovery: &Path) -> anyhow::Result<Vec<File>> {
    use anyhow::Context;
    ainc_client::install_tls_provider();
    let _launches = tokio::time::timeout(Duration::from_secs(120), COMPANION_LAUNCHES.write())
        .await
        .context("companion startup has not completed")?;
    let mut locks = Vec::new();
    let daemon_lock = discovery.with_extension("lock");
    match File::options().read(true).write(true).open(&daemon_lock) {
        Ok(lock) => {
            if lock.try_lock().is_err() {
                let url =
                    std::fs::read_to_string(discovery).context("read owned daemon discovery")?;
                let url = reqwest::Url::parse(url.trim())?;
                anyhow::ensure!(
                    url.scheme() == "http"
                        && matches!(url.host_str(), Some("127.0.0.1" | "[::1]" | "localhost")),
                    "owned daemon must use a loopback URL"
                );
                let token = std::fs::read_to_string(discovery.with_file_name("owner-token"))?;
                let response = reqwest::Client::new()
                    .post(url.join("/internal/drain")?)
                    .bearer_auth(token.trim())
                    .timeout(Duration::from_secs(10))
                    .send()
                    .await;
                match response {
                    Ok(response) => anyhow::ensure!(
                        response.status() == reqwest::StatusCode::ACCEPTED,
                        "daemon refused update drain ({})",
                        response.status()
                    ),
                    // It may have finished draining between the lock probe and HTTP.
                    Err(error) => {
                        lock.try_lock().context(error)?;
                    }
                }
                wait_for_lock(&lock)
                    .await
                    .context("daemon has not drained")?;
            }
            locks.push(lock);
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !discovery.exists() => {}
        Err(error) => return Err(error).context("open daemon ownership lock"),
    }
    let runtime_lock = discovery.with_file_name("runtime").join("owner.lock");
    match File::options().read(true).write(true).open(runtime_lock) {
        Ok(lock) => {
            wait_for_lock(&lock)
                .await
                .context("bundled runtime has not stopped")?;
            locks.push(lock);
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("open runtime ownership lock"),
    }
    Ok(locks)
}

async fn wait_for_lock(lock: &File) -> anyhow::Result<()> {
    tokio::time::timeout(Duration::from_secs(120), async {
        loop {
            match lock.try_lock() {
                Ok(()) => return Ok(()),
                Err(std::fs::TryLockError::WouldBlock) => {
                    tokio::time::sleep(Duration::from_millis(100)).await
                }
                Err(error) => return Err(anyhow::Error::from(error)),
            }
        }
    })
    .await?
}

pub fn open(_: &mut App, _: bool) {
    native_update::check(false);
}
pub fn open_changelog(_: &mut App) {
    native_update::changelog();
}
pub fn init(cx: &mut App) {
    let view = cx.new(UpdateView::new);
    cx.set_global(Updates(view));
    cx.on_action(|_: &CheckForUpdates, cx| open(cx, true));
    cx.on_action(|_: &ShowChangelog, cx| open_changelog(cx));
}

#[cfg(ainc_upgrade_test)]
pub fn start_upgrade_test(cx: &mut App) {
    let Ok(mode) = std::env::var("AINC_UPGRADE_TEST_MODE") else {
        return;
    };
    if std::env::var("AINC_UPGRADE_TEST_FROM").as_deref() != Ok(ainc_release::VERSION) {
        let marker = std::env::var("AINC_UPGRADE_TEST_SUCCESS_FILE").expect("upgrade test marker");
        cx.background_executor()
            .spawn(async move {
                crate::daemon::block_on(async move {
                    // client() starts the exact new bundled companion and waits for
                    // its readiness endpoint, using the isolated discovery profile.
                    let client = crate::daemon::client().await?;
                    let version = client.version_show().send().await?;
                    anyhow::ensure!(
                        version.version == ainc_release::VERSION,
                        "replacement daemon version mismatch"
                    );
                    client.health_ready().send().await?;
                    let marker = std::path::PathBuf::from(marker);
                    let temporary = marker.with_extension("tmp");
                    std::fs::write(
                        &temporary,
                        format!("{} {}\n", ainc_release::VERSION, std::process::id()),
                    )?;
                    std::fs::rename(temporary, marker)?;
                    anyhow::Ok(())
                })
                .expect("replacement runtime must become ready");
            })
            .detach();
        return;
    }
    match mode.as_str() {
        "manual" => native_update::check(false),
        "automatic" | "download-only" => {
            native_update::setting(Setting::AutomaticChecks, true);
            native_update::setting(Setting::AutomaticDownload, true);
            native_update::check(true);
        }
        _ => panic!("unknown upgrade test mode"),
    }
}

#[cfg(ainc_upgrade_test)]
fn publish_downloaded_marker(cx: &mut App) {
    // Automatic mode may immediately begin drain; this marker is required only
    // by the download-only gate, where the original runtime must remain usable.
    if std::env::var("AINC_UPGRADE_TEST_MODE").as_deref() != Ok("download-only") {
        return;
    }
    let marker = std::env::var("AINC_UPGRADE_TEST_DOWNLOADED_FILE").expect("download-only marker");
    cx.background_executor()
        .spawn(async move {
            crate::daemon::block_on(async move {
                let client = crate::daemon::client().await?;
                let version = client.version_show().send().await?;
                anyhow::ensure!(
                    version.version == ainc_release::VERSION,
                    "cached update must leave the current daemon running"
                );
                client.health_ready().send().await?;
                let marker = std::path::PathBuf::from(marker);
                let temporary = marker.with_extension("tmp");
                std::fs::write(
                    &temporary,
                    format!(
                        "{} {} downloaded-unverified\n",
                        ainc_release::VERSION,
                        std::process::id()
                    ),
                )?;
                std::fs::rename(temporary, marker)?;
                anyhow::Ok(())
            })
            .expect("current runtime must remain ready after inert download");
        })
        .detach();
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn drain_waits_for_inflight_startup_and_blocks_replacement_launches() {
        let directory = tempfile::tempdir().unwrap();
        let discovery = directory.path().join("api-url");
        let launch = companion_launch_guard().await.unwrap();
        INSTALLATION_PENDING.store(true, Ordering::Release);
        let mut drain = Box::pin(drain_owned_runtime(&discovery));
        assert!(futures::poll!(&mut drain).is_pending());
        drop(launch);
        assert!(drain.await.unwrap().is_empty());
        assert!(companion_launch_guard().await.is_err());
        INSTALLATION_PENDING.store(false, Ordering::Release);
        assert!(companion_launch_guard().await.is_ok());
    }

    async fn drain_server(
        discovery: &Path,
        response: &'static str,
        release_daemon: Option<File>,
    ) -> tokio::task::JoinHandle<()> {
        let server = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        std::fs::write(
            discovery,
            format!("http://{}", server.local_addr().unwrap()),
        )
        .unwrap();
        std::fs::write(
            discovery.with_file_name("owner-token"),
            "isolated-owner-token",
        )
        .unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = server.accept().await.unwrap();
            let mut request = Vec::new();
            loop {
                let mut bytes = [0; 1024];
                let count = stream.read(&mut bytes).await.unwrap();
                assert_ne!(count, 0);
                request.extend_from_slice(&bytes[..count]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            let request = String::from_utf8(request).unwrap().to_lowercase();
            assert!(request.starts_with("post /internal/drain http/1.1"));
            assert!(request.contains("authorization: bearer isolated-owner-token\r\n"));
            drop(release_daemon);
            stream.write_all(response.as_bytes()).await.unwrap();
        })
    }

    #[tokio::test]
    async fn install_waits_for_both_owned_daemon_and_runtime_locks() {
        let directory = tempfile::tempdir().unwrap();
        let discovery = directory.path().join("api-url");
        let daemon = File::create(discovery.with_extension("lock")).unwrap();
        daemon.lock().unwrap();
        let runtime = directory.path().join("runtime");
        std::fs::create_dir(&runtime).unwrap();
        let owner = File::create(runtime.join("owner.lock")).unwrap();
        owner.lock().unwrap();
        let server = drain_server(
            &discovery,
            "HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\n\r\n",
            Some(daemon),
        )
        .await;
        let path = discovery.clone();
        let drain = tokio::spawn(async move { drain_owned_runtime(&path).await });
        server.await.unwrap();
        // Observe the actual ownership handoff, rather than guessing its timing.
        let probe = File::open(discovery.with_extension("lock")).unwrap();
        loop {
            if probe.try_lock().is_err() {
                break;
            }
            probe.unlock().unwrap();
            tokio::task::yield_now().await;
        }
        assert!(!drain.is_finished(), "runtime still owns its children");
        drop(owner);
        let locks = drain.await.unwrap().unwrap();
        assert_eq!(locks.len(), 2);
        assert!(
            probe.try_lock().is_err(),
            "restart is blocked until app exit"
        );
        drop(locks);
        probe.try_lock().unwrap();
    }

    #[tokio::test]
    async fn rejected_drain_never_releases_the_install_barrier() {
        let directory = tempfile::tempdir().unwrap();
        let discovery = directory.path().join("api-url");
        let daemon = File::create(discovery.with_extension("lock")).unwrap();
        daemon.lock().unwrap();
        let server = drain_server(
            &discovery,
            "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n",
            None,
        )
        .await;
        let error = drain_owned_runtime(&discovery).await.unwrap_err();
        assert!(error.to_string().contains("daemon refused update drain"));
        server.await.unwrap();
        let probe = File::open(discovery.with_extension("lock")).unwrap();
        assert!(probe.try_lock().is_err());
    }

    #[tokio::test]
    async fn stopped_profile_needs_no_network_and_missing_ownership_fails_closed() {
        let directory = tempfile::tempdir().unwrap();
        let discovery = directory.path().join("api-url");
        assert!(drain_owned_runtime(&discovery).await.unwrap().is_empty());
        std::fs::write(&discovery, "http://127.0.0.1:1").unwrap();
        assert!(drain_owned_runtime(&discovery).await.is_err());
        let lock = File::create(discovery.with_extension("lock")).unwrap();
        drop(lock);
        assert_eq!(drain_owned_runtime(&discovery).await.unwrap().len(), 1);
    }
}
