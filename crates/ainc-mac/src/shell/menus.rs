//! The user menu popover.
use super::*;

impl Shell {
    fn menu_action(
        control: Control,
    ) -> impl Fn(&mut Self, &mut Window, &mut Context<Self>) + Clone {
        move |this, window, cx| this.dispatch(control.clone(), window, cx)
    }

    pub(super) fn user_menu(&self, support: bool, ui: &mut Ui<Self>) -> Deferred {
        let update_ready = ui
            .cx
            .try_global::<crate::updates::Updates>()
            .is_some_and(|updates| updates.0.read(ui.cx).is_ready());
        let support = column()
            .relative()
            .child(
                MenuEntry::new("user-menu.support", "Support")
                    .icon(Icon::Help)
                    .trailing(
                        row()
                            .debug_selector(|| "user-menu.support.chevron".into())
                            .w(px(ICON_SIZE))
                            .flex_none()
                            .child(icon(Icon::ChevronRight, ICON_SIZE)),
                    )
                    .build(ui, Self::menu_action(Control::SupportMenu))
                    .when(support, |s| s.bg(rgb(SELECTED))),
            )
            .when(support, |s| {
                s.child(floating(
                    menu_shell(MENU_WIDTH)
                        .debug_selector(|| "user-menu.support.menu".into())
                        .gap(px(MENU_INSET))
                        .child(
                            MenuEntry::new("user-menu.help", "Help Center")
                                .icon(Icon::ArrowUpRight)
                                .build(ui, Self::menu_action(Control::HelpCenter)),
                        )
                        .child(
                            MenuEntry::new("user-menu.feedback", "Send Feedback")
                                .icon(Icon::Feedback)
                                .build(ui, Self::menu_action(Control::SendFeedback)),
                        )
                        .child(
                            MenuEntry::new("user-menu.about", "About AgentInc")
                                .icon(Icon::Info)
                                .build(ui, Self::menu_action(Control::About)),
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
                        column().flex_1().min_w_0().child(
                            div()
                                .truncate()
                                .text_size(type_size(HEADING_SIZE))
                                .font_weight(FontWeight::MEDIUM)
                                .child(self.profile.name.clone()),
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
                                .child(icon(Icon::Download, ICON_SIZE_SM).text_color(rgb(TEXT)))
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
                                .build(ui, Self::menu_action(Control::InstallUpdate)),
                        ),
                )
            })
            .child(menu_divider().debug_selector(|| "user-menu.divider".into()))
            .child(
                column()
                    .gap(px(MENU_INSET))
                    .child(
                        MenuEntry::new("user-menu.updates", "Check for Updates")
                            .icon(Icon::Download)
                            .build(ui, Self::menu_action(Control::CheckForUpdates)),
                    )
                    .child(
                        MenuEntry::new("user-menu.settings", "Settings")
                            .icon(Icon::Settings)
                            .shortcut(shortcuts::SETTINGS.glyph)
                            .build(
                                ui,
                                Self::menu_action(Control::Go(Destination::Page(Route::Settings))),
                            ),
                    )
                    .child(support),
            );
        floating(menu, Anchor::BottomLeft, point(px(0.), px(-SPACE_2)))
    }
}
