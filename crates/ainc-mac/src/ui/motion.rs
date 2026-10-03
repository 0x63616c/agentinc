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

/// Linear blend between two already-blended colors.
pub fn mix(from: Rgba, to: Rgba, t: f32) -> Rgba {
    let t = t.clamp(0., 1.);
    Rgba {
        r: from.r + (to.r - from.r) * t,
        g: from.g + (to.g - from.g) * t,
        b: from.b + (to.b - from.b) * t,
        a: 1.,
    }
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

/// What every component builder takes: the window, the view context and,
/// behind them, the one hover store. Hosts build one per render and pass it
/// down; nothing about hover reaches them.
pub struct Ui<'a, 'b, V: 'static> {
    pub window: &'a mut Window,
    pub cx: &'a mut Context<'b, V>,
}
impl<'a, 'b, V: 'static> Ui<'a, 'b, V> {
    pub fn new(window: &'a mut Window, cx: &'a mut Context<'b, V>) -> Self {
        HoverFade::animate(window);
        Self { window, cx }
    }
    /// The hover amount for `id` plus the listener that drives it: every
    /// hover-faded control is built from this one pair.
    pub fn hover(
        &mut self,
        id: &ElementId,
        enabled: bool,
    ) -> (f32, impl Fn(&bool, &mut Window, &mut App) + 'static) {
        HoverFade::track(id, enabled, self.cx)
    }
}

/// Small, interruptible hover fades shared by every control with a hover look.
/// One store serves the single app window and every page entity rendered in it.
#[derive(Default)]
pub struct HoverFade(std::collections::HashMap<ElementId, (std::time::Instant, f32, f32)>);
thread_local! {
    static HOVER: std::cell::RefCell<HoverFade> = std::cell::RefCell::new(HoverFade::default());
}
impl HoverFade {
    fn value(entry: &(std::time::Instant, f32, f32)) -> f32 {
        let t = if reduced_motion() {
            1.
        } else {
            (entry.0.elapsed().as_secs_f32() / (HOVER_MS as f32 / 1000.)).min(1.)
        };
        entry.1 + (entry.2 - entry.1) * (1. - (1. - t).powi(3))
    }
    fn set(&mut self, id: ElementId, hovered: bool) {
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
    fn progress(&self, id: &ElementId) -> f32 {
        self.0.get(id).map(Self::value).unwrap_or(0.)
    }
    /// The one place hover is computed: the amount for `id` and the listener
    /// that drives it, from the window's shared store.
    fn track<V: 'static>(
        id: &ElementId,
        enabled: bool,
        cx: &mut Context<V>,
    ) -> (f32, impl Fn(&bool, &mut Window, &mut App) + 'static) {
        let progress = if enabled {
            HOVER.with(|fade| fade.borrow().progress(id))
        } else {
            0.
        };
        let hover_id = id.clone();
        let listener = cx.listener(move |_: &mut V, over: &bool, _, cx| {
            if enabled {
                HOVER.with(|fade| fade.borrow_mut().set(hover_id.clone(), *over));
                cx.notify();
            }
        });
        (progress, listener)
    }
    /// Drop finished fades and keep the frame clock running while any is live.
    fn animate(window: &mut Window) {
        HOVER.with(|fade| fade.borrow_mut().step(window));
    }
    fn step(&mut self, window: &mut Window) {
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
    fn hover_fade_tracks_each_control_separately() {
        use super::HoverFade;
        use gpui::ElementId;
        let mut fade = HoverFade::default();
        let a = ElementId::Name("a".into());
        let b = ElementId::Name("b".into());
        assert_eq!(fade.progress(&a), 0.);
        fade.set(a.clone(), true);
        assert!(fade.progress(&a) >= 0.);
        assert_eq!(fade.progress(&b), 0.);
        fade.set(a.clone(), false);
        assert!(fade.progress(&a) <= 1.);
    }

    #[test]
    fn blend_interpolates_each_channel() {
        assert_eq!(blend(SHELL, PRIMARY, 0.), rgb(SHELL));
        assert_eq!(blend(SHELL, PRIMARY, 1.), rgb(PRIMARY));
        let mid = blend(SHELL, PRIMARY, 0.5);
        assert!((mid.r - 0.5).abs() < 0.01 && (mid.g - 0.5).abs() < 0.01);
        assert_eq!(blend(SURFACE, SURFACE_RAISED, 2.), rgb(SURFACE_RAISED));
    }
}
