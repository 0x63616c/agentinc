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
        let workspace = self.workspace_state();
        let current = workspace
            .workspaces
            .iter()
            .find(|item| item.id == workspace.current_id);
        let workspace_name = current
            .map_or("World Wide Webb", |item| item.name.as_str())
            .to_owned();
        let workspace_icon = current
            .and_then(|item| item.icon.as_deref())
            .filter(|icon| !icon.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| workspace_name.chars().next().unwrap_or('W').to_string());
        let workspace_color = current
            .and_then(|item| item.color.as_deref())
            .and_then(|color| u32::from_str_radix(color.trim_start_matches('#'), 16).ok());
        let workspace_ink = workspace_color.map_or(TEXT, |color| {
            let brightness =
                ((color >> 16) & 0xff) * 3 + ((color >> 8) & 0xff) * 6 + (color & 0xff);
            if brightness > 1400 { SHELL } else { TEXT }
        });
        let switcher = self
            .button(
                "workspace-picker",
                "Switch workspace",
                Control::WorkspacePicker,
                cx,
            )
            .min_w_0()
            .h(px(96.))
            .px(px(14.))
            .mb(px(14.))
            .rounded(px(12.))
            .border_1()
            .border_color(rgb(BORDER))
            .bg(rgb(SURFACE_RAISED))
            .active(|s| s.bg(rgb(SELECTED)))
            .child(
                column()
                    .flex_1()
                    .min_w_0()
                    .gap(px(9.))
                    .child(
                        div()
                            .text_size(type_size(CAPTION_SIZE))
                            .text_color(rgb(TEXT_MUTED))
                            .child("WORKSPACE"),
                    )
                    .child(
                        row()
                            .gap(px(10.))
                            .child(
                                row()
                                    .size(px(32.))
                                    .flex_shrink_0()
                                    .justify_center()
                                    .rounded(px(9.))
                                    .border_1()
                                    .border_color(rgb(BORDER_OVERLAY))
                                    .bg(rgb(workspace_color.unwrap_or(HOVER)))
                                    .text_color(rgb(workspace_ink))
                                    .text_size(type_size(18.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(workspace_icon),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .debug_selector(|| "workspace-title".into())
                                    .text_size(type_size(LABEL_SIZE))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(workspace_name),
                            ),
                    ),
            );
        let mut nav = column().gap(px(2.));
        for (index, page) in PAGES.iter().filter(|page| page.in_sidebar).enumerate() {
            nav = nav.child(self.sidebar_item(page.route, Some(index + 1), cx));
        }
        nav = nav.child(self.sidebar_item(Route::Settings, None, cx).mt(px(12.)));
        column()
            .w_full()
            .h_full()
            .flex_shrink_0()
            .debug_selector(|| "sidebar-content".into())
            .px(px(12.))
            .pt(px(20.))
            .child(switcher)
            .child(
                self.button("shell.search", "Search · ⌘ K", Control::Search, cx)
                    .debug_selector(|| "shell.search".into())
                    .w_full()
                    .min_w_0()
                    .h(px(32.))
                    .mb(px(16.))
                    .pl(px(HEADER_SEARCH_LEFT_INSET))
                    .pr(px(HEADER_SEARCH_RIGHT_INSET))
                    .bg(rgb(SURFACE_SEARCH))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .text_color(rgb(MUTED))
                    .text_size(type_size(LABEL_SIZE))
                    .child(icon("search", 14.))
                    .child(div().flex_1().min_w_0().truncate().child("Go to…"))
                    .when(self.session.panes[0].width >= 210., |s| {
                        s.child(shortcut_badge("⌘ K").w(px(36.)))
                    }),
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
