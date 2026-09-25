//! GPUI's layout slot for the real Ghostty AppKit surface.
#[cfg(target_os = "macos")]
pub(crate) fn terminal_colors() -> String {
    use super::tokens::*;

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

#[cfg(target_os = "macos")]
pub fn terminal_surface(
    host: std::rc::Rc<std::cell::RefCell<crate::terminal::TerminalHost>>,
    visible: bool,
    focus: bool,
) -> impl gpui::IntoElement {
    use gpui::{Styled as _, canvas};
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
