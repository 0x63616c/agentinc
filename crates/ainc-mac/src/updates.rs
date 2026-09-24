//! App-owned update UI. It remains available when the product backend is down.
use crate::style::*;
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
#[derive(Clone)]
pub struct Updates(pub Entity<UpdateView>);
impl Global for Updates {}
pub struct UpdateView {
    preferences: Preferences,
    directory: PathBuf,
    message: String,
    release: Option<(SignedManifest, Manifest)>,
    busy: bool,
    ready: bool,
    progress: Arc<AtomicU64>,
    changelog: bool,
}
impl UpdateView {
    fn new(cx: &mut Context<Self>) -> Self {
        let directory = std::env::var_os("AGENTINC_SESSION_PATH")
            .map(PathBuf::from)
            .and_then(|p| p.parent().map(|p| p.join("updates")))
            .unwrap_or_else(|| {
                PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                    .join("Library/Application Support/Agentinc OS/updates")
            });
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
        Self {
            preferences,
            directory,
            message,
            release: None,
            busy: false,
            ready: false,
            progress: Arc::new(AtomicU64::new(0)),
            changelog: false,
        }
    }
    fn save(&mut self) {
        if let Err(error) = self
            .preferences
            .save(&self.directory.join("preferences.json"))
        {
            self.message = error.to_string();
        }
    }
    fn check(&mut self, manual: bool, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.message = "Checking for updates…".into();
        let request = cx.background_executor().spawn(async {
            crate::storage::background(updater::check(
                ainc_release::FEED_URL,
                ainc_release::UPDATE_PUBLIC_KEY,
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
                            if !manual && this.preferences.automatic_download {
                                this.download(cx);
                            }
                        }
                        Ok(_) => {
                            this.message = "You’re up to date".into();
                            this.release = None;
                        }
                        Err(error) => this.message = error.to_string(),
                    },
                    Err(error) => this.message = format!("Could not check for updates: {error:#}"),
                }
                this.save();
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
        self.message = "Downloading update…".into();
        self.progress.store(0, Ordering::Relaxed);
        let directory = self.directory.clone();
        let progress = self.progress.clone();
        let request = cx.background_executor().spawn(async move {
            std::fs::create_dir_all(&directory)?;
            std::fs::write(directory.join("feed.json"), serde_json::to_vec(&signed)?)?;
            crate::storage::background(updater::download(
                &manifest,
                &directory.join("app.tar.gz"),
                progress,
            ))
        });
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                this.ready = result.is_ok();
                this.message = match result {
                    Ok(()) => "Update verified and ready to install".into(),
                    Err(e) => format!("Download failed: {e:#}"),
                };
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn install(&mut self, cx: &mut Context<Self>) {
        let result = (|| -> anyhow::Result<()> {
            anyhow::ensure!(self.ready, "download is not verified");
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
            std::process::Command::new(macos.join("ainc-update"))
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
            Ok(()) => cx.quit(),
            Err(error) => {
                self.message = format!("Install failed: {error:#}");
                cx.notify();
            }
        }
    }
    fn button(
        &self,
        id: &'static str,
        label: &'static str,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(id)
            .cursor_pointer()
            .px(px(12.))
            .py(px(8.))
            .rounded(px(6.))
            .bg(rgb(0x292929))
            .role(accesskit::Role::Button)
            .aria_label(label)
            .on_click(cx.listener(move |this, _, window, cx| action(this, window, cx)))
            .child(label)
    }
    pub fn settings(&mut self, cx: &mut Context<Self>) -> AnyElement {
        column()
            .gap(px(12.))
            .child(div().text_size(px(16.)).child("Software updates"))
            .child(self.message.clone())
            .child(self.button(
                "updates.check",
                "Check for Updates",
                |this, _, cx| this.check(true, cx),
                cx,
            ))
            .child(self.button(
                "updates.auto",
                if self.preferences.automatic_checks {
                    "Automatic checks: On"
                } else {
                    "Automatic checks: Off"
                },
                |this, _, cx| {
                    this.preferences.automatic_checks = !this.preferences.automatic_checks;
                    this.save();
                    cx.notify();
                },
                cx,
            ))
            .child(self.button(
                "updates.interval",
                if self.preferences.interval_hours == 24 {
                    "Check daily"
                } else {
                    "Check weekly"
                },
                |this, _, cx| {
                    this.preferences.interval_hours = if this.preferences.interval_hours == 24 {
                        168
                    } else {
                        24
                    };
                    this.save();
                    cx.notify();
                },
                cx,
            ))
            .child(self.button(
                "updates.download",
                if self.preferences.automatic_download {
                    "Automatic download: On"
                } else {
                    "Automatic download: Off"
                },
                |this, _, cx| {
                    this.preferences.automatic_download = !this.preferences.automatic_download;
                    this.save();
                    cx.notify();
                },
                cx,
            ))
            .child(self.button(
                "updates.notes",
                "Release Notes and Changelog",
                |_, _, cx| open(cx, false),
                cx,
            ))
            .into_any_element()
    }
}
impl Render for UpdateView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.busy {
            window.request_animation_frame();
        }
        let mut view = column()
            .id("update-window")
            .size_full()
            .overflow_y_scroll()
            .p(px(28.))
            .gap(px(16.))
            .bg(rgb(0x161616))
            .text_color(rgb(0xeeeeee))
            .text_size(px(14.))
            .child(div().text_size(px(22.)).child("Software Update"))
            .child(self.message.clone());
        if let Some((_, manifest)) = &self.release {
            view = view.child(if self.changelog {
                manifest.changelog.clone()
            } else {
                manifest.notes.clone()
            });
            if self.busy {
                view = view.child(format!(
                    "{} / {} MB",
                    self.progress.load(Ordering::Relaxed) / 1_000_000,
                    manifest.archive_bytes / 1_000_000
                ));
            }
            if !self.busy {
                view = view.child(
                    row()
                        .gap(px(8.))
                        .child(self.button(
                            "updates.install",
                            if self.ready {
                                "Install and Relaunch"
                            } else {
                                "Download Update"
                            },
                            |this, _, cx| {
                                if this.ready {
                                    this.install(cx)
                                } else {
                                    this.download(cx)
                                }
                            },
                            cx,
                        ))
                        .child(self.button(
                            "updates.later",
                            "Remind Me Later",
                            |this, window, cx| {
                                this.preferences.remind_after = updater::now() + 86400;
                                this.save();
                                window.remove_window();
                                cx.notify();
                            },
                            cx,
                        ))
                        .child(self.button(
                            "updates.skip",
                            "Skip This Version",
                            |this, window, cx| {
                                this.preferences.skipped_version =
                                    this.release.as_ref().map(|(_, m)| m.version.to_string());
                                this.save();
                                window.remove_window();
                                cx.notify();
                            },
                            cx,
                        )),
                );
            }
        }
        view.child(self.button(
            "updates.changelog",
            "Full Changelog",
            |this, _, cx| {
                this.changelog = !this.changelog;
                cx.notify();
            },
            cx,
        ))
    }
}
pub fn open(cx: &mut App, check: bool) {
    let view = cx.global::<Updates>().0.clone();
    if check {
        view.update(cx, |this, cx| this.check(true, cx));
    }
    let bounds = Bounds::centered(None, size(px(660.), px(520.)), cx);
    if let Err(error) = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("AgentInc Updates".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        |_, _| view,
    ) {
        log::error!("Update window: {error}");
    }
}
pub fn init(cx: &mut App) {
    let view = cx.new(UpdateView::new);
    cx.set_global(Updates(view));
    cx.on_action(|_: &CheckForUpdates, cx| open(cx, true));
    cx.on_action(|_: &ShowChangelog, cx| open(cx, false));
}
