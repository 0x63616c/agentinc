//! Transient notices stacked above the status bar.
use super::{badge::Tone, button::*, display::icon, layout::*, motion::*, tokens::*};
use gpui::{prelude::*, *};
use std::time::Duration;

pub struct Toast {
    pub id: u64,
    pub title: SharedString,
    pub body: Option<SharedString>,
    pub tone: Tone,
}

#[derive(Default)]
pub struct Toasts {
    items: Vec<Toast>,
    next: u64,
}

impl Toasts {
    pub const LIFETIME: Duration = Duration::from_secs(6);

    /// Adds a toast and returns its id. Toasts stay until dismissed; a host
    /// that wants a transient notice schedules the dismissal itself.
    pub fn push(
        &mut self,
        title: impl Into<SharedString>,
        body: Option<SharedString>,
        tone: Tone,
    ) -> u64 {
        self.next += 1;
        let id = self.next;
        self.items.push(Toast {
            id,
            title: title.into(),
            body,
            tone,
        });
        id
    }
    pub fn dismiss(&mut self, id: u64) -> bool {
        let before = self.items.len();
        self.items.retain(|toast| toast.id != id);
        before != self.items.len()
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Renders the stack anchored `right` and `bottom` pixels from the host's
    /// bottom-right corner, so the host can put it on its own content rail.
    pub fn render<V: HoverHost>(
        &self,
        hover: &HoverFade,
        right: f32,
        bottom: f32,
        on_dismiss: impl Fn(&mut V, u64, &mut Window, &mut Context<V>) + Clone + 'static,
        cx: &mut Context<V>,
    ) -> Div {
        column()
            .absolute()
            .right(px(right))
            .bottom(px(bottom))
            .gap(px(SPACE_2))
            .items_end()
            .debug_selector(|| "toasts".into())
            .children(self.items.iter().map(|toast| {
                let id = toast.id;
                let on_dismiss = on_dismiss.clone();
                let (foreground, _) = toast.tone.colors();
                let glyph = match toast.tone {
                    Tone::Success => "check",
                    Tone::Warning => "warning",
                    Tone::Danger => "warning",
                    _ => "info",
                };
                row()
                    .id(("toast", id))
                    .w(px(TOAST_WIDTH))
                    .items_start()
                    .gap(px(SPACE_3))
                    .p(px(SPACE_3))
                    .rounded(px(RADIUS_LG))
                    .bg(rgb(SURFACE_OVERLAY))
                    .border_1()
                    .border_color(rgb(BORDER_STRONG))
                    .shadow(shadow_toast())
                    .child(
                        div()
                            .mt(px(2.))
                            .child(icon(glyph, ICON_SIZE).text_color(rgb(foreground))),
                    )
                    .child(
                        column()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(type_size(LABEL_SIZE))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(toast.title.clone()),
                            )
                            .when_some(toast.body.clone(), |s, body| {
                                s.child(
                                    div()
                                        .text_size(type_size(CAPTION_SIZE))
                                        .text_color(rgb(TEXT_SECONDARY))
                                        .child(body),
                                )
                            }),
                    )
                    .child(
                        Button::new(("toast.close", id), "Dismiss")
                            .icon("close")
                            .icon_only()
                            .ghost()
                            .small()
                            .build(
                                hover,
                                move |view, window, cx| on_dismiss(view, id, window, cx),
                                cx,
                            )
                            .debug_selector(move || format!("toast.close.{id}")),
                    )
                    .with_spring(
                        ("toast.enter", id),
                        SpringAnimation::new(SPRING_GENTLE).to(1f32).from(0f32),
                        |toast, t: f32| {
                            let t = t.clamp(0., 1.);
                            toast.opacity(t).mt(px(SPACE_2 * (1. - t)))
                        },
                    )
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::{Toasts, Tone};

    #[test]
    fn ids_are_unique_and_dismissal_is_exact() {
        let mut toasts = Toasts::default();
        let first = toasts.push("Saved", None, Tone::Info);
        let second = toasts.push("Failed", Some("Retry later".into()), Tone::Danger);
        assert_ne!(first, second);
        assert!(!toasts.is_empty());
        assert!(toasts.dismiss(first));
        assert!(!toasts.dismiss(first));
        assert!(toasts.dismiss(second));
        assert!(toasts.is_empty());
    }
}
