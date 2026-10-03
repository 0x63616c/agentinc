//! Terminal: the Ghostty surface hosted as an AppKit child of this window. The
//! page owns the host's lifetime, hides it under overlays and when another
//! page is shown, and relays the shortcuts Ghostty swallows back to the shell.
#[cfg(target_os = "macos")]
#[path = "terminal/host.rs"]
mod host;

use crate::{
    daemon::Daemon,
    overlay::Overlay,
    page::Page,
    routes::{Destination, Route},
    ui::*,
};
use gpui::{prelude::*, *};
use std::{cell::RefCell, rc::Rc, sync::Arc};

/// A key chord Ghostty received that belongs to the shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shortcut {
    GoTo,
    Settings,
    Back,
    Forward,
    Page(Route),
}

/// How the terminal reaches the shell without knowing it.
pub type ShortcutHandler = Rc<dyn Fn(Shortcut, &mut Window, &mut App)>;
pub type CommandHeldHandler = Rc<dyn Fn(bool, &mut App)>;
#[derive(Clone)]
pub struct TerminalCallbacks {
    pub on_shortcut: ShortcutHandler,
    /// Ghostty saw Command pressed or released; the sidebar shows its hints.
    pub on_command_held: CommandHeldHandler,
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub struct TerminalPage {
    daemon: Arc<Daemon>,
    overlays: Rc<RefCell<OverlayHost<Overlay>>>,
    callbacks: TerminalCallbacks,
    #[cfg(target_os = "macos")]
    host: Option<Rc<RefCell<host::TerminalHost>>>,
    error: Option<String>,
    pending_focus: bool,
}
impl EventEmitter<Destination> for TerminalPage {}

impl TerminalPage {
    pub fn new(
        daemon: Arc<Daemon>,
        overlays: Rc<RefCell<OverlayHost<Overlay>>>,
        callbacks: TerminalCallbacks,
        _: &mut Context<Self>,
    ) -> Self {
        Self {
            daemon,
            overlays,
            callbacks,
            #[cfg(target_os = "macos")]
            host: None,
            error: None,
            pending_focus: false,
        }
    }
    #[cfg(all(test, feature = "rendered-tests"))]
    #[allow(dead_code)]
    pub(crate) fn fixture_unavailable(&mut self, cx: &mut Context<Self>) {
        #[cfg(target_os = "macos")]
        {
            self.host = None;
        }
        self.error = Some(copy::unavailable(
            "Terminal",
            "Ghostty could not start: the bundled runtime is missing",
        ));
        cx.notify();
    }
    #[cfg(target_os = "macos")]
    fn surface(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.host.is_none() && self.error.is_none() {
            let workspace_id = self.daemon.workspaces().current_id;
            match host::TerminalHost::new(
                window,
                cx.to_async(),
                self.callbacks.clone(),
                &workspace_id,
            ) {
                Ok(host) => {
                    self.host = Some(Rc::new(RefCell::new(host)));
                    self.pending_focus = true;
                }
                Err(error) => {
                    self.error = Some(copy::unavailable(
                        "Terminal",
                        &format!("Ghostty could not start: {error:#}"),
                    ))
                }
            }
        }
        let host = self.host.clone()?;
        // AppKit child views paint above GPUI's Metal layer. Hide Ghostty
        // before GPUI paints any app popover, menu, or dialog over this page.
        let visible = self.overlays.borrow().active().is_none();
        if !visible {
            host.borrow_mut().hide();
        }
        let focus = std::mem::take(&mut self.pending_focus);
        Some(
            PageFrame::canvas()
                .child(
                    column().size_full().min_h_0().child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .w_full()
                            .child(terminal_surface(host, visible, focus)),
                    ),
                )
                .build()
                .into_any_element(),
        )
    }
}

impl Page for TerminalPage {
    const ROUTE: Route = Route::Terminal;
    fn shown(&mut self, shown: bool, _: &mut Context<Self>) {
        if shown {
            self.pending_focus = true;
        }
        #[cfg(target_os = "macos")]
        if !shown && let Some(host) = &self.host {
            host.borrow_mut().hide();
        }
    }
}

impl Render for TerminalPage {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        #[cfg(target_os = "macos")]
        if let Some(surface) = self.surface(_window, _cx) {
            return surface;
        }
        #[cfg(target_os = "macos")]
        let message = self
            .error
            .clone()
            .unwrap_or_else(|| "Ghostty could not start.".into());
        #[cfg(not(target_os = "macos"))]
        let message = "Terminal requires macOS and the Ghostty runtime.";
        PageFrame::canvas()
            .child(
                column().size_full().min_h_0().p(px(PAGE_X)).child(
                    EmptyState::new("terminal", "Terminal unavailable")
                        .description(message)
                        .selector("terminal.unavailable")
                        .build(),
                ),
            )
            .build()
            .into_any_element()
    }
}

#[cfg(target_os = "macos")]
fn terminal_colors() -> String {
    // Ghostty reads the user's config first. These final color values use the
    // same roles as the surrounding AgentInc surface and controls.
    let selection = TEXT_SELECTION >> 8;
    let mut config = format!(
        "background = #{SURFACE:06x}\nforeground = #{TEXT:06x}\ncursor-color = #{PRIMARY:06x}\ncursor-text = #{TEXT_ON_PRIMARY:06x}\nselection-background = #{selection:06x}\nselection-foreground = #{TEXT:06x}\nbackground-opacity = 1\n"
    );
    for (index, color) in TERMINAL_ANSI.into_iter().enumerate() {
        config.push_str(&format!("palette = {index}=#{color:06x}\n"));
    }
    config
}

/// GPUI's layout slot for the real Ghostty AppKit surface.
#[cfg(target_os = "macos")]
fn terminal_surface(
    host: Rc<RefCell<host::TerminalHost>>,
    visible: bool,
    focus: bool,
) -> impl IntoElement {
    canvas(
        move |bounds, _, _| {
            if visible {
                host.borrow_mut().set_frame(
                    f64::from(f32::from(bounds.origin.x)),
                    f64::from(f32::from(bounds.origin.y)),
                    f64::from(f32::from(bounds.size.width)),
                    f64::from(f32::from(bounds.size.height)),
                    focus,
                );
            } else {
                host.borrow_mut().hide();
            }
        },
        |_, _, _, _| {},
    )
    .size_full()
}
