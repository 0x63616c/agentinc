//! AgentInc's custom Sparkle user driver. Calls run on the app's main thread.
#[cfg(all(feature = "automation", target_os = "macos"))]
use ainc_release::Manifest;
#[cfg(any(test, target_os = "macos"))]
use pulldown_cmark::{Event, Options, Parser, html};
#[cfg(target_os = "macos")]
use std::ffi::{CStr, CString};
use std::{collections::VecDeque, sync::Mutex};

static ACTIONS: Mutex<VecDeque<(NativeAction, bool)>> = Mutex::new(VecDeque::new());

/// Native callback codes shared with the custom Sparkle user driver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeAction {
    Skip = 1,
    Later = 2,
    Install = 3,
    CancelDownload = 4,
    AutomaticChanged = 5,
    Retry = 6,
    Dismiss = 7,
    PrepareInstall = 8,
    ReleaseInstall = 9,
    Downloaded = 10,
}
impl NativeAction {
    pub fn from_code(code: i32) -> Option<Self> {
        Some(match code {
            1 => Self::Skip,
            2 => Self::Later,
            3 => Self::Install,
            4 => Self::CancelDownload,
            5 => Self::AutomaticChanged,
            6 => Self::Retry,
            7 => Self::Dismiss,
            8 => Self::PrepareInstall,
            9 => Self::ReleaseInstall,
            10 => Self::Downloaded,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy)]
pub enum Setting {
    AutomaticChecks = 0,
    AutomaticDownload = 1,
    Weekly = 2,
}

#[derive(Clone, Default, PartialEq)]
pub struct State {
    pub automatic_checks: bool,
    pub automatic_download: bool,
    pub weekly: bool,
    pub ready: bool,
    pub can_check: bool,
    pub enabled: bool,
    pub message: String,
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn ainc_restore_relaunch_profile(version: *const i8) -> *const i8;
    fn ainc_sparkle_installation_fenced() -> bool;
    fn ainc_sparkle_start(legacy: *const i8, version: *const i8) -> bool;
    fn ainc_sparkle_check(background: bool);
    fn ainc_sparkle_changelog();
    fn ainc_sparkle_message() -> *const i8;
    fn ainc_sparkle_state() -> u32;
    fn ainc_sparkle_setting(setting: i32, enabled: bool);
    fn ainc_sparkle_prepared(error: *const i8);
    #[cfg(feature = "automation")]
    fn ainc_update_offer(
        version: *const i8,
        current: *const i8,
        html: *const i8,
        automatic: bool,
        ready: bool,
        changelog: bool,
    );
    #[cfg(feature = "automation")]
    fn ainc_update_progress(message: *const i8, received: u64, total: u64);
    #[cfg(feature = "automation")]
    fn ainc_update_close();
    #[cfg(feature = "automation")]
    fn ainc_update_capture(path: *const i8, progress: bool) -> bool;
    #[cfg(feature = "automation")]
    fn ainc_update_smoke_init();
}

pub(crate) fn installation_fenced() -> bool {
    #[cfg(target_os = "macos")]
    {
        unsafe { ainc_sparkle_installation_fenced() }
    }
    #[cfg(not(target_os = "macos"))]
    false
}

/// Restore supported production overrides before threads or profile reads.
/// The shipping native handoff excludes all upgrade-test feed/key variables.
pub fn restore_relaunch_environment() -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let version = cstring(ainc_release::VERSION);
        let error = unsafe { ainc_restore_relaunch_profile(version.as_ptr()) };
        if !error.is_null() {
            anyhow::bail!("{}", unsafe { CStr::from_ptr(error) }.to_string_lossy());
        }
    }
    Ok(())
}

pub fn start(legacy: &std::path::Path) {
    #[cfg(target_os = "macos")]
    if ainc_release::identity::PRODUCTION {
        let legacy = cstring(&legacy.to_string_lossy());
        let version = cstring(ainc_release::VERSION);
        unsafe { ainc_sparkle_start(legacy.as_ptr(), version.as_ptr()) };
    }
    #[cfg(not(target_os = "macos"))]
    let _ = legacy;
}

