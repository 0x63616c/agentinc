use super::*;

impl Shell {
    pub(super) fn layout_body(
        &self,
        left: AnyElement,
        center: AnyElement,
        right: Option<AnyElement>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let left_side = pane::Side::Left;
        let left_width =
            self.pane_visible[left_side.index()].min(self.pane_limit(left_side, window));
        let left_saved = self.session.panes[left_side.index()].width;
        row()
            .relative()
            .flex_1()
            .min_h_0()
            .pt(px(48.))
            .pb(px(8.))
            .pr(px(8.))
            .child(self.pane(left_side, left, left_width))
            .pl(px(8. * (1. - left_width / left_saved)))
            .child(
                row()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .gap(px(PANEL_GAP))
                    .child(
                        column()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .gap(px(8.))
                            .child(center)
                            .child(status_bar()),
                    )
                    .when_some(right, |body, panel| body.child(panel)),
            )
            .when(
                self.session.panes[left_side.index()].open && left_width > 0.,
                |body| {
                    body.child(
                        self.resize_handle(left_side, cx)
                            .absolute()
                            .left(px(left_width - 5.))
                            .top(px(48.))
                            .bottom(px(8.)),
                    )
                },
            )
    }
}
