use super::*;

impl Shell {
    fn sidebar_item(
        &self,
        route: Route,
        index: Option<usize>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let selected = self.session.current() == route;
        let tint = if selected { TEXT } else { MUTED };
        let hover_group = format!("sidebar-item-{}", index.unwrap_or(0));
        self.button(
            ("nav", index.unwrap_or(0)),
            route.label(),
            Control::Navigate(route),
            cx,
        )
        .group(hover_group.clone())
        .accessibility_id(format!(
            "nav.{}",
            route.label().to_lowercase().replace(' ', "-")
        ))
        .min_h(type_size(32.))
        .when(route == Route::Agents, |s| s.mt(px(12.)))
        .px(px(10.))
        .gap(px(12.))
        .debug_selector(move || match index {
            Some(index) => format!("sidebar-nav-{index}"),
            None => "sidebar-settings".into(),
        })
        .text_size(type_size(LABEL_SIZE))
        .text_color(rgb(tint))
        .when(selected, |s| {
            s.bg(rgb(HOVER)).font_weight(FontWeight::MEDIUM)
        })
        .child(nav_icon(
            if route == Route::Assistant {
                "evee-outline"
            } else {
                route.icon()
            },
            selected,
            tint,
            hover_group,
        ))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .when_some(index, |s, index| {
                    s.debug_selector(move || format!("sidebar-label-{index}"))
                })
                .child(route.label()),
        )
        .when_some(index.filter(|_| self.command_held), |s, index| {
            s.child(
                shortcut_badge(format!("⌘{}", index % 10))
                    .flex_shrink_0()
                    .debug_selector(move || format!("sidebar-badge-{index}")),
            )
        })
        .when(index.is_none() && self.command_held, |s| {
            s.child(shortcut_badge("⌘,").flex_shrink_0())
        })
    }

    pub(super) fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut nav = column().gap(px(2.));
        for (index, page) in PAGES.iter().filter(|page| page.in_sidebar).enumerate() {
            nav = nav.child(self.sidebar_item(page.route, Some(index + 1), cx));
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
                    .pl(px(SIDEBAR_IDENTITY_LEFT_INSET))
                    .pr(px(SIDEBAR_IDENTITY_RIGHT_INSET))
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
            .child(self.sidebar_item(Route::Settings, None, cx).mb(px(8.)))
            .child(
                self.button(
                    "profile",
                    "Settings",
                    Control::Navigate(Route::Settings),
                    cx,
                )
                .h(px(44.))
                .flex_shrink_0()
                .w_full()
                .items_center()
                .px(px(10.))
                .gap(px(8.))
                .debug_selector(|| "sidebar-profile".into())
                .child(match &self.profile.photo {
                    Some(photo) => img(photo.clone())
                        .size(px(24.))
                        .flex_shrink_0()
                        .rounded_full()
                        .into_any_element(),
                    None => row()
                        .size(px(24.))
                        .flex_shrink_0()
                        .rounded_full()
                        .bg(rgb(BORDER))
                        .justify_center()
                        .child(self.profile.name.chars().next().unwrap_or('C').to_string())
                        .into_any_element(),
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .debug_selector(|| "sidebar-profile-name".into())
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
                        .child(ainc_release::identity::version()),
                ),
            )
    }
}
