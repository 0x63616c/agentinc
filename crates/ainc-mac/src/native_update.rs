//! AppKit presentation for the Rust updater. All calls run on the app's main thread.
use ainc_release::Manifest;
#[cfg(any(test, target_os = "macos"))]
use pulldown_cmark::{Event, Options, Parser, html};
#[cfg(target_os = "macos")]
use std::ffi::CString;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

static ACTION: AtomicI32 = AtomicI32::new(0);
static AUTOMATIC: AtomicBool = AtomicBool::new(false);

pub enum Status<'a> {
    Checking,
    UpToDate,
    Failed(&'a str),
    Info(&'a str),
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn ainc_update_offer(
        version: *const i8,
        current: *const i8,
        html: *const i8,
        automatic: bool,
        ready: bool,
        changelog: bool,
    );
    fn ainc_update_status(message: *const i8, current: *const i8, kind: i32);
    fn ainc_update_progress(message: *const i8, received: u64, total: u64);
    fn ainc_update_close();
    #[cfg(ainc_upgrade_test)]
    fn ainc_update_test_click_install();
    #[cfg(feature = "automation")]
    fn ainc_update_capture(path: *const i8, progress: bool) -> bool;
    #[cfg(feature = "automation")]
    fn ainc_update_smoke_init();
}

#[unsafe(no_mangle)]
extern "C" fn ainc_update_action(action: i32, automatic: bool) {
    AUTOMATIC.store(automatic, Ordering::Relaxed);
    ACTION.store(action, Ordering::Release);
}

pub fn take_action() -> Option<(i32, bool)> {
    let action = ACTION.swap(0, Ordering::AcqRel);
    (action != 0).then(|| (action, AUTOMATIC.load(Ordering::Relaxed)))
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
fn offer_html(manifest: &Manifest, current: &str, changelog: bool) -> String {
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

#[cfg(target_os = "macos")]
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

#[cfg(not(target_os = "macos"))]
pub fn offer(_: &Manifest, _: bool, _: bool, _: bool) {}

pub fn status(status: Status<'_>) {
    let (kind, message) = match status {
        Status::Checking => (0, "Checking for updates…"),
        Status::UpToDate => (1, "You’re up to date"),
        Status::Failed(message) => (2, message),
        Status::Info(message) => (3, message),
    };
    #[cfg(target_os = "macos")]
    {
        let message = cstring(message);
        let current = cstring(ainc_release::identity::version().as_str());
        unsafe { ainc_update_status(message.as_ptr(), current.as_ptr(), kind) }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (kind, message);
}

#[cfg(target_os = "macos")]
pub fn progress(received: u64, total: u64) {
    let message = cstring("Downloading update…");
    unsafe { ainc_update_progress(message.as_ptr(), received, total) }
}

#[cfg(not(target_os = "macos"))]
pub fn progress(_: u64, _: u64) {}

#[cfg(target_os = "macos")]
pub fn close() {
    unsafe { ainc_update_close() }
}

#[cfg(all(ainc_upgrade_test, target_os = "macos"))]
pub fn upgrade_test_click_install() {
    unsafe { ainc_update_test_click_install() }
}

#[cfg(not(target_os = "macos"))]
pub fn close() {}

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
    fn offer_renders_every_missed_version_and_changelog_keeps_installed_versions() {
        let mut manifest: Manifest =
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
