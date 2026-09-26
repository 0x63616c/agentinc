//! Native loading states, animated only while they are visible.
use super::{
    layout::{column, row},
    reduced_motion,
    tokens::*,
};
use gpui::{prelude::*, *};
use std::time::{Duration, Instant};

const LAUNCH_OVERLAY_MIN_DURATION: Duration = Duration::from_millis(500);
const LAUNCH_OVERLAY_FADE_DURATION: f32 = 0.32;

pub struct LoadingFrame {
    phase: f32,
    still: bool,
}

impl LoadingFrame {
    pub fn new(start: Instant, window: &mut Window) -> Self {
        let still = reduced_motion();
        let phase = if still {
            0.
        } else {
            window.request_animation_frame();
            start.elapsed().as_secs_f32() * 4.
        };
        Self { phase, still }
    }

    pub fn inline(&self, label: impl Into<SharedString>) -> Div {
        let mut dots = row().gap(px(4.)).items_center();
        for index in 0..3 {
            let wave = ((self.phase - index as f32 * 0.65).sin() + 1.) * 0.5;
            dots = dots.child(
                div()
                    .size(px(if self.still { 5. } else { 4. + 2. * wave }))
                    .rounded_full()
                    .bg(rgb(PRIMARY))
                    .opacity(if self.still { 0.7 } else { 0.25 + 0.7 * wave }),
            );
        }
        row()
            .gap(px(10.))
            .items_center()
            .text_size(type_size(LABEL_SIZE))
            .text_color(rgb(TEXT_SECONDARY))
            .child(dots)
            .child(label.into())
    }

    pub fn page(&self, label: impl Into<SharedString>) -> Div {
        column()
            .w_full()
            .min_h(px(240.))
            .items_center()
            .justify_center()
            .gap(px(18.))
            .child(self.mark(58.))
            .child(self.inline(label))
    }

    fn mark(&self, size: f32) -> Div {
        let breath = if self.still {
            0.
        } else {
            (self.phase.sin() + 1.) * 0.5
        };
        let frame_size = size + 42.;
        let center = frame_size / 2.;
        let orbit = size / 2. + 13.;
        let mut mark = div()
            .relative()
            .size(px(frame_size))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .size(px(size + 24.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .border_1()
                    .border_color(rgba((BORDER_STRONG << 8) | (70. + 100. * breath) as u32))
                    .bg(rgba((PRIMARY << 8) | (8. + 10. * breath) as u32))
                    .child(
                        img(ImageSource::Resource(Resource::Embedded("evee.png".into())))
                            .size(px(size))
                            .rounded_full()
                            .flex_shrink_0(),
                    ),
            );
        if !self.still {
            for index in 0..3 {
                let angle = self.phase * 0.55 + index as f32 * std::f32::consts::TAU / 3.;
                let dot = 3. + index as f32;
                mark = mark.child(
                    div()
                        .absolute()
                        .left(px(center + orbit * angle.cos() - dot / 2.))
                        .top(px(center + orbit * angle.sin() - dot / 2.))
                        .size(px(dot))
                        .rounded_full()
                        .bg(rgb(PRIMARY))
                        .opacity(0.35 + 0.2 * index as f32),
                );
            }
        }
        mark
    }
}

/// A placeholder bar that breathes while content loads.
pub fn skeleton(id: impl Into<ElementId>, width: Option<f32>, height: f32) -> impl IntoElement {
    div()
        .h(px(height))
        .when_some(width, |s, width| s.w(px(width)))
        .when(width.is_none(), |s| s.w_full())
        .rounded(px(RADIUS_SM))
        .bg(rgb(SKELETON))
        .with_animation(
            id,
            Animation::new(Duration::from_millis(SKELETON_MS))
                .repeat()
                .with_easing(pulsating_between(0.45, 1.)),
            |bar, t| bar.opacity(t),
        )
}

/// A stack of skeleton rows standing in for a list or table.
pub fn skeleton_rows(id: &'static str, count: usize) -> Div {
    column()
        .debug_selector(move || id.into())
        .w_full()
        .gap(px(SPACE_3))
        .children((0..count).map(|index| {
            row()
                .w_full()
                .gap(px(SPACE_3))
                .child(skeleton(
                    ElementId::NamedInteger(format!("{id}.mark").into(), index as u64),
                    Some(24.),
                    24.,
                ))
                .child(
                    column()
                        .flex_1()
                        .gap(px(SPACE_2))
                        .child(skeleton(
                            ElementId::NamedInteger(format!("{id}.title").into(), index as u64),
                            Some(180. + (index % 3) as f32 * 60.),
                            12.,
                        ))
                        .child(skeleton(
                            ElementId::NamedInteger(format!("{id}.body").into(), index as u64),
                            Some(120. + (index % 2) as f32 * 80.),
                            10.,
                        )),
                )
        }))
}

/// A short handoff after the first shell frame; the main UI remains ready underneath.
pub fn launch_overlay(start: Instant, window: &mut Window) -> Option<Div> {
    let elapsed = start.elapsed();
    if elapsed
        >= LAUNCH_OVERLAY_MIN_DURATION + Duration::from_secs_f32(LAUNCH_OVERLAY_FADE_DURATION)
    {
        return None;
    }
    let frame = LoadingFrame::new(start, window);
    let fade_elapsed = elapsed
        .saturating_sub(LAUNCH_OVERLAY_MIN_DURATION)
        .as_secs_f32();
    let fade_duration = if reduced_motion() {
        0.12
    } else {
        LAUNCH_OVERLAY_FADE_DURATION
    };
    let fade = (1. - (fade_elapsed / fade_duration).powi(3)).clamp(0., 1.);
    Some(
        column()
            .absolute()
            .inset_0()
            .occlude()
            .items_center()
            .justify_center()
            .gap(px(20.))
            .bg(rgb(SHELL))
            .opacity(fade)
            .child(frame.mark(84.))
            .child(
                div()
                    .text_size(type_size(CAPTION_SIZE))
                    .text_color(rgb(TEXT_SECONDARY))
                    .child("AGENTINC"),
            ),
    )
}

#[cfg(test)]
mod tests {
    use super::LAUNCH_OVERLAY_MIN_DURATION;

    #[test]
    fn launch_overlay_minimum_duration_is_half_a_second() {
        assert_eq!(
            LAUNCH_OVERLAY_MIN_DURATION,
            std::time::Duration::from_millis(500)
        );
    }
}
