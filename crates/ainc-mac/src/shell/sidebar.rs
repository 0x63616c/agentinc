use super::*;

/// The `@handle` shown under a display name until local accounts land.
pub(crate) fn handle_for(name: &str) -> String {
    let handle: String = name
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    format!("@{}", if handle.is_empty() { "you" } else { &handle })
}

impl Shell {
    fn sidebar_item(
        &self,
        route: Route,
        index: Option<usize>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let selected = self.session.current() == route;
        let tint = if selected { TEXT } else { TEXT_SECONDARY };
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
        .h(px(CONTROL_HEIGHT))
        .when(route == Route::Agents, |s| s.mt(px(SPACE_6)))
        .px(px(SPACE_2))
        .gap(px(SIDEBAR_TEXT_GAP))
        .rounded(px(RADIUS_MD))
        .debug_selector(move || match index {
            Some(index) => format!("sidebar-nav-{index}"),
            None => "sidebar-settings".into(),
        })
        .text_size(type_size(LABEL_SIZE))
        .text_color(rgb(tint))
        .when(selected, |s| {
            s.bg(rgb(SELECTED)).font_weight(FontWeight::MEDIUM)
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
                kbd(format!("⌘{}", index % 10))
                    .debug_selector(move || format!("sidebar-badge-{index}")),
            )
        })
        .when(index.is_none() && self.command_held, |s| s.child(kbd("⌘,")))
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
        let wide = self.session.panes[0].width >= 210.;
        let picker: ElementId = "workspace-picker".into();
        let picker_hover = self.hover.progress(&picker);
        let mut nav = column().gap(px(2.));
        for (index, page) in PAGES.iter().filter(|page| page.in_sidebar).enumerate() {
            nav = nav.child(self.sidebar_item(page.route, Some(index + 1), cx));
        }
        // Settings closes the navigation as its own group; the user menu repeats it.
        nav = nav.child(self.sidebar_item(Route::Settings, None, cx).mt(px(SPACE_6)));
        let user_menu = match self.overlays.borrow().active() {
            Some(Overlay::UserMenu { support }) => Some(support),
            _ => None,
        };
        let user_menu_open = user_menu.is_some();
        column()
            .w_full()
            .h_full()
            .flex_shrink_0()
            .debug_selector(|| "sidebar-content".into())
            .px(px(SIDEBAR_INSET))
            .pt(px(SPACE_4))
            .pb(px(SPACE_2))
            .child(
                // A bordered card: even insets, an eyebrow, the mark and the name.
                self.button(
                    picker.clone(),
                    "Switch workspace",
                    Control::WorkspacePicker,
                    cx,
                )
                .w_full()
                .min_w_0()
                .p(px(SPACE_4))
                .mb(px(SPACE_3))
                .rounded(px(RADIUS_LG))
                .border_1()
                .border_color(rgb(BORDER))
                .bg(blend(SURFACE_RAISED, HOVER_STRONG, picker_hover))
                .child(
                    column()
                        .flex_1()
                        .min_w_0()
                        .gap(px(SPACE_2))
                        .child(eyebrow("Workspace"))
                        .child(
                            row()
                                .gap(px(SPACE_2 + 2.))
                                .child(
                                    row()
                                        .size(px(WORKSPACE_MARK_SIZE))
                                        .flex_shrink_0()
                                        .justify_center()
                                        .rounded(px(RADIUS_MD))
                                        .border_1()
                                        .border_color(rgb(BORDER_STRONG))
                                        .bg(rgb(workspace_color.unwrap_or(SURFACE_CONTROL)))
                                        .text_size(type_size(TITLE_SIZE))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(rgb(workspace_ink))
                                        .child(workspace_icon),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .debug_selector(|| "workspace-title".into())
                                        .text_size(type_size(BODY_SIZE))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(rgb(TEXT))
                                        .child(workspace_name),
                                )
                                .child(icon("chevronUpDown", ICON_SIZE_SM)),
                        ),
                ),
            )
            .child(
                self.button("shell.search", "Search · ⌘ K", Control::Search, cx)
                    .debug_selector(|| "shell.search".into())
                    .w_full()
                    .min_w_0()
                    .h(px(CONTROL_HEIGHT))
                    .mb(px(SPACE_4))
                    .pl(px(SPACE_3))
                    .pr(px(HEADER_SEARCH_RIGHT_INSET))
                    .gap(px(SPACE_2 + 2.))
                    .rounded(px(RADIUS_MD))
                    .bg(rgb(SURFACE_INPUT))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .text_color(rgb(TEXT_SECONDARY))
                    .text_size(type_size(LABEL_SIZE))
                    .child(icon("search", ICON_SIZE_SM))
                    .child(div().flex_1().min_w_0().truncate().child("Go to…"))
                    .when(wide, |s| s.child(kbd("⌘K"))),
            )
            .child(nav)
            .child(div().flex_1())
            .child(
                // A flex column gives the floating menu the row's top-left as its origin.
                column()
                    .relative()
                    .w_full()
                    .child(
                        self.button("profile", "Account menu", Control::UserMenu, cx)
                            .h(px(44.))
                            .flex_shrink_0()
                            .w_full()
                            .items_center()
                            .px(px(SPACE_2 - 2.))
                            .gap(px(SPACE_2 - 2.))
                            .rounded(px(RADIUS_MD))
                            .when(user_menu_open, |s| s.bg(rgb(SELECTED)))
                            .debug_selector(|| "sidebar-profile".into())
                            .child(avatar(
                                &self.profile.name,
                                self.profile.photo.clone(),
                                AVATAR_SIZE,
                            ))
                            .child(
                                column()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .truncate()
                                            .debug_selector(|| "sidebar-profile-name".into())
                                            .text_size(type_size(LABEL_SIZE))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(rgb(TEXT))
                                            .child(self.profile.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .truncate()
                                            .debug_selector(|| "sidebar-profile-handle".into())
                                            .text_size(type_size(CAPTION_SIZE))
                                            .text_color(rgb(TEXT_TERTIARY))
                                            .child(handle_for(&self.profile.name)),
                                    ),
                            )
                            .child(icon("chevronUpDown", ICON_SIZE_SM)),
                    )
                    .when_some(user_menu, |s, support| s.child(self.user_menu(support, cx))),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::handle_for;

    #[test]
    fn handles_are_lowercase_alphanumeric() {
        assert_eq!(handle_for("Calum"), "@calum");
        assert_eq!(handle_for("Calum Webb"), "@calumwebb");
        assert_eq!(handle_for("  "), "@you");
    }
}
