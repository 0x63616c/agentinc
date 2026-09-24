use super::*;

impl Shell {
    pub(super) fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut nav = column().gap(px(2.));
        for page in PAGES.iter().filter(|page| page.in_sidebar) {
            let route = page.route;
            let index = page.shortcut.expect("sidebar route has shortcut");
            nav = nav.child(
                self.button(
                    ("nav", index as usize),
                    route.label(),
                    Control::Navigate(route),
                    cx,
                )
                .accessibility_id(format!(
                    "nav.{}",
                    route.label().to_lowercase().replace(' ', "-")
                ))
                .min_h(type_size(32.))
                .px(px(10.))
                .gap(px(12.))
                .debug_selector(move || format!("sidebar-nav-{index}"))
                .text_size(type_size(LABEL_SIZE))
                .text_color(rgb(MUTED))
                .when(self.session.current() == route, |s| {
                    s.bg(rgb(HOVER))
                        .text_color(rgb(TEXT))
                        .font_weight(FontWeight::MEDIUM)
                })
                .child(nav_icon(
                    if route == Route::Assistant {
                        "evee-outline"
                    } else {
                        route.icon()
                    },
                    self.session.current() == route,
                ))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .debug_selector(move || format!("sidebar-label-{index}"))
                        .child(route.label()),
                )
                .when(self.command_held, |s| {
                    s.child(
                        shortcut_badge(format!("⌘{index}"))
                            .flex_shrink_0()
                            .debug_selector(move || format!("sidebar-badge-{index}")),
                    )
                }),
            );
        }
        column()
            .w_full()
            .h_full()
            .flex_shrink_0()
            .debug_selector(|| "sidebar-content".into())
            .px(px(12.))
            .pt(px(20.))
            .child(
                row()
                    .min_w_0()
                    .pl(px(6.5))
                    .pr(px(6.))
                    .gap(px(8.5))
                    .mb(px(22.))
                    .child(
                        row()
                            .size(px(24.))
                            .flex_shrink_0()
                            .justify_center()
                            .rounded(px(7.))
                            .border_1()
                            .border_color(rgb(BORDER_OVERLAY))
                            .bg(rgb(HOVER))
                            .child({
                                #[cfg(feature = "automation")]
                                if let Ok(name) = std::env::var("AGENTINC_CAPTURE_WORKSPACE") {
                                    name.chars().next().unwrap_or('W').to_string()
                                } else {
                                    "W".to_owned()
                                }
                                #[cfg(not(feature = "automation"))]
                                "W".to_owned()
                            }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .debug_selector(|| "workspace-title".into())
                            .text_size(type_size(LABEL_SIZE))
                            .child({
                                #[cfg(feature = "automation")]
                                {
                                    std::env::var("AGENTINC_CAPTURE_WORKSPACE")
                                        .unwrap_or_else(|_| "World Wide Webb".to_owned())
                                }
                                #[cfg(not(feature = "automation"))]
                                "World Wide Webb".to_owned()
                            }),
                    ),
            )
            .child(nav)
            .child(div().flex_1())
            .child(
                self.button(
                    "profile",
                    "Settings",
                    Control::Navigate(Route::Settings),
                    cx,
                )
                .h(px(98.))
                .flex_shrink_0()
                .w_full()
                .flex_col()
                .items_start()
                .justify_center()
                .px(px(10.))
                .gap(px(4.))
                .debug_selector(|| "sidebar-profile".into())
                .child(match &self.profile.photo {
                    Some(photo) => img(photo.clone())
                        .size(px(24.))
                        .rounded_full()
                        .into_any_element(),
                    None => row()
                        .size(px(24.))
                        .rounded_full()
                        .bg(rgb(BORDER))
                        .justify_center()
                        .child(self.profile.name.chars().next().unwrap_or('C').to_string())
                        .into_any_element(),
                })
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(type_size(LABEL_SIZE))
                        .child(self.profile.name.clone()),
                )
                .child(
                    div()
                        .id("sidebar.version")
                        .flex_shrink_0()
                        .debug_selector(|| "sidebar-version".into())
                        .accessibility_id("sidebar.version")
                        .role(accesskit::Role::Label)
                        .aria_label(ainc_release::identity::version())
                        .text_size(type_size(CAPTION_SIZE))
                        .text_color(rgb(TEXT_MUTED))
                        .child(ainc_release::identity::version())
                        .when(!ainc_release::identity::PRODUCTION, |version| {
                            version.tooltip(|_, cx| cx.new(|_| crate::about::CommitTooltip).into())
                        }),
                ),
            )
    }
}
