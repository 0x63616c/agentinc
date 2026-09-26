//! Motion behavior shared by controls: reduced motion, hover fades and blends.
use super::tokens::*;
use gpui::*;

/// Read once per process; changing the macOS preference takes effect on relaunch.
pub fn reduced_motion() -> bool {
    static REDUCED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *REDUCED.get_or_init(|| {
        std::process::Command::new("/usr/bin/defaults")
            .args(["read", "com.apple.universalaccess", "reduceMotion"])
            .output()
            .is_ok_and(|output| {
                output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "1"
            })
    })
}

// Focus rings follow the web's focus-visible rule: they appear after keyboard
// navigation and disappear at the next pointer press. The window shares one flag.
static FOCUS_VISIBLE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Records whether the last focus change came from the keyboard.
pub fn set_focus_visible(visible: bool) {
    FOCUS_VISIBLE.store(visible, std::sync::atomic::Ordering::Relaxed);
}

/// Whether controls should draw their focus ring right now.
pub fn focus_visible() -> bool {
    FOCUS_VISIBLE.load(std::sync::atomic::Ordering::Relaxed)
}

/// Linear blend between two opaque colors, used to fade hover surfaces.
pub fn blend(from: u32, to: u32, t: f32) -> Rgba {
    let t = t.clamp(0., 1.);
    let channel = |shift: u32| {
        let a = ((from >> shift) & 0xff) as f32;
        let b = ((to >> shift) & 0xff) as f32;
        (a + (b - a) * t).round() as u32
    };
    rgb((channel(16) << 16) | (channel(8) << 8) | channel(0))
}

/// A view that owns hover fades for the buttons it renders.
pub trait HoverHost: 'static {
    fn hover_fade(&mut self) -> &mut HoverFade;
}

/// Small, interruptible hover fades shared by every control with a hover look.
#[derive(Default)]
pub struct HoverFade(std::collections::HashMap<ElementId, (std::time::Instant, f32, f32)>);
impl HoverFade {
    fn value(entry: &(std::time::Instant, f32, f32)) -> f32 {
        let t = if reduced_motion() {
            1.
        } else {
            (entry.0.elapsed().as_secs_f32() / (HOVER_MS as f32 / 1000.)).min(1.)
        };
        entry.1 + (entry.2 - entry.1) * (1. - (1. - t).powi(3))
    }
    pub fn set(&mut self, id: ElementId, hovered: bool) {
        let from = self.0.get(&id).map(Self::value).unwrap_or(0.);
        self.0.insert(
            id,
            (
                std::time::Instant::now(),
                from,
                if hovered { 1. } else { 0. },
            ),
        );
    }
    /// The current hover amount for a control, from 0 (rest) to 1 (hovered).
    pub fn progress(&self, id: &ElementId) -> f32 {
        self.0.get(id).map(Self::value).unwrap_or(0.)
    }
    pub fn animate(&mut self, window: &mut Window) {
        self.0.retain(|_, entry| {
            entry.2 > 0. || entry.0.elapsed().as_secs_f32() < HOVER_MS as f32 / 1000.
        });
        if !reduced_motion()
            && self
                .0
                .values()
                .any(|entry| entry.0.elapsed().as_secs_f32() < HOVER_MS as f32 / 1000.)
        {
            window.request_animation_frame();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PRIMARY, SHELL, SURFACE, SURFACE_RAISED, blend};
    use gpui::rgb;

    #[test]
    fn blend_interpolates_each_channel() {
        assert_eq!(blend(SHELL, PRIMARY, 0.), rgb(SHELL));
        assert_eq!(blend(SHELL, PRIMARY, 1.), rgb(PRIMARY));
        let mid = blend(SHELL, PRIMARY, 0.5);
        assert!((mid.r - 0.5).abs() < 0.01 && (mid.g - 0.5).abs() < 0.01);
        assert_eq!(blend(SURFACE, SURFACE_RAISED, 2.), rgb(SURFACE_RAISED));
    }
}