pub fn state() -> State {
    #[cfg(target_os = "macos")]
    {
        let bits = unsafe { ainc_sparkle_state() };
        State {
            automatic_checks: bits & 1 != 0,
            automatic_download: bits & 2 != 0,
            weekly: bits & 4 != 0,
            ready: bits & 8 != 0,
            can_check: bits & 16 != 0,
            enabled: bits & 32 != 0,
            message: unsafe { CStr::from_ptr(ainc_sparkle_message()) }
                .to_string_lossy()
                .into_owned(),
        }
    }
    #[cfg(not(target_os = "macos"))]
    State::default()
}

pub fn check(background: bool) {
    #[cfg(target_os = "macos")]
    unsafe {
        ainc_sparkle_check(background)
    };
    #[cfg(not(target_os = "macos"))]
    let _ = background;
}

pub fn changelog() {
    #[cfg(target_os = "macos")]
    unsafe {
        ainc_sparkle_changelog()
    };
}

pub fn setting(setting: Setting, enabled: bool) {
    #[cfg(target_os = "macos")]
    unsafe {
        ainc_sparkle_setting(setting as i32, enabled)
    };
    #[cfg(not(target_os = "macos"))]
    let _ = (setting, enabled);
}

pub fn prepared(error: Option<&str>) {
    #[cfg(target_os = "macos")]
    {
        let error = error.map(cstring);
        unsafe { ainc_sparkle_prepared(error.as_ref().map_or(std::ptr::null(), |s| s.as_ptr())) };
    }
    #[cfg(not(target_os = "macos"))]
    let _ = error;
}

#[unsafe(no_mangle)]
extern "C" fn ainc_update_action(action: i32, automatic: bool) {
    if let Some(action) = NativeAction::from_code(action) {
        ACTIONS
            .lock()
            .expect("native update actions")
            .push_back((action, automatic));
    }
}

pub fn take_action() -> Option<(NativeAction, bool)> {
    ACTIONS.lock().expect("native update actions").pop_front()
}

#[cfg(target_os = "macos")]
fn cstring(text: &str) -> CString {
    CString::new(text.replace('\0', "")).expect("NUL stripped")
}

#[cfg(any(test, target_os = "macos"))]
pub fn notes_html(markdown: &str) -> String {
    let parser =
        Parser::new_ext(markdown, Options::ENABLE_STRIKETHROUGH).map(|event| match event {
            Event::Html(raw) | Event::InlineHtml(raw) => Event::Text(raw),
            other => other,
        });
    let mut body = String::new();
    html::push_html(&mut body, parser);
    // AppKit's HTML importer paints two bullets for <li>; use paragraphs instead.
    body = body
        .replace("<ul>", "")
        .replace("</ul>", "")
        .replace("<li>", "<p>• ")
        .replace("</li>", "</p>");
    format!(
        "<html><body style=\"font: 13px -apple-system; color: -apple-system-label;\">{body}</body></html>"
    )
}

#[cfg(any(test, target_os = "macos"))]
fn sparkle_notes(markdown: &str, current: &str, history: bool) -> String {
    let selected = if !history {
        ainc_release::notes::parse_changelog(markdown).and_then(|mut releases| {
            let installed = current.parse().ok()?;
            releases.retain(|release| release.version > installed);
            Some(ainc_release::notes::changelog(&releases))
        })
    } else {
        None
    };
    let markdown = selected.as_deref().unwrap_or(markdown);
    notes_html(
        markdown
            .strip_prefix("# AgentInc changelog\n\n")
            .unwrap_or(markdown),
    )
}

// AppKit copies the returned UTF-8 string, then returns ownership to Rust.
#[cfg(target_os = "macos")]
#[unsafe(no_mangle)]
unsafe extern "C" fn ainc_update_format_notes(
    markdown: *const i8,
    current: *const i8,
    history: bool,
) -> *mut i8 {
    let markdown = unsafe { CStr::from_ptr(markdown) }.to_string_lossy();
    let current = unsafe { CStr::from_ptr(current) }.to_string_lossy();
    cstring(&sparkle_notes(&markdown, &current, history)).into_raw()
}

#[cfg(target_os = "macos")]
#[unsafe(no_mangle)]
unsafe extern "C" fn ainc_update_free_notes(text: *mut i8) {
    drop(unsafe { CString::from_raw(text) });
}

#[cfg(any(test, all(feature = "automation", target_os = "macos")))]
fn offer_html(manifest: &ainc_release::Manifest, current: &str, changelog: bool) -> String {
    if changelog {
        notes_html(&manifest.changelog)
    } else {
        let notes = manifest.notes_since(current);
        // The offer already has a title; start with the newest version section.
        notes_html(
            notes
                .strip_prefix("# AgentInc changelog\n\n")
                .unwrap_or(&notes),
        )
    }
}

