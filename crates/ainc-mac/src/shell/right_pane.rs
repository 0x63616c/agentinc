use super::*;

impl Shell {
    pub(super) fn evee(&self, cx: &mut Context<Self>) -> impl IntoElement {
        panel()
            .debug_selector(|| "right-pane".into())
            .w_full()
            .h_full()
            .flex_shrink_0()
            .px(px(RIGHT_PANE_CONTENT_INSET))
            .pb(px(RIGHT_PANE_CONTENT_INSET))
            .child(
                row()
                    .h(px(50.))
                    .flex_shrink_0()
                    .gap(px(8.))
                    .text_size(type_size(LABEL_SIZE))
                    .ml(px(-RIGHT_PANE_CONTENT_INSET))
                    .mr(px(-RIGHT_PANE_CONTENT_INSET))
                    .pl(px(EVEE_HEADER_LEFT_INSET))
                    .pr(px(EVEE_HEADER_RIGHT_INSET))
                    .border_b_1()
                    .border_color(rgb(BORDER_SUBTLE))
                    .mb(px(RIGHT_PANE_CONTENT_INSET))
                    .child(evee_logo(29.))
                    .child("Evee")
                    .child(div().flex_1())
                    .child(self.icon_button(
                        "close-evee",
                        "Close Evee panel",
                        "close",
                        Control::Evee,
                        cx,
                    )),
            )
            .child(
                div()
                    .debug_selector(|| "right-content".into())
                    .flex_1()
                    .min_h_0()
                    .child(self.assistant.clone()),
            )
    }
}
