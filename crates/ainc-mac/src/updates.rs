//! App-owned update state. Native AppKit windows remain available when the backend is down.
use crate::native_update;
use crate::ui::*;
use ainc_release::{
    Manifest, SignedManifest,
    updater::{self, Preferences},
};
use gpui::{prelude::*, *};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

actions!(updates, [CheckForUpdates, ShowChangelog]);

fn apply_choice(
    preferences: &mut Preferences,
    action: i32,
    automatic: bool,
    version: Option<&str>,
    now: u64,
) {
    preferences.automatic_download = automatic;
    match action {
        1 => preferences.skipped_version = version.map(str::to_owned),
        2 => preferences.remind_after = now + 86400,
        _ => {}
    }
}
#[derive(Clone)]
pub struct Updates(pub Entity<UpdateView>);
#[derive(Clone)]
pub struct UpdateHost(pub WindowHandle<crate::shell::Shell>);
impl Global for UpdateHost {}
impl Global for Updates {}
pub struct UpdateView {
    preferences: Preferences,
    directory: PathBuf,
    message: String,
    release: Option<(SignedManifest, Manifest)>,
    busy: bool,
    ready: bool,
    available: bool,
    progress: Arc<AtomicU64>,
    cancel: Option<tokio::sync::watch::Sender<bool>>,
    downloading: bool,
    install_after_download: bool,
    visible: bool,
    changelog: bool,
    hover: HoverFade,
}
impl HoverHost for UpdateView {
    fn hover_fade(&mut self) -> &mut HoverFade {
        &mut self.hover
    }
}
impl UpdateView {
    fn new(cx: &mut Context<Self>) -> Self {
        let directory = std::env::var_os("AGENTINC_SESSION_PATH")
            .map(PathBuf::from)
            .and_then(|p| p.parent().map(|p| p.join("updates")))
            .unwrap_or_else(|| ainc_release::identity::support_dir().join("updates"));
        let loaded = Preferences::load(&directory.join("preferences.json"));
        let message = loaded
            .as_ref()
            .err()
            .map(|e| e.to_string())
            .unwrap_or_else(|| format!("AgentInc {}", ainc_release::VERSION));
        let preferences = loaded.unwrap_or_default();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_secs(60))
                    .await;
                if this
                    .update(cx, |this, cx| {
                        if this.preferences.due(updater::now()) && !this.busy {
                            this.check(false, cx);
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
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
        if directory.join("feed.json").is_file() && !ainc_release::update_public_key().is_empty() {
            let saved = directory.clone();
            let request = cx.background_executor().spawn(async move {
                let signed: SignedManifest =
                    serde_json::from_slice(&std::fs::read(saved.join("feed.json"))?)?;
                let manifest =
                    updater::verify_download(&signed, ainc_release::update_public_key(), &saved)?;
                anyhow::ensure!(
                    manifest.is_upgrade(ainc_release::VERSION, std::env::consts::ARCH)?,
                    "saved update is no longer newer"
                );
                anyhow::Ok((signed, manifest))
            });
            cx.spawn(async move |this, cx| {
                if let Ok(release) = request.await {
                    let _ = this.update(cx, |this, cx| {
                        if !this.busy
                            && this
                                .release
                                .as_ref()
                                .is_none_or(|(_, manifest)| manifest.version <= release.1.version)
                        {
                            this.release = Some(release);
                            this.available = true;
                            this.ready = true;
                            this.message = "Update verified and ready to install".into();
                            this.present();
                            cx.notify();
                        }
                    });
                }
            })
            .detach();
        }
        Self {
            preferences,
            directory,
            message,
            release: None,
            busy: false,
            ready: false,
            available: false,
            progress: Arc::new(AtomicU64::new(0)),
            cancel: None,
            downloading: false,
            install_after_download: false,
            visible: false,
            changelog: false,
            hover: HoverFade::default(),
        }
    }
    pub fn is_ready(&self) -> bool {
        self.ready
            && updater::now() >= self.preferences.remind_after
            && self.release.as_ref().is_some_and(|(_, manifest)| {
                self.preferences.skipped_version.as_deref() != Some(&manifest.version.to_string())
            })
    }
    fn save(&mut self) {
        if let Err(error) = self
            .preferences
            .save(&self.directory.join("preferences.json"))
        {
            self.message = error.to_string();
        }
    }
    fn present(&self) {
        if !self.visible {
            return;
        }
        if self.downloading {
            if let Some((_, manifest)) = &self.release {
                native_update::progress(
                    self.progress.load(Ordering::Relaxed),
                    manifest.archive_bytes,
                );
            }
        } else if self.busy {
            native_update::status(&self.message);
        } else if self.available {
            if let Some((_, manifest)) = &self.release {
                native_update::offer(
                    manifest,
                    self.preferences.automatic_download,
                    self.ready,
                    self.changelog,
                );
                #[cfg(ainc_upgrade_test)]
                if std::env::var_os("AINC_UPGRADE_TEST_MODE").is_some() {
                    native_update::upgrade_test_click_install();
                }
            }
        } else {
            native_update::status(&self.message);
        }
    }
    fn poll_native(&mut self, cx: &mut Context<Self>) {
        if self.downloading && self.visible {
            self.present_progress();
        }
        if let Some((action, automatic)) = native_update::take_action() {
            if action == 4 {
                if let Some(cancel) = &self.cancel {
                    let _ = cancel.send(true);
                }
                self.visible = false;
                return;
            }
            let version = self.release.as_ref().map(|(_, m)| m.version.to_string());
            apply_choice(
                &mut self.preferences,
                action,
                automatic,
                version.as_deref(),
                updater::now(),
            );
            match action {
                1 => {
                    self.visible = false;
                }
                2 => {
                    self.visible = false;
                }
                3 => {
                    if self.ready {
                        self.install(cx);
                    } else {
                        self.install_after_download = true;
                        self.download(cx);
                    }
                }
                _ => {}
            }
            self.save();
            cx.notify();
        }
    }
    fn present_progress(&self) {
        if let Some((_, manifest)) = &self.release {
            native_update::progress(
                self.progress.load(Ordering::Relaxed),
                manifest.archive_bytes,
            );
        }
    }
    fn check(&mut self, manual: bool, cx: &mut Context<Self>) {
        if !ainc_release::identity::PRODUCTION {
            self.message = "Updates are available in production builds.".into();
            cx.notify();
            return;
        }
        if self.busy {
            return;
        }
        if manual {
            self.preferences.skipped_version = None;
            self.preferences.remind_after = 0;
        }
        self.busy = true;
        self.message = "Checking for updates…".into();
        self.present();
        let request = cx.background_executor().spawn(async {
            crate::storage::background(updater::check(
                &ainc_release::update_feed_url(),
                ainc_release::update_public_key(),
            ))
        });
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                this.preferences.last_check = updater::now();
                match result {
                    Ok((signed, manifest)) => match manifest
                        .is_upgrade(ainc_release::VERSION, std::env::consts::ARCH)
                    {
                        Ok(true)
                            if manual
                                || this.preferences.skipped_version.as_deref()
                                    != Some(&manifest.version.to_string()) =>
                        {
                            this.message = format!("AgentInc {} is available", manifest.version);
                            this.release = Some((signed, manifest));
                            this.ready = false;
                            this.available = true;
                            if !manual && this.preferences.automatic_download {
                                this.download(cx);
                            }
                        }
                        Ok(_) => {
                            this.message = "You’re up to date".into();
                            this.available = false;
                            this.release = Some((signed, manifest));
                        }
                        Err(error) => this.message = error.to_string(),
                    },
                    Err(error) => this.message = format!("Could not check for updates: {error:#}"),
                }
                this.save();
                this.present();
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn download(&mut self, cx: &mut Context<Self>) {
        let Some((signed, manifest)) = self.release.clone() else {
            return;
        };
        if self.busy {
            return;
        }
        self.busy = true;
        self.downloading = true;
        let (cancel, cancelled) = tokio::sync::watch::channel(false);
        self.cancel = Some(cancel);
        self.message = "Downloading update…".into();
        self.progress.store(0, Ordering::Relaxed);
        let directory = self.directory.clone();
        let progress = self.progress.clone();
        self.present();
        let request = cx.background_executor().spawn(async move {
            std::fs::create_dir_all(&directory)?;
            std::fs::write(directory.join("feed.json"), serde_json::to_vec(&signed)?)?;
            crate::storage::background(updater::download_cancellable(
                &manifest,
                &directory.join("app.tar.gz"),
                progress,
                cancelled,
            ))?;
            updater::verify_download(&signed, ainc_release::update_public_key(), &directory)
                .map(|_| ())
        });
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                this.downloading = false;
                let cancelled = this.cancel.take().is_some_and(|cancel| *cancel.borrow());
                if cancelled {
                    this.install_after_download = false;
                    this.message = "Download canceled".into();
                    native_update::close();
                    cx.notify();
                    return;
                }
                this.ready = result.is_ok();
                this.message = match result {
                    Ok(()) => "Update verified and ready to install".into(),
                    Err(e) => format!("Download failed: {e:#}"),
                };
                if this.ready && this.install_after_download {
                    this.install(cx);
                } else {
                    #[cfg(ainc_upgrade_test)]
                    if this.ready
                        && std::env::var("AINC_UPGRADE_TEST_MODE").as_deref() == Ok("automatic")
                    {
                        this.visible = true;
                    }
                    this.present();
                }
                this.install_after_download = false;
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn install(&mut self, cx: &mut Context<Self>) {
        let result = (|| -> anyhow::Result<()> {
            anyhow::ensure!(self.ready, "download is not verified");
            let host = cx.global::<UpdateHost>().0;
            host.update(cx, |shell, _, cx| shell.flush_for_update(cx))??;
            let executable = std::env::current_exe()?;
            let macos = executable
                .parent()
                .ok_or_else(|| anyhow::anyhow!("app bundle unavailable"))?;
            let app = macos
                .parent()
                .and_then(|p| p.parent())
                .ok_or_else(|| anyhow::anyhow!("app bundle unavailable"))?;
            let discovery = crate::storage::discovery_path()?;
            // The helper verifies the signed feed/archive again after launch.
            let log = std::fs::File::create(self.directory.join("install.log"))?;
            ainc_release::process::prepare_child(&mut std::process::Command::new(
                macos.join("ainc-update"),
            ))
            .arg(app)
            .arg(&self.directory)
            .arg(std::process::id().to_string())
            .arg(discovery)
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                native_update::close();
                cx.quit()
            }
            Err(error) => {
                self.message = format!("Install failed: {error:#}");
                self.present();
                cx.notify();
            }
        }
    }
    pub fn settings(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        self.hover.animate(window);
        let frequency = if self.preferences.interval_hours == 24 {
            0
        } else {
            1
        };
        settings_section(
            "Software updates",
            column()
                .child(settings_row(
                    "Updates",
                    self.message.clone(),
                    Button::new("updates.check", "Check Now")
                        .secondary()
                        .icon("refresh")
                        .enabled(!self.busy)
                        .build(
                            &self.hover,
                            |this: &mut Self, _, cx| {
                                this.visible = true;
                                this.changelog = false;
                                this.check(true, cx);
                            },
                            cx,
                        ),
                ))
                .child(settings_divider())
                .child(settings_row(
                    "Automatic checks",
                    "Check for new versions in the background.",
                    toggle(
                        "updates.auto",
                        "Automatic checks",
                        self.preferences.automatic_checks,
                        true,
                        |this: &mut Self, _, cx| {
                            this.preferences.automatic_checks = !this.preferences.automatic_checks;
                            this.save();
                            cx.notify();
                        },
                        cx,
                    ),
                ))
                .child(settings_divider())
                .child(settings_row(
                    "Check frequency",
                    "How often AgentInc checks for updates.",
                    segmented(
                        "updates.frequency",
                        ["Daily", "Weekly"],
                        frequency,
                        true,
                        &self.hover,
                        |this: &mut Self, index, _, cx| {
                            this.preferences.interval_hours = if index == 0 { 24 } else { 168 };
                            this.save();
                            cx.notify();
                        },
                        cx,
                    ),
                ))
                .child(settings_divider())
                .child(settings_row(
                    "Automatic download",
                    "Download new versions when they become available.",
                    toggle(
                        "updates.download",
                        "Automatic download",
                        self.preferences.automatic_download,
                        true,
                        |this: &mut Self, _, cx| {
                            this.preferences.automatic_download =
                                !this.preferences.automatic_download;
                            this.save();
                            cx.notify();
                        },
                        cx,
                    ),
                ))
                .child(settings_divider())
                .child(settings_row(
                    "Release notes",
                    "See what changed in the latest version.",
                    Button::new("updates.notes", "View Changelog")
                        .secondary()
                        .build(&self.hover, |_: &mut Self, _, cx| open_changelog(cx), cx),
                )),
        )
        .into_any_element()
    }
}

pub fn open(cx: &mut App, check: bool) {
    let view = cx.global::<Updates>().0.clone();
    view.update(cx, |this, cx| {
        this.visible = true;
        if check || this.release.is_none() {
            this.changelog = false;
            this.check(true, cx);
        } else {
            this.present();
        }
    });
}

pub fn open_changelog(cx: &mut App) {
    let view = cx.global::<Updates>().0.clone();
    view.update(cx, |this, cx| {
        this.changelog = true;
        this.visible = true;
        if this.release.is_none() {
            this.check(true, cx);
        } else {
            this.present();
        }
    });
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
        std::fs::write(
            marker,
            format!("{} {}\n", ainc_release::VERSION, std::process::id()),
        )
        .expect("write upgrade test marker");
        return;
    }
    match mode.as_str() {
        "manual" => open(cx, true),
        "automatic" => {
            let view = cx.global::<Updates>().0.clone();
            view.update(cx, |this, cx| {
                this.preferences.automatic_checks = true;
                this.preferences.automatic_download = true;
                this.preferences.last_check = 0;
                this.visible = false;
                this.check(false, cx);
            });
        }
        _ => panic!("unknown upgrade test mode"),
    }
}

#[cfg(test)]
mod tests {
    use super::{Preferences, apply_choice};

    #[test]
    fn native_choices_preserve_skip_remind_and_automatic_download() {
        let mut preferences = Preferences::default();
        apply_choice(&mut preferences, 5, true, Some("0.2.0"), 100);
        assert!(preferences.automatic_download);
        apply_choice(&mut preferences, 1, true, Some("0.2.0"), 100);
        assert_eq!(preferences.skipped_version.as_deref(), Some("0.2.0"));
        apply_choice(&mut preferences, 2, false, Some("0.2.0"), 100);
        assert_eq!(preferences.remind_after, 86500);
        assert!(!preferences.automatic_download);
    }
}
