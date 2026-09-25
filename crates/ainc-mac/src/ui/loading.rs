//! Native loading states, animated only while they are visible.
use super::{
    layout::{column, row},
    reduced_motion,
    tokens::*,
};
use gpui::{prelude::*, *};
use std::time::Instant;

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
            .text_color(rgb(MUTED))
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
                    .border_color(rgba((SELECTED_BORDER << 8) | (70. + 100. * breath) as u32))
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

/// A short handoff after the first shell frame; the main UI remains ready underneath.
pub fn launch_overlay(start: Instant, window: &mut Window) -> Option<Div> {
    let duration = if reduced_motion() { 0.12 } else { 0.32 };
    let elapsed = start.elapsed().as_secs_f32();
    if elapsed >= duration {
        return None;
    }
    let frame = LoadingFrame::new(start, window);
    let fade = (1. - (elapsed / duration).powi(3)).clamp(0., 1.);
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
                    .text_color(rgb(MUTED))
                    .child("AGENTINC"),
            ),
    )
}
