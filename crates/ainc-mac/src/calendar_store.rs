//! The Mac's calendar store, read through EventKit (see `calendar_bridge.m`).
//! It includes every calendar the Mac syncs, such as iCloud and Google.
use ainc_client::types::ImportedEvent;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    /// The system has not asked yet. Only the macOS store reports it.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    NotAsked,
    /// Refused, restricted, or write-only: AgentInc cannot read events.
    Denied,
    Granted,
}

/// Where imported events come from. Tests substitute a fixed source.
pub trait CalendarSource: Send + Sync + 'static {
    fn access(&self) -> Access;
    /// Ask once through the system prompt. Blocks until the person answers.
    fn request(&self) -> Access;
    /// Every event overlapping `[start, end)` in Unix seconds.
    fn events(&self, start: i64, end: i64) -> anyhow::Result<Vec<ImportedEvent>>;
}

#[cfg(target_os = "macos")]
mod eventkit {
    use super::*;
    use std::ffi::{CStr, c_char, c_void};

    unsafe extern "C" {
        fn agentinc_calendar_status() -> i32;
        fn agentinc_calendar_request(
            context: *mut c_void,
            callback: extern "C" fn(*mut c_void, i32),
        );
        fn agentinc_calendar_events(start: f64, end: f64) -> *mut c_char;
        fn agentinc_calendar_free(json: *mut c_char);
    }

    extern "C" fn granted(context: *mut c_void, granted: i32) {
        // SAFETY: `request` passes a leaked sender that only this callback reclaims.
        let sender = unsafe { Box::from_raw(context.cast::<std::sync::mpsc::Sender<bool>>()) };
        let _ = sender.send(granted != 0);
    }

    pub struct EventKit;
    impl CalendarSource for EventKit {
        fn access(&self) -> Access {
            // SAFETY: a plain status read with no arguments.
            match unsafe { agentinc_calendar_status() } {
                0 => Access::NotAsked,
                3 => Access::Granted,
                _ => Access::Denied,
            }
        }
        fn request(&self) -> Access {
            let (sender, answer) = std::sync::mpsc::channel();
            let context = Box::into_raw(Box::new(sender)).cast::<c_void>();
            // SAFETY: EventKit calls `granted` exactly once with `context`.
            unsafe { agentinc_calendar_request(context, granted) };
            match answer.recv() {
                Ok(true) => Access::Granted,
                _ => self.access(),
            }
        }
        fn events(&self, start: i64, end: i64) -> anyhow::Result<Vec<ImportedEvent>> {
            // SAFETY: the bridge returns a malloc'd UTF-8 string or NULL.
            let raw = unsafe { agentinc_calendar_events(start as f64, end as f64) };
            anyhow::ensure!(!raw.is_null(), "Calendar access is not granted.");
            // SAFETY: `raw` is a valid NUL-terminated string until it is freed below.
            let json = unsafe { CStr::from_ptr(raw) }.to_bytes().to_vec();
            // SAFETY: freed exactly once, after the copy above.
            unsafe { agentinc_calendar_free(raw) };
            Ok(serde_json::from_slice(&json)?)
        }
    }
}
#[cfg(target_os = "macos")]
pub use eventkit::EventKit;

/// The platform's calendar store.
pub fn system() -> std::sync::Arc<dyn CalendarSource> {
    #[cfg(target_os = "macos")]
    return std::sync::Arc::new(EventKit);
    #[cfg(not(target_os = "macos"))]
    return std::sync::Arc::new(Unavailable);
}

/// No calendar store on this platform.
pub struct Unavailable;
impl CalendarSource for Unavailable {
    fn access(&self) -> Access {
        Access::Denied
    }
    fn request(&self) -> Access {
        Access::Denied
    }
    fn events(&self, _: i64, _: i64) -> anyhow::Result<Vec<ImportedEvent>> {
        anyhow::bail!("This Mac has no calendar store.")
    }
}