#[cfg(all(feature = "automation", target_os = "macos"))]
pub fn offer(manifest: &Manifest, automatic: bool, ready: bool, changelog: bool) {
    let version = cstring(&manifest.version.to_string());
    let current = cstring(ainc_release::VERSION);
    let notes = cstring(&offer_html(manifest, ainc_release::VERSION, changelog));
    unsafe {
        ainc_update_offer(
            version.as_ptr(),
            current.as_ptr(),
            notes.as_ptr(),
            automatic,
            ready,
            changelog,
        )
    }
}

#[cfg(all(feature = "automation", target_os = "macos"))]
pub fn progress(received: u64, total: u64) {
    let message = cstring("Downloading update...");
    unsafe { ainc_update_progress(message.as_ptr(), received, total) }
}

#[cfg(all(feature = "automation", target_os = "macos"))]
pub fn close() {
    unsafe { ainc_update_close() }
}

#[cfg(all(feature = "automation", target_os = "macos"))]
pub fn smoke(manifest: &Manifest, directory: &std::path::Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(directory)?;
    unsafe { ainc_update_smoke_init() };
    offer(manifest, false, false, false);
    let before = cstring(&directory.join("offer.png").to_string_lossy());
    anyhow::ensure!(
        unsafe { ainc_update_capture(before.as_ptr(), false) },
        "offer capture failed"
    );
    progress(manifest.archive_bytes / 3, manifest.archive_bytes);
    let after = cstring(&directory.join("progress.png").to_string_lossy());
    anyhow::ensure!(
        unsafe { ainc_update_capture(after.as_ptr(), true) },
        "progress capture failed"
    );
    close();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_is_formatted_and_raw_html_is_escaped() {
        let html = notes_html(
            "# Changes\n\n- **Fast** updates\n- [Details](https://example.com)\n\n<script>alert(1)</script>",
        );
        assert!(html.contains("<h1>Changes</h1>"));
        assert!(html.contains("<strong>Fast</strong>"));
        assert!(html.contains("<a href=\"https://example.com\">Details</a>"));
        assert!(html.contains("<p>• <strong>Fast</strong> updates</p>"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>"));
    }

    #[test]
    fn sparkle_markdown_history_keeps_formatting_and_filters_installed_releases() {
        let markdown = "# AgentInc changelog\n\n## AgentInc 0.6.0\n\n- **Delta** updates\n\n## AgentInc 0.5.0\n\n- Old change\n";
        let offer = sparkle_notes(markdown, "0.5.0", false);
        assert!(offer.contains("<strong>Delta</strong>"));
        assert!(offer.contains("<h2>AgentInc 0.6.0</h2>"));
        assert!(!offer.contains("Old change"));
        assert!(sparkle_notes(markdown, "0.5.0", true).contains("Old change"));
    }

    #[test]
    fn offer_renders_every_missed_version_and_changelog_keeps_installed_versions() {
        let mut manifest: ainc_release::Manifest =
            serde_json::from_str(include_str!("../tests/fixtures/update-manifest.json")).unwrap();
        manifest.version = "0.4.3".parse().unwrap();
        manifest.changelog = ainc_release::notes::changelog(&[0, 1, 2, 3].map(|patch| {
            ainc_release::notes::ReleaseNotes {
                version: format!("0.4.{patch}").parse().unwrap(),
                notes: format!("- **Change {patch}**"),
            }
        }));
        let html = offer_html(&manifest, "0.4.0", false);
        assert!(html.contains("<h2>AgentInc 0.4.3</h2>"));
        assert!(html.contains("<h2>AgentInc 0.4.2</h2>"));
        assert!(html.contains("<h2>AgentInc 0.4.1</h2>"));
        assert!(!html.contains("AgentInc 0.4.0"));
        assert!(html.contains("<strong>Change 1</strong>"));
        assert!(offer_html(&manifest, "0.4.3", true).contains("<h2>AgentInc 0.4.0</h2>"));
        manifest.changelog = "Legacy history".into();
        assert_eq!(
            offer_html(&manifest, "0.4.0", false),
            notes_html(&manifest.notes)
        );
    }
}
