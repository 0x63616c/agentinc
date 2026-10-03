use super::*;

impl Shell {
    fn sidebar_item(&self, route: Route, index: usize, cx: &mut Context<Self>) -> Stateful<Div> {
        let selected = self.session.current() == route;
        let tint = if selected { TEXT } else { TEXT_SECONDARY };
        let hover_group = format!("sidebar-item-{index}");
        self.button(("nav", index), route.label(), Control::Navigate(route), cx)
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
            .debug_selector(move || format!("sidebar-nav-{index}"))
            .text_size(type_size(LABEL_SIZE))
            .text_color(rgb(tint))
            .when(selected, |s| {
                s.bg(rgb(SELECTED)).font_weight(FontWeight::MEDIUM)
            })
            .child(nav_icon(route.icon(), selected, tint, hover_group))
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
                    kbd(format!("⌘{}", index % 10))
                        .debug_selector(move || format!("sidebar-badge-{index}")),
                )
            })
    }

    pub(super) fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut nav = column().gap(px(2.));
        for (index, page) in PAGES.iter().filter(|page| page.in_sidebar).enumerate() {
            nav = nav.child(self.sidebar_item(page.route, index + 1, cx));
        }
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
                self.button("shell.search", "Search · ⌘ K", Control::Search, cx)
                    .debug_selector(|| "shell.search".into())
                    .w_full()
                    .min_w_0()
                    .h(px(CONTROL_HEIGHT))
                    .mb(px(SPACE_4))
                    // The magnifier centres on the navigation icon column and the
                    // placeholder lands on the label rail.
                    .pl(px(SPACE_2))
                    .pr(px(HEADER_SEARCH_RIGHT_INSET))
                    // One more than the label rail's gap: the field's border sits inside it.
                    .gap(px(SPACE_3 + 1.))
                    .rounded(px(RADIUS_MD))
                    .bg(rgb(SURFACE_INPUT))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .text_color(rgb(TEXT_SECONDARY))
                    .text_size(type_size(LABEL_SIZE))
                    .child(icon("search", ICON_SIZE_SM))
                    .child(div().flex_1().min_w_0().truncate().child("Go to…"))
                    .child(kbd("⌘K").debug_selector(|| "sidebar-search-shortcut".into())),
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
                            .pl(px(SIDEBAR_PROFILE_INSET))
                            // Align the visible glyph, not its transparent SVG box.
                            .pr(px(SIDEBAR_PROFILE_INSET - CHEVRON_UP_DOWN_GLYPH_INSET))
                            // Keep the name on the navigation label rail.
                            .gap(px(SPACE_1))
                            .rounded(px(RADIUS_MD))
                            .when(user_menu_open, |s| s.bg(rgb(SELECTED)))
                            .debug_selector(|| "sidebar-profile".into())
                            .child(
                                div()
                                    .size(px(AVATAR_SIZE))
                                    .flex_shrink_0()
                                    .debug_selector(|| "sidebar-profile-avatar".into())
                                    .child(avatar(
                                        &self.profile.name,
                                        self.profile.photo.clone(),
                                        AVATAR_SIZE,
                                    )),
                            )
                            .child(
                                column().flex_1().min_w_0().child(
                                    div()
                                        .truncate()
                                        .debug_selector(|| "sidebar-profile-name".into())
                                        .text_size(type_size(LABEL_SIZE))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(rgb(TEXT))
                                        .child(self.profile.name.clone()),
                                ),
                            )
                            .child(
                                icon("chevronUpDown", ICON_SIZE_SM)
                                    .debug_selector(|| "sidebar-profile-chevron".into()),
                            ),
                    )
                    .when_some(user_menu, |s, support| s.child(self.user_menu(support, cx))),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{Shell, bind_keys};
    use crate::{
        model::{Overlay, PANE_WIDTHS},
        ui::SPACE_2,
    };
    use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext, px};

    fn draw_sidebar(shell: &Entity<Shell>, width: f32, cx: &mut VisualTestContext) {
        cx.update(|window, cx| {
            shell.update(cx, |shell, cx| {
                shell.session.panes[0].width = width;
                shell.pane_visible[0] = width;
                cx.notify();
            });
            window.draw(cx).clear(cx);
        });
    }

    #[gpui::test]
    fn search_shortcut_stays_visible_and_clickable_at_every_sidebar_width(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        cx.update(bind_keys);
        let (shell, cx) = cx.add_window_view(|window, cx| {
            Shell::fixture(dir.path().join("session.json"), window, cx)
        });
        for command_held in [false, true] {
            shell.update(cx, |shell, _| shell.command_held = command_held);
            for width in [
                PANE_WIDTHS[0].0,
                209.,
                210.,
                PANE_WIDTHS[0].2,
                PANE_WIDTHS[0].1,
            ] {
                draw_sidebar(&shell, width, cx);
                let search = cx.debug_bounds("shell.search").unwrap();
                let shortcut = cx.debug_bounds("sidebar-search-shortcut").unwrap();
                assert!(shortcut.size.width > px(0.));
                assert!(shortcut.size.height > px(0.));
                assert!(search.contains(&shortcut.origin));
                assert!(search.contains(&shortcut.bottom_right()));
            }
        }
        draw_sidebar(&shell, PANE_WIDTHS[0].0, cx);
        let shortcut_center = cx.debug_bounds("sidebar-search-shortcut").unwrap().center();
        cx.simulate_click(shortcut_center, Modifiers::default());
        shell.read_with(cx, |shell, _| {
            assert_eq!(shell.overlays.borrow().active(), Some(Overlay::Search));
        });
        cx.simulate_keystrokes("escape");
        shell.read_with(cx, |shell, _| {
            assert_eq!(shell.overlays.borrow().active(), None);
        });
        cx.simulate_keystrokes("cmd-k");
        shell.read_with(cx, |shell, _| {
            assert_eq!(shell.overlays.borrow().active(), Some(Overlay::Search));
        });
    }

    #[gpui::test]
    fn profile_edges_match_navigation_insets_and_label_rail_with_a_long_name(
        cx: &mut TestAppContext,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let (shell, cx) = cx.add_window_view(|window, cx| {
            Shell::fixture(dir.path().join("session.json"), window, cx)
        });
        shell.update(cx, |shell, _| {
            shell.profile.name = "A deliberately long profile display name".into();
        });
        for width in [PANE_WIDTHS[0].0, PANE_WIDTHS[0].2, PANE_WIDTHS[0].1] {
            draw_sidebar(&shell, width, cx);
            let profile = cx.debug_bounds("sidebar-profile").unwrap();
            let avatar = cx.debug_bounds("sidebar-profile-avatar").unwrap();
            let chevron = cx.debug_bounds("sidebar-profile-chevron").unwrap();
            let name = cx.debug_bounds("sidebar-profile-name").unwrap();
            let navigation = cx.debug_bounds("sidebar-nav-1").unwrap();
            let navigation_label = cx.debug_bounds("sidebar-label-1").unwrap();
            let left_inset = f32::from(avatar.origin.x - profile.origin.x);
            // The glyph's rightmost rounded stroke is x=17.75 in its 24-point SVG.
            let visible_chevron_right =
                f32::from(chevron.origin.x) + f32::from(chevron.size.width) * 17.75 / 24.;
            let right_inset = f32::from(profile.right()) - visible_chevron_right;
            assert!(
                (f32::from(avatar.origin.x - navigation.origin.x) - SPACE_2).abs() < 0.5,
                "avatar must share the navigation icon inset at width {width}"
            );
            assert!(
                (right_inset - left_inset).abs() < 0.5,
                "insets at width {width}: {left_inset} / {right_inset}"
            );
            assert!((f32::from(name.origin.x - navigation_label.origin.x)).abs() < 0.5);
            assert!(name.right() <= chevron.origin.x);
        }
        let chevron_center = cx.debug_bounds("sidebar-profile-chevron").unwrap().center();
        cx.simulate_click(chevron_center, Modifiers::default());
        shell.read_with(cx, |shell, _| {
            assert_eq!(
                shell.overlays.borrow().active(),
                Some(Overlay::UserMenu { support: false })
            );
        });
    }
}
