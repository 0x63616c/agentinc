//! AppKit presentation for the Rust updater. All calls run on the app's main thread.
use ainc_release::Manifest;
#[cfg(any(test, target_os = "macos"))]
use pulldown_cmark::{Event, Options, Parser, html};
#[cfg(target_os = "macos")]
use std::ffi::CString;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

static ACTION: AtomicI32 = AtomicI32::new(0);
static AUTOMATIC: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn ainc_update_offer(
        version: *const i8,
        current: *const i8,
        html: *const i8,
        automatic: bool,
        ready: bool,
    );
    fn ainc_update_status(message: *const i8);
    fn ainc_update_progress(message: *const i8, received: u64, total: u64);
    fn ainc_update_close();
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

#[cfg(target_os = "macos")]
pub fn offer(manifest: &Manifest, automatic: bool, ready: bool, changelog: bool) {
    let version = cstring(&manifest.version.to_string());
    let current = cstring(ainc_release::VERSION);
    let notes = cstring(&notes_html(if changelog {
        &manifest.changelog
    } else {
        &manifest.notes
    }));
    unsafe {
        ainc_update_offer(
            version.as_ptr(),
            current.as_ptr(),
            notes.as_ptr(),
            automatic,
            ready,
        )
    }
}

#[cfg(not(target_os = "macos"))]
pub fn offer(_: &Manifest, _: bool, _: bool, _: bool) {}

#[cfg(target_os = "macos")]
pub fn status(message: &str) {
    let message = cstring(message);
    unsafe { ainc_update_status(message.as_ptr()) }
}

#[cfg(not(target_os = "macos"))]
pub fn status(_: &str) {}

#[cfg(target_os = "macos")]
pub fn progress(received: u64, total: u64) {
    let message = cstring("Downloading update...");
    unsafe { ainc_update_progress(message.as_ptr(), received, total) }
}

#[cfg(not(target_os = "macos"))]
pub fn progress(_: u64, _: u64) {}

#[cfg(target_os = "macos")]
pub fn close() {
    unsafe { ainc_update_close() }
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
}
