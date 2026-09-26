//! The user menu popover and the notification panel.
use super::*;

impl Shell {
    fn menu_action(
        control: Control,
    ) -> impl Fn(&mut Self, &mut Window, &mut Context<Self>) + Clone {
        move |this, window, cx| this.dispatch(control.clone(), window, cx)
    }

    pub(super) fn user_menu(&self, support: bool, cx: &mut Context<Self>) -> Deferred {
        let update_ready = cx
            .try_global::<crate::updates::Updates>()
            .is_some_and(|updates| updates.0.read(cx).is_ready());
        let support = column()
            .relative()
            .child(
                MenuEntry::new("user-menu.support", "Support")
                    .icon("help")
                    .trailing(
                        row()
                            .w(px(ICON_SIZE - CHEVRON_GLYPH_INSET))
                            .overflow_hidden()
                            .child(icon("chevronRight", ICON_SIZE)),
                    )
                    .selector("user-menu.support")
                    .build(&self.hover, Self::menu_action(Control::SupportMenu), cx)
                    .when(support, |s| s.bg(rgb(SELECTED))),
            )
            .when(support, |s| {
                s.child(floating(
                    menu_shell(MENU_WIDTH)
                        .debug_selector(|| "user-menu.support.menu".into())
                        .child(
                            MenuEntry::new("user-menu.help", "Help Center")
                                .icon("arrowUpRight")
                                .build(&self.hover, Self::menu_action(Control::HelpCenter), cx),
                        )
                        .child(
                            MenuEntry::new("user-menu.feedback", "Send Feedback")
                                .icon("feedback")
                                .build(&self.hover, Self::menu_action(Control::SendFeedback), cx),
                        )
                        .child(
                            MenuEntry::new("user-menu.about", "About AgentInc")
                                .icon("info")
                                .build(&self.hover, Self::menu_action(Control::About), cx),
                        ),
                    Anchor::TopLeft,
                    point(px(POPOVER_WIDTH - CHIP_GAP), px(-MENU_INSET)),
                ))
            });
        let menu = popover_shell(POPOVER_WIDTH)
            .debug_selector(|| "user-menu".into())
            .child(
                row()
                    .px(px(SPACE_2))
                    .py(px(SPACE_2))
                    .gap(px(SPACE_3))
                    .child(avatar(
                        &self.profile.name,
                        self.profile.photo.clone(),
                        AVATAR_SIZE_LG,
                    ))
                    .child(
                        column()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .truncate()
                                    .text_size(type_size(HEADING_SIZE))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(self.profile.name.clone()),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_size(type_size(CAPTION_SIZE))
                                    .text_color(rgb(TEXT_SECONDARY))
                                    .child(handle_for(&self.profile.name)),
                            ),
                    ),
            )
            .when(update_ready, |s| {
                s.child(
                    column()
                        .debug_selector(|| "user-menu.update".into())
                        .mx(px(MENU_INSET))
                        .mb(px(MENU_INSET))
                        .p(px(SPACE_3))
                        .gap(px(SPACE_2))
                        .rounded(px(RADIUS_MD))
                        .bg(rgb(SURFACE_RAISED))
                        .border_1()
                        .border_color(rgb(BORDER))
                        .child(
                            row()
                                .gap(px(SPACE_2))
                                .child(icon("download", ICON_SIZE_SM).text_color(rgb(TEXT)))
                                .child(
                                    div()
                                        .text_size(type_size(LABEL_SIZE))
                                        .font_weight(FontWeight::MEDIUM)
                                        .child("New update available"),
                                ),
                        )
                        .child(
                            Button::new("user-menu.install", "Install")
                                .primary()
                                .small()
                                .full_width()
                                .build(&self.hover, Self::menu_action(Control::InstallUpdate), cx),
                        ),
                )
            })
            .child(menu_divider())
            .child(
                MenuEntry::new("user-menu.updates", "Check for Updates")
                    .icon("download")
                    .build(&self.hover, Self::menu_action(Control::CheckForUpdates), cx),
            )
            .child(
                MenuEntry::new("user-menu.settings", "Settings")
                    .icon("settings")
                    .shortcut("⌘,")
                    .build(
                        &self.hover,
                        Self::menu_action(Control::Navigate(Route::Settings)),
                        cx,
                    ),
            )
            .child(support)
            .child(menu_divider())
            .child(menu_label("Local users · coming soon"))
            .child(
                MenuEntry::new("user-menu.switch-user", "Switch user")
                    .icon("users")
                    .enabled(false)
                    .build(&self.hover, |_, _, _| {}, cx),
            )
            .child(
                MenuEntry::new("user-menu.add-user", "Add user")
                    .icon("plus")
                    .enabled(false)
                    .build(&self.hover, |_, _, _| {}, cx),
            );
        floating(menu, Anchor::BottomLeft, point(px(0.), px(-SPACE_2)))
    }

    pub(super) fn notification_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let unread = self
            .notification_items
            .iter()
            .filter(|item| item.unread)
            .count();
        popover_shell(350.)
            .absolute()
            .top(px(TITLEBAR_HEIGHT + SPACE_1))
            .right(px(HEADER_EDGE_INSET))
            .debug_selector(|| "notifications.panel".into())
            .overflow_hidden()
            .child(
                row()
                    .h(px(44.))
                    .px(px(SPACE_2))
                    .gap(px(SPACE_2))
                    .child(
                        div()
                            .px(px(SPACE_1))
                            .font_weight(FontWeight::MEDIUM)
                            .child("Notifications"),
                    )
                    .when(unread > 0, |s| s.child(count_badge(unread)))
                    .child(div().flex_1())
                    .when(unread > 0, |s| {
                        s.child(
                            Button::new("mark-all-read", "Mark all read")
                                .ghost()
                                .small()
                                .build(&self.hover, Self::menu_action(Control::MarkAllRead), cx),
                        )
                    })
                    .child(
                        Button::new("dismiss-notifications", "Close notifications")
                            .icon("close")
                            .icon_only()
                            .ghost()
                            .small()
                            .build(&self.hover, Self::menu_action(Control::Dismiss), cx),
                    ),
            )
            .child(menu_divider())
            .when(self.notification_items.is_empty(), |s| {
                s.child(
                    column()
                        .py(px(SPACE_6))
                        .px(px(SPACE_4))
                        .items_center()
                        .gap(px(SPACE_2))
                        .child(icon("inbox", ICON_SIZE_LG))
                        .child(
                            div()
                                .text_size(type_size(LABEL_SIZE))
                                .font_weight(FontWeight::MEDIUM)
                                .child("You're all caught up"),
                        )
                        .child(caption(
                            "Activity from agents, tickets and runs lands here.",
                        )),
                )
            })
            .children(
                self.notification_items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        list_item(("notification", index), item.title.clone())
                            .items_start()
                            .gap(px(SPACE_3))
                            .px(px(SPACE_3))
                            .py(px(SPACE_3))
                            .rounded(px(RADIUS_MD))
                            .child(div().mt(px(2.)).child(icon(item.icon, ICON_SIZE)))
                            .child(
                                column()
                                    .flex_1()
                                    .min_w_0()
                                    .gap(px(SPACE_HALF))
                                    .child(
                                        row()
                                            .gap(px(SPACE_2))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .truncate()
                                                    .text_size(type_size(LABEL_SIZE))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .child(item.title.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_size(type_size(CAPTION_SIZE))
                                                    .text_color(rgb(TEXT_TERTIARY))
                                                    .child(item.relative_time.clone()),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_size(type_size(CAPTION_SIZE))
                                            .text_color(rgb(TEXT_SECONDARY))
                                            .child(item.body.clone()),
                                    ),
                            )
                            .when(item.unread, |s| {
                                s.child(div().mt(px(6.)).child(status_dot(Tone::Accent)))
                            })
                    }),
            )
    }
}
