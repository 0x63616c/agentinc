//! App-owned update state. Native AppKit windows remain available when the backend is down.
use crate::action::{Pending, Run};
use crate::native_update::{self, NativeAction, Status};
use crate::ui::*;
use ainc_release::{
    Manifest, SignedManifest,
    updater::{self, Preferences},
};
use gpui::{prelude::*, *};
use std::{
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

actions!(updates, [CheckForUpdates, ShowChangelog]);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Retry {
    Check,
    Download { install: bool },
    Install,
}

/// Something that happened to the update flow.
#[derive(Debug)]
enum Event {
    /// The user acted in a native update window.
    Native {
        action: NativeAction,
        automatic: bool,
        /// The version on offer, if any.
        version: Option<String>,
        now: u64,
    },
}

/// What the view must do after [`UpdateState::on`], in order.
#[derive(Debug, PartialEq, Eq)]
enum Effect {
    Check,
    Download,
    Install,
    CancelDownload,
    Save,
}

/// The decisions of the update flow, free of windows and I/O.
struct UpdateState {
    preferences: Preferences,
    ready: bool,
    visible: bool,
    install_after_download: bool,
    failure: Option<Retry>,
}
impl UpdateState {
    fn on(&mut self, event: Event) -> Vec<Effect> {
        let Event::Native {
            action,
            automatic,
            version,
            now,
        } = event;
        match action {
            NativeAction::Retry => match self.failure.take() {
                Some(Retry::Check) => vec![Effect::Check],
                Some(Retry::Download { install }) => {
                    self.install_after_download = install;
                    vec![Effect::Download]
                }
                Some(Retry::Install) => vec![Effect::Install],
                None => vec![],
            },
            NativeAction::Dismiss => {
                self.visible = false;
                vec![]
            }
            NativeAction::CancelDownload => {
                self.visible = false;
                vec![Effect::CancelDownload]
            }
            NativeAction::Skip
            | NativeAction::Later
            | NativeAction::Install
            | NativeAction::AutomaticChanged => {
                self.preferences.automatic_download = automatic;
                let mut effects = vec![];
                match action {
                    NativeAction::Skip => {
                        self.preferences.skipped_version = version;
                        self.visible = false;
                    }
                    NativeAction::Later => {
                        self.preferences.remind_after = now + 86400;
                        self.visible = false;
                    }
                    NativeAction::Install if self.ready => effects.push(Effect::Install),
                    NativeAction::Install => {
                        self.install_after_download = true;
                        effects.push(Effect::Download);
                    }
                    _ => {}
                }
                effects.push(Effect::Save);
                effects
            }
        }
    }
}

/// Runs before the installer is spawned; an Err postpones the install.
type BeforeInstall = Rc<dyn Fn(&mut App) -> anyhow::Result<()>>;
#[derive(Clone)]
pub struct Updates(pub Entity<UpdateView>);
impl Global for Updates {}
pub struct UpdateView {
    state: UpdateState,
    directory: PathBuf,
    message: String,
    release: Option<(SignedManifest, Manifest)>,
    busy: Pending,
    available: bool,
    progress: Arc<AtomicU64>,
    cancel: Option<tokio::sync::watch::Sender<bool>>,
    downloading: bool,
    changelog: bool,
    before_install: Option<BeforeInstall>,
}
impl UpdateView {
    fn new(cx: &mut Context<Self>) -> Self {
        let directory = std::env::var_os("AINC_SESSION_PATH")
            .map(PathBuf::from)
            .and_then(|p| p.parent().map(|p| p.join("updates")))
            .unwrap_or_else(|| ainc_release::identity::support_dir().join("updates"));
        let loaded = Preferences::load(&directory.join("preferences.json"));
        let message = match &loaded {
            Ok(_) => format!("AgentInc {}", ainc_release::identity::version()),
            Err(error) => {
                tracing::warn!(%error, "could not read update preferences");
                copy::unavailable("Update preferences", "Defaults are in use")
            }
        };
        let preferences = loaded.unwrap_or_default();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_secs(60))
                    .await;
                if this
                    .update(cx, |this, cx| {
                        if this.state.preferences.due(updater::now()) && !this.busy.busy() {
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
            cx.run(
                &Pending::default(),
                move || {
                    let signed: SignedManifest =
                        serde_json::from_slice(&std::fs::read(saved.join("feed.json"))?)?;
                    let manifest = updater::verify_download(
                        &signed,
                        ainc_release::update_public_key(),
                        &saved,
                    )?;
                    anyhow::ensure!(
                        manifest.is_upgrade(ainc_release::VERSION, std::env::consts::ARCH)?,
                        "saved update is no longer newer"
                    );
                    anyhow::Ok((signed, manifest))
                },
                |this, release, _| {
                    let Ok(release) = release else { return };
                    if !this.busy.busy()
                        && this
                            .release
                            .as_ref()
                            .is_none_or(|(_, manifest)| manifest.version <= release.1.version)
                    {
                        this.release = Some(release);
                        this.available = true;
                        this.state.ready = true;
                        this.message = "Update verified and ready to install".into();
                        this.present();
                    }
                },
            );
        }
        Self {
            state: UpdateState {
                preferences,
                ready: false,
                visible: false,
                install_after_download: false,
                failure: None,
            },
            directory,
            message,
            release: None,
            busy: Pending::default(),
            available: false,
            progress: Arc::new(AtomicU64::new(0)),
            cancel: None,
            downloading: false,
            changelog: false,
            before_install: None,
        }
    }
    /// The shell flushes its state and drafts here before an install.
    pub fn on_before_install(&mut self, hook: impl Fn(&mut App) -> anyhow::Result<()> + 'static) {
        self.before_install = Some(Rc::new(hook));
    }
    pub fn is_ready(&self) -> bool {
        self.state.ready
            && updater::now() >= self.state.preferences.remind_after
            && self.release.as_ref().is_some_and(|(_, manifest)| {
                self.state.preferences.skipped_version.as_deref()
                    != Some(&manifest.version.to_string())
            })
    }
    fn save(&mut self) {
        if let Err(error) = self
            .state
            .preferences
            .save(&self.directory.join("preferences.json"))
        {
            tracing::warn!(%error, "could not save update preferences");
            self.message =
                "Update preferences could not be saved. Changes remain in this window.".into();
        }
    }
    fn present(&self) {
        if !self.state.visible {
            return;
        }
        if self.downloading {
            if let Some((_, manifest)) = &self.release {
                native_update::progress(
                    self.progress.load(Ordering::Relaxed),
                    manifest.archive_bytes,
                );
            }
        } else if self.busy.busy() {
            native_update::status(Status::Checking);
        } else if self.state.failure.is_some() {
            native_update::status(Status::Failed(&self.message));
        } else if self.available || (self.changelog && self.release.is_some()) {
            if let Some((_, manifest)) = &self.release {
                native_update::offer(
                    manifest,
                    self.state.preferences.automatic_download,
                    self.state.ready,
                    self.changelog,
                );
                #[cfg(ainc_upgrade_test)]
                if std::env::var_os("AINC_UPGRADE_TEST_MODE").is_some() {
                    native_update::upgrade_test_click_install();
                }
            }
        } else {
            native_update::status(if self.release.is_some() {
                Status::UpToDate
            } else {
                Status::Info(&self.message)
            });
        }
    }
    fn poll_native(&mut self, cx: &mut Context<Self>) {
        if self.downloading && self.state.visible {
            self.present_progress();
        }
        if let Some((action, automatic)) = native_update::take_action() {
            let version = self.release.as_ref().map(|(_, m)| m.version.to_string());
            let effects = self.state.on(Event::Native {
                action,
                automatic,
                version,
                now: updater::now(),
            });
            for effect in effects {
                match effect {
                    Effect::Check => self.check(true, cx),
                    Effect::Download => self.download(cx),
                    Effect::Install => self.install(cx),
                    Effect::CancelDownload => {
                        if let Some(cancel) = &self.cancel {
                            let _ = cancel.send(true);
                        }
                    }
                    Effect::Save => self.save(),
                }
            }
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
            self.present();
            cx.notify();
            return;
        }
        if self.busy.busy() {
            return;
        }
        if manual {
            self.state.preferences.skipped_version = None;
            self.state.preferences.remind_after = 0;
        }
        self.state.failure = None;
        self.message = "Checking for updates…".into();
        self.present();
        cx.run(
            &self.busy.clone(),
            || {
                crate::daemon::block_on(updater::check(
                    &ainc_release::update_feed_url(),
                    ainc_release::update_public_key(),
                ))
            },
            move |this, result, cx| {
                this.state.preferences.last_check = updater::now();
                match result {
                    Ok((signed, manifest)) => match manifest
                        .is_upgrade(ainc_release::VERSION, std::env::consts::ARCH)
                    {
                        Ok(true)
                            if manual
                                || this.state.preferences.skipped_version.as_deref()
                                    != Some(&manifest.version.to_string()) =>
                        {
                            this.message = format!("AgentInc {} is available", manifest.version);
                            this.release = Some((signed, manifest));
                            this.state.ready = false;
                            this.available = true;
                            if !manual && this.state.preferences.automatic_download {
                                this.download(cx);
                            }
                        }
                        Ok(_) => {
                            this.message = "You’re up to date".into();
                            this.available = false;
                            this.release = Some((signed, manifest));
                        }
                        Err(error) => {
                            tracing::warn!(%error, "could not compare the update version");
                            this.message = copy::unavailable("The update check", "Try again");
                            this.state.failure = Some(Retry::Check);
                        }
                    },
                    Err(failure) => {
                        this.message = failure.message("The update check");
                        this.state.failure = Some(Retry::Check);
                    }
                }
                this.save();
                this.present();
            },
        );
    }
    fn download(&mut self, cx: &mut Context<Self>) {
        let Some((signed, manifest)) = self.release.clone() else {
            return;
        };
        if self.busy.busy() {
            return;
        }
        self.downloading = true;
        self.state.failure = None;
        let (cancel, cancelled) = tokio::sync::watch::channel(false);
        self.cancel = Some(cancel);
        self.message = "Downloading update…".into();
        self.progress.store(0, Ordering::Relaxed);
        let directory = self.directory.clone();
        let progress = self.progress.clone();
        self.present();
        cx.run(
            &self.busy.clone(),
            move || {
                std::fs::create_dir_all(&directory)?;
                std::fs::write(directory.join("feed.json"), serde_json::to_vec(&signed)?)?;
                crate::daemon::block_on(updater::download_cancellable(
                    &manifest,
                    &directory.join("app.tar.gz"),
                    progress,
                    cancelled,
                ))?;
                updater::verify_download(&signed, ainc_release::update_public_key(), &directory)
                    .map(|_| ())
            },
            |this, result, cx| {
                this.downloading = false;
                let cancelled = this.cancel.take().is_some_and(|cancel| *cancel.borrow());
                if cancelled {
                    this.state.install_after_download = false;
                    this.message = "Download cancelled".into();
                    native_update::close();
                    return;
                }
                this.state.ready = result.is_ok();
                this.state.failure = result.is_err().then_some(Retry::Download {
                    install: this.state.install_after_download,
                });
                this.message = match result {
                    Ok(()) => "Update verified and ready to install".into(),
                    Err(failure) => failure.message("The update download"),
                };
                if this.state.ready && this.state.install_after_download {
                    this.install(cx);
                } else {
                    #[cfg(ainc_upgrade_test)]
                    if this.state.ready
                        && std::env::var("AINC_UPGRADE_TEST_MODE").as_deref() == Ok("automatic")
                    {
                        this.state.visible = true;
                    }
                    this.present();
                }
                this.state.install_after_download = false;
            },
        );
    }
    fn install(&mut self, cx: &mut Context<Self>) {
        self.state.failure = None;
        let result = (|| -> anyhow::Result<()> {
            anyhow::ensure!(self.state.ready, "download is not verified");
            if let Some(hook) = self.before_install.clone() {
                hook(cx)?;
            }
            let executable = std::env::current_exe()?;
            let macos = executable
                .parent()
                .ok_or_else(|| anyhow::anyhow!("app bundle unavailable"))?;
            let app = macos
                .parent()
                .and_then(|p| p.parent())
                .ok_or_else(|| anyhow::anyhow!("app bundle unavailable"))?;
            let discovery = crate::daemon::discovery_path()?;
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
                tracing::warn!(
                    error = format!("{error:#}"),
                    "could not start the installer"
                );
                self.message = copy::unavailable("The update install", "Try again");
                self.state.failure = Some(Retry::Install);
                self.present();
                cx.notify();
            }
        }
    }
    pub fn settings(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let ui = &mut Ui::new(window, cx);
        let frequency = if self.state.preferences.interval_hours == 24 {
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
                        .icon(Icon::Refresh)
                        .enabled(!self.busy.busy())
                        .build(ui, |this: &mut Self, _, cx| {
                            this.state.visible = true;
                            this.changelog = false;
                            this.check(true, cx);
                        }),
                ))
                .child(settings_divider())
                .child(settings_row(
                    "Automatic checks",
                    "Check for new versions in the background.",
                    Toggle::new("updates.auto", "Automatic checks")
                        .on(self.state.preferences.automatic_checks)
                        .build(ui, |this: &mut Self, _, cx| {
                            this.state.preferences.automatic_checks =
                                !this.state.preferences.automatic_checks;
                            this.save();
                            cx.notify();
                        }),
                ))
                .child(settings_divider())
                .child(settings_row(
                    "Check frequency",
                    "How often AgentInc checks for updates.",
                    Segmented::new("updates.frequency", ["Daily", "Weekly"])
                        .selected(frequency)
                        .build(ui, |this: &mut Self, index, _, cx| {
                            this.state.preferences.interval_hours =
                                if index == 0 { 24 } else { 168 };
                            this.save();
                            cx.notify();
                        }),
                ))
                .child(settings_divider())
                .child(settings_row(
                    "Automatic download",
                    "Download new versions when they become available.",
                    Toggle::new("updates.download", "Automatic download")
                        .on(self.state.preferences.automatic_download)
                        .build(ui, |this: &mut Self, _, cx| {
                            this.state.preferences.automatic_download =
                                !this.state.preferences.automatic_download;
                            this.save();
                            cx.notify();
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

pub fn open(cx: &mut App, check: bool) {
    let view = cx.global::<Updates>().0.clone();
    view.update(cx, |this, cx| {
        this.state.visible = true;
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
        this.state.visible = true;
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
                this.state.preferences.automatic_checks = true;
                this.state.preferences.automatic_download = true;
                this.state.preferences.last_check = 0;
                this.state.visible = false;
                this.check(false, cx);
            });
        }
        _ => panic!("unknown upgrade test mode"),
    }
}

#[cfg(test)]
mod tests {
    use super::{Effect, Event, NativeAction, Preferences, Retry, UpdateState};

    fn state() -> UpdateState {
        UpdateState {
            preferences: Preferences::default(),
            ready: false,
            visible: true,
            install_after_download: false,
            failure: None,
        }
    }
    fn native(state: &mut UpdateState, action: NativeAction, automatic: bool) -> Vec<Effect> {
        state.on(Event::Native {
            action,
            automatic,
            version: Some("0.2.0".into()),
            now: 100,
        })
    }

    #[test]
    fn native_choices_preserve_skip_remind_and_automatic_download() {
        let mut state = state();
        let effects = native(&mut state, NativeAction::AutomaticChanged, true);
        assert!(state.preferences.automatic_download);
        assert_eq!(effects, [Effect::Save]);
        assert!(state.visible);
        native(&mut state, NativeAction::Skip, true);
        assert_eq!(state.preferences.skipped_version.as_deref(), Some("0.2.0"));
        assert!(!state.visible);
        native(&mut state, NativeAction::Later, false);
        assert_eq!(state.preferences.remind_after, 86500);
        assert!(!state.preferences.automatic_download);
    }

    #[test]
    fn install_downloads_first_unless_the_update_is_ready() {
        let mut state = state();
        assert_eq!(
            native(&mut state, NativeAction::Install, false),
            [Effect::Download, Effect::Save]
        );
        assert!(state.install_after_download);
        state.ready = true;
        assert_eq!(
            native(&mut state, NativeAction::Install, false),
            [Effect::Install, Effect::Save]
        );
    }

    #[test]
    fn retry_replays_the_failed_step_once() {
        let mut state = state();
        assert_eq!(native(&mut state, NativeAction::Retry, false), []);
        state.failure = Some(Retry::Check);
        assert_eq!(
            native(&mut state, NativeAction::Retry, false),
            [Effect::Check]
        );
        assert_eq!(state.failure, None);
        state.failure = Some(Retry::Download { install: true });
        assert_eq!(
            native(&mut state, NativeAction::Retry, false),
            [Effect::Download]
        );
        assert!(state.install_after_download);
        state.failure = Some(Retry::Install);
        assert_eq!(
            native(&mut state, NativeAction::Retry, false),
            [Effect::Install]
        );
    }

    #[test]
    fn cancel_and_dismiss_hide_the_window_without_saving() {
        let mut state = state();
        assert_eq!(
            native(&mut state, NativeAction::CancelDownload, true),
            [Effect::CancelDownload]
        );
        assert!(!state.visible);
        assert!(!state.preferences.automatic_download);
        state.visible = true;
        assert_eq!(native(&mut state, NativeAction::Dismiss, true), []);
        assert!(!state.visible);
    }
}
