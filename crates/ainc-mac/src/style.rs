//! Values measured from the final Control CSS, including its overrides.
use gpui::{prelude::*, *};
use std::borrow::Cow;
pub const SHELL: u32 = 0x0c0c0c;
pub const SURFACE: u32 = 0x040404;
pub const BORDER: u32 = 0x272727;
pub const TEXT: u32 = 0xededed;
pub const MUTED: u32 = 0xa0a0a0;
pub const FOCUS: u32 = 0xb5cabe;
pub const SIDEBAR: f32 = 178.;

pub fn row() -> Div {
    div().flex().items_center()
}
pub fn column() -> Div {
    div().flex().flex_col()
}
pub fn panel() -> Div {
    column()
        .bg(rgb(SURFACE))
        .border_1()
        .border_color(rgb(BORDER))
        .rounded(px(14.))
}
pub fn icon(name: &'static str, size: f32) -> Svg {
    svg()
        .path(format!("{name}.svg"))
        .size(px(size))
        .text_color(rgb(MUTED))
        .flex_shrink_0()
}
pub fn nav_icon(name: &'static str, selected: bool) -> Svg {
    svg()
        .path(if selected {
            format!("selected/{name}.svg")
        } else {
            format!("{name}.svg")
        })
        .size(px(17.))
        .text_color(rgb(if selected { TEXT } else { MUTED }))
        .flex_shrink_0()
}
pub fn evee_logo(size: f32) -> impl IntoElement {
    img(ImageSource::Resource(Resource::Embedded("evee.png".into())))
        .size(px(size))
        .rounded_full()
        .flex_shrink_0()
}
pub fn shortcut_badge(label: impl Into<SharedString>) -> Div {
    row()
        .h(px(20.))
        .min_w(px(20.))
        .px(px(4.))
        .justify_center()
        .rounded(px(4.))
        .bg(rgb(0x141414))
        .text_size(px(10.))
        .text_color(rgb(MUTED))
        .child(label.into())
}
pub struct Assets;
impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        if let Some(name) = path.strip_prefix("selected/") {
            return self.load(name).map(|data| {
                data.map(|data| {
                    Cow::Owned(
                        String::from_utf8_lossy(&data)
                            .replace("stroke-width=\"1.5\"", "stroke-width=\"2\"")
                            .into_bytes(),
                    )
                })
            });
        }
        let data: &'static [u8] = match path {
            "agents.svg" => include_bytes!("../assets/agents.svg"),
            "arrowRight.svg" => include_bytes!("../assets/arrowRight.svg"),
            "arrowUpRight.svg" => include_bytes!("../assets/arrowUpRight.svg"),
            "bell.svg" => include_bytes!("../assets/bell.svg"),
            "calendar.svg" => include_bytes!("../assets/calendar.svg"),
            "chevronLeft.svg" => include_bytes!("../assets/chevronLeft.svg"),
            "chevronRight.svg" => include_bytes!("../assets/chevronRight.svg"),
            "close.svg" => include_bytes!("../assets/close.svg"),
            "evee.png" => include_bytes!("../assets/evee.png"),
            "grid.svg" => include_bytes!("../assets/grid.svg"),
            "home.svg" => include_bytes!("../assets/home.svg"),
            "panel.svg" => include_bytes!("../assets/panel.svg"),
            "photos.svg" => include_bytes!("../assets/photos.svg"),
            "plus.svg" => include_bytes!("../assets/plus.svg"),
            "search.svg" => include_bytes!("../assets/search.svg"),
            "settings.svg" => include_bytes!("../assets/settings.svg"),
            "spark.svg" => include_bytes!("../assets/spark.svg"),
            "sun.svg" => include_bytes!("../assets/sun.svg"),
            "tasks.svg" => include_bytes!("../assets/tasks.svg"),
            _ => return Ok(None),
        };
        Ok(Some(Cow::Borrowed(data)))
    }
    fn list(&self, _: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(vec![])
    }
}
// One continuous contour avoids vertical border tails at the inverse shoulders.
// Tab width is 142px; the 12px shoulders meet the panel border 2px above its base.
pub fn tab_contour(active: bool) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let p = |x: f32, y: f32| bounds.origin + point(px(x), px(y));
            let contour = |path: &mut PathBuilder| {
                path.move_to(p(0., 38.));
                path.cubic_bezier_to(p(12., 26.), p(8., 38.), p(12., 34.));
                path.line_to(p(12., 10.));
                path.cubic_bezier_to(p(22., 0.), p(12., 4.477), p(16.477, 0.));
                path.line_to(p(144., 0.));
                path.cubic_bezier_to(p(154., 10.), p(149.523, 0.), p(154., 4.477));
                path.line_to(p(154., 26.));
                path.cubic_bezier_to(p(166., 38.), p(154., 34.), p(158., 38.));
            };
            let mut fill = PathBuilder::fill();
            contour(&mut fill);
            fill.line_to(p(166., 40.));
            fill.line_to(p(0., 40.));
            fill.close();
            if let Ok(path) = fill.build() {
                window.paint_path(path, rgb(if active { SURFACE } else { 0x191919 }));
            }
            let mut line = PathBuilder::stroke(px(1.));
            contour(&mut line);
            if active && let Ok(path) = line.build() {
                window.paint_path(path, rgb(BORDER));
            }
        },
    )
    .absolute()
    .top_0()
    .left(px(-12.))
    .w(px(166.))
    .h(px(40.))
}

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
            (entry.0.elapsed().as_secs_f32() / 0.14).min(1.)
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
        rgba(0x25252500 | (alpha * 255.) as u32)
    }
    pub fn animate(&mut self, window: &mut Window) {
        self.0
            .retain(|_, entry| entry.2 > 0. || entry.0.elapsed().as_secs_f32() < 0.14);
        if !reduced_motion()
            && self
                .0
                .values()
                .any(|entry| entry.0.elapsed().as_secs_f32() < 0.14)
        {
            window.request_animation_frame();
        }
    }
}
