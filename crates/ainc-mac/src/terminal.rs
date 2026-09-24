//! The AppKit host for GhosttyKit's libghostty renderer and exec backend.
#[cfg(target_os = "macos")]
mod macos {
    use crate::shell::{Control, Shell};
    use anyhow::{Context as _, Result, anyhow};
    use gpui::{AnyWindowHandle, AsyncApp, WeakEntity, Window};
    use libloading::Library;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::{
        ffi::{CString, c_void},
        path::PathBuf,
        ptr::NonNull,
    };

    type Navigate = unsafe extern "C" fn(*mut c_void, i32);
    type Create = unsafe extern "C" fn(
        *mut c_void,
        *const std::ffi::c_char,
        *const std::ffi::c_char,
        Option<Navigate>,
        *mut c_void,
    ) -> *mut c_void;
    type SetFrame = unsafe extern "C" fn(*mut c_void, f64, f64, f64, f64, bool, bool);
    type Destroy = unsafe extern "C" fn(*mut c_void);

    struct Navigation {
        app: AsyncApp,
        window: AnyWindowHandle,
        shell: WeakEntity<Shell>,
    }

    unsafe extern "C" fn navigate(context: *mut c_void, number: i32) {
        // SAFETY: the bridge retains this pointer only while TerminalHost and
        // its Navigation box are alive, and invokes it synchronously on the UI thread.
        let Some(navigation) = (unsafe { (context as *const Navigation).as_ref() }) else {
            return;
        };
        let Ok(number) = u8::try_from(number) else {
            return;
        };
        let Some(route) = crate::model::Route::from_shortcut(number) else {
            return;
        };
        navigation.app.update(|cx| {
            let _ = navigation.window.update(cx, |_, window, cx| {
                let _ = navigation.shell.update(cx, |shell, cx| {
                    shell.dispatch(Control::Navigate(route), window, cx);
                });
            });
        });
    }

    pub struct TerminalHost {
        _library: Library,
        raw: NonNull<c_void>,
        set_frame: SetFrame,
        destroy: Destroy,
        _navigation: Box<Navigation>,
        frame: (f64, f64, f64, f64),
    }

    impl TerminalHost {
        pub fn new(window: &Window, app: AsyncApp, shell: WeakEntity<Shell>) -> Result<Self> {
            let handle = HasWindowHandle::window_handle(window)
                .map_err(|error| anyhow!("GPUI native window: {error}"))?;
            let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
                return Err(anyhow!("Ghostty requires an AppKit window"));
            };
            let home = std::env::var_os("HOME").context("HOME is missing")?;
            let home = CString::new(home.to_string_lossy().as_bytes()).context("invalid HOME")?;
            let colors = CString::new(crate::ui::terminal_colors())?;
            let library = unsafe { Library::new(Self::library_path()?) }
                .context("load AgentInc Ghostty bridge")?;
            // SAFETY: the library is pinned in this struct for the lifetime of
            // these function pointers and its ABI is declared in Bridge.swift.
            let (create, set_frame, destroy): (Create, SetFrame, Destroy) = unsafe {
                (
                    *library.get(b"agentinc_ghostty_create")?,
                    *library.get(b"agentinc_ghostty_set_frame")?,
                    *library.get(b"agentinc_ghostty_destroy")?,
                )
            };
            let navigation = Box::new(Navigation {
                app,
                window: window.window_handle(),
                shell,
            });
            // SAFETY: GPUI's AppKit handle is a live NSView pointer. Swift
            // attaches a child NSView on the same main thread and retains it.
            let raw = unsafe {
                create(
                    handle.ns_view.as_ptr(),
                    home.as_ptr(),
                    colors.as_ptr(),
                    Some(navigate),
                    (&*navigation as *const Navigation).cast_mut().cast(),
                )
            };
            let raw = NonNull::new(raw).context("Ghostty host creation failed")?;
            Ok(Self {
                _library: library,
                raw,
                set_frame,
                destroy,
                _navigation: navigation,
                frame: (0., 0., 0., 0.),
            })
        }

        fn library_path() -> Result<PathBuf> {
            let executable = std::env::current_exe().context("locate AgentInc executable")?;
            let bundled = executable
                .parent()
                .and_then(|path| path.parent())
                .context("locate AgentInc bundle")?
                .join("Frameworks/libAgentIncGhosttyBridge.dylib");
            if bundled.is_file() {
                return Ok(bundled);
            }
            Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("ghostty-bridge/.build/debug/libAgentIncGhosttyBridge.dylib"))
        }

        pub fn set_frame(&mut self, x: f64, y: f64, width: f64, height: f64, focus: bool) {
            self.frame = (x, y, width, height);
            // SAFETY: raw is owned by the live Swift host and calls stay on
            // GPUI's macOS main thread.
            unsafe { (self.set_frame)(self.raw.as_ptr(), x, y, width, height, true, focus) }
        }

        pub fn hide(&self) {
            let (x, y, width, height) = self.frame;
            // SAFETY: same lifetime and thread contract as set_frame.
            unsafe { (self.set_frame)(self.raw.as_ptr(), x, y, width, height, false, false) }
        }
    }

    impl Drop for TerminalHost {
        fn drop(&mut self) {
            // SAFETY: Swift releases its retained host before the library and
            // callback context stored by this struct are dropped.
            unsafe { (self.destroy)(self.raw.as_ptr()) }
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos::TerminalHost;

#[cfg(not(target_os = "macos"))]
pub struct TerminalHost;
