use super::*;

impl Shell {
    pub(super) fn evee(&self, cx: &mut Context<Self>) -> impl IntoElement {
        panel()
            .w_full()
            .h_full()
            .flex_shrink_0()
            .px(px(20.))
            .pb(px(20.))
            .child(
                row()
                    .h(px(50.))
                    .flex_shrink_0()
                    .gap(px(8.))
                    .text_size(type_size(LABEL_SIZE))
                    .ml(px(-20.))
                    .mr(px(-20.))
                    .pl(px(10.))
                    .pr(px(9.))
                    .border_b_1()
                    .border_color(rgb(0x1a1a1a))
                    .mb(px(20.))
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
            .child(div().flex_1().min_h_0().child(self.assistant.clone()))
    }
}
