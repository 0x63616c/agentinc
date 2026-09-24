//! Motion behavior shared by controls.
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

/// Small, interruptible hover fades shared by the assistant and task controls.
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
    pub fn color(&self, id: &ElementId) -> Rgba {
        let alpha = self.0.get(id).map(Self::value).unwrap_or(0.);
        rgba((HOVER_ROW << 8) | (alpha * 255.) as u32)
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
