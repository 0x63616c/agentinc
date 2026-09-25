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

    type Shortcut = unsafe extern "C" fn(*mut c_void, i32);
    type Create = unsafe extern "C" fn(
        *mut c_void,
        *const std::ffi::c_char,
        *const std::ffi::c_char,
        *const std::ffi::c_char,
        *const std::ffi::c_char,
        u32,
        Option<Shortcut>,
        *mut c_void,
    ) -> *mut c_void;
    type SetFrame = unsafe extern "C" fn(*mut c_void, f64, f64, f64, f64, bool, bool);
    type Destroy = unsafe extern "C" fn(*mut c_void);

    struct Navigation {
        app: AsyncApp,
        window: AnyWindowHandle,
        shell: WeakEntity<Shell>,
    }

    unsafe extern "C" fn shortcut(context: *mut c_void, number: i32) {
        // SAFETY: the bridge retains this pointer only while TerminalHost and
        // its Navigation box are alive, and invokes it synchronously on the UI thread.
        let Some(navigation) = (unsafe { (context as *const Navigation).as_ref() }) else {
            return;
        };
        let control = match number {
            -1 => Control::Search,
            -2 => Control::Navigate(crate::model::Route::Settings),
            0..=9 => {
                let Some(route) = crate::model::Route::from_shortcut(number as u8) else {
                    return;
                };
                Control::Navigate(route)
            }
            _ => return,
        };
        navigation.app.update(|cx| {
            let _ = navigation.window.update(cx, |_, window, cx| {
                let _ = navigation.shell.update(cx, |shell, cx| {
                    shell.dispatch(control, window, cx);
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
        visible: bool,
    }

    impl TerminalHost {
        pub fn new(
            window: &Window,
            app: AsyncApp,
            shell: WeakEntity<Shell>,
            workspace_id: &str,
        ) -> Result<Self> {
            let handle = HasWindowHandle::window_handle(window)
                .map_err(|error| anyhow!("GPUI native window: {error}"))?;
            let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
                return Err(anyhow!("Ghostty requires an AppKit window"));
            };
            let home = std::env::var_os("HOME").context("HOME is missing")?;
            let home = CString::new(home.to_string_lossy().as_bytes()).context("invalid HOME")?;
            let helper = std::env::current_exe()?.with_file_name("aincd");
            let helper = CString::new(helper.to_string_lossy().as_bytes())?;
            let layout_name = if workspace_id == "local" {
                "terminal-layout.json".to_owned()
            } else {
                format!("terminal-layout-{workspace_id}.json")
            };
            let layout = crate::storage::discovery_path()?.with_file_name(layout_name);
            let layout = CString::new(layout.to_string_lossy().as_bytes())?;
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
                    helper.as_ptr(),
                    layout.as_ptr(),
                    colors.as_ptr(),
                    crate::ui::BORDER,
                    Some(shortcut),
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
                visible: false,
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
            let focus = focus || !self.visible;
            self.visible = true;
            // SAFETY: raw is owned by the live Swift host and calls stay on
            // GPUI's macOS main thread.
            unsafe { (self.set_frame)(self.raw.as_ptr(), x, y, width, height, true, focus) }
        }

        pub fn hide(&mut self) {
            self.visible = false;
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

pub use macos::TerminalHost;
