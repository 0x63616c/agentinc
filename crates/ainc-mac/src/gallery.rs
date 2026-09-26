//! The living component gallery: every design-system piece rendered from the
//! same code the app uses. Reachable from the command palette as "Components".
use crate::{input::TextInput, ui::*};
use gpui::{prelude::*, *};
use std::time::Instant;

pub const SECTIONS: [&str; 4] = ["Buttons", "Inputs", "Data", "Overlays"];

pub struct GalleryPage {
    section: usize,
    toggle_on: bool,
    checked: bool,
    segment: usize,
    chip: usize,
    select_value: Option<usize>,
    select_open: bool,
    text: Entity<TextInput>,
    error_text: Entity<TextInput>,
    search: Entity<TextInput>,
    area: Entity<TextInput>,
    toasts: Toasts,
    started: Instant,
    hover: HoverFade,
    _subscriptions: Vec<Subscription>,
}

impl HoverHost for GalleryPage {
    fn hover_fade(&mut self) -> &mut HoverFade {
        &mut self.hover
    }
}

fn specimen(title: &'static str, note: &'static str, content: impl IntoElement) -> Div {
    column()
        .debug_selector(move || format!("gallery.{}", title.to_lowercase().replace(' ', "-")))
        .gap(px(SPACE_2))
        .child(
            column()
                .gap(px(SPACE_HALF))
                .px(px(SPACE_HALF))
                .child(eyebrow(title))
                .child(caption(note)),
        )
        .child(card().gap(px(SPACE_3)).child(content))
}

fn columns() -> [TableColumn; 3] {
    [
        TableColumn::new("Name"),
        TableColumn::new("Status").width(140.),
        TableColumn::new("Updated").width(110.).right(),
    ]
}

impl GalleryPage {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let text =
            cx.new(|cx| TextInput::field("Ticket title", false, cx).identified("gallery.text"));
        let error_text = cx.new(|cx| {
            let mut input = TextInput::field("Rule name", false, cx).identified("gallery.error");
            input.content = "Every 0 minutes".into();
            input
        });
        let search =
            cx.new(|cx| TextInput::field("Search…", false, cx).identified("gallery.search"));
        let area = cx.new(|cx| {
            let mut input = TextInput::composer(cx).identified("gallery.area");
            input.content = "Review the open Tickets, then plan the next agent run.".into();
            input
        });
        let subscriptions = [&text, &error_text, &search, &area]
            .into_iter()
            .map(|input| cx.observe(input, |_, _, cx| cx.notify()))
            .collect();
        let mut toasts = Toasts::default();
        toasts.push(
            "Ticket assigned to Evee",
            Some("Reconcile weekly budget and receipts".into()),
            Tone::Success,
        );
        Self {
            section: 0,
            toggle_on: true,
            checked: true,
            segment: 1,
            chip: 0,
            select_value: Some(0),
            select_open: false,
            text,
            error_text,
            search,
            area,
            toasts,
            started: Instant::now(),
            hover: HoverFade::default(),
            _subscriptions: subscriptions,
        }
    }

    #[cfg(all(test, feature = "rendered-tests"))]
    #[allow(dead_code)]
    pub(crate) fn fixture_section(&mut self, section: usize, cx: &mut Context<Self>) {
        self.section = section.min(SECTIONS.len() - 1);
        self.select_open = false;
        cx.notify();
    }

    #[cfg(all(test, feature = "rendered-tests"))]
    #[allow(dead_code)]
    pub(crate) fn fixture_select_open(&mut self, open: bool, cx: &mut Context<Self>) {
        self.select_open = open;
        cx.notify();
    }

    fn buttons(&self, cx: &mut Context<Self>) -> Div {
        let noop = |_: &mut Self, _: &mut Window, _: &mut Context<Self>| {};
        column()
            .gap(px(SECTION_GAP))
            .child(specimen(
                "Variants",
                "One white primary per surface. Secondary beside it, ghost for quiet rows, destructive only when something is removed.",
                row().flex_wrap().gap(px(CONTROL_GAP))
                    .child(Button::new("gallery.primary", "Primary").primary().build(&self.hover, noop, cx))
                    .child(Button::new("gallery.secondary", "Secondary").secondary().build(&self.hover, noop, cx))
                    .child(Button::new("gallery.ghost", "Ghost").ghost().on_surface(SURFACE_RAISED).build(&self.hover, noop, cx))
                    .child(Button::new("gallery.destructive", "Delete").destructive().build(&self.hover, noop, cx)),
            ))
            .child(specimen(
                "Sizes and icons",
                "Small for dense rows, regular everywhere else, large for the one action on an empty page.",
                row().flex_wrap().items_center().gap(px(CONTROL_GAP))
                    .child(Button::new("gallery.small", "Small").secondary().small().icon("plus").build(&self.hover, noop, cx))
                    .child(Button::new("gallery.regular", "New Ticket").primary().icon("plus").build(&self.hover, noop, cx))
                    .child(Button::new("gallery.large", "Get started").primary().large().build(&self.hover, noop, cx))
                    .child(Button::icon_only("gallery.icon-ghost", "more", "More").build(&self.hover, noop, cx))
                    .child(Button::icon_only("gallery.icon-secondary", "refresh", "Refresh").secondary().build(&self.hover, noop, cx))
                    .child(Button::icon_only("gallery.icon-primary", "send", "Send").primary().build(&self.hover, noop, cx).rounded_full()),
            ))
            .child(specimen(
                "States",
                "Disabled controls keep their layout at reduced opacity. Selected ghosts and secondaries pick up the selected surface.",
                row().flex_wrap().gap(px(CONTROL_GAP))
                    .child(Button::new("gallery.disabled-primary", "Primary").primary().enabled(false).build(&self.hover, noop, cx))
                    .child(Button::new("gallery.disabled-secondary", "Secondary").secondary().enabled(false).build(&self.hover, noop, cx))
                    .child(Button::new("gallery.selected-secondary", "Selected").secondary().selected(true).build(&self.hover, noop, cx))
                    .child(Button::new("gallery.selected-ghost", "Selected ghost").ghost().selected(true).build(&self.hover, noop, cx))
                    .child(Button::new("gallery.trailing", "Go to…").secondary().icon("search").trailing(kbd("⌘K")).build(&self.hover, noop, cx)),
            ))
    }

    fn inputs(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        column()
            .gap(px(SECTION_GAP))
            .child(specimen(
                "Fields",
                "Label above, hint or error below. Focus lightens the border; errors turn it red.",
                row()
                    .items_start()
                    .gap(px(SPACE_4))
                    .child(
                        div().flex_1().child(
                            Field::new(self.text.clone())
                                .label("Title")
                                .hint("Say what needs doing.")
                                .build(window, cx),
                        ),
                    )
                    .child(
                        div().flex_1().child(
                            Field::new(self.error_text.clone())
                                .label("Every (minutes)")
                                .error(Some("Enter an interval in whole minutes."))
                                .build(window, cx),
                        ),
                    )
                    .child(
                        div().flex_1().child(
                            Field::new(self.search.clone())
                                .label("Search")
                                .leading_icon("search")
                                .build(window, cx),
                        ),
                    ),
            ))
            .child(specimen(
                "Text area",
                "Multiline fields grow with their content and start top-aligned.",
                Field::new(self.area.clone())
                    .label("Instructions")
                    .multiline()
                    .build(window, cx),
            ))
            .child(specimen(
                "Select",
                "The menu floats above the page, so opening it never resizes its row.",
                row()
                    .gap(px(SPACE_4))
                    .items_center()
                    .child(
                        Select::new(
                            "gallery.select",
                            vec![
                                SelectOption::new("Codex default"),
                                SelectOption::new("Codex One").description("Fast"),
                                SelectOption::new("Codex Two").description("Careful"),
                            ],
                        )
                        .value(self.select_value)
                        .open(self.select_open)
                        .width(240.)
                        .build(
                            &self.hover,
                            |this, _, cx| {
                                this.select_open = !this.select_open;
                                cx.notify();
                            },
                            |this, index, _, cx| {
                                this.select_value = Some(index);
                                this.select_open = false;
                                cx.notify();
                            },
                            cx,
                        ),
                    )
                    .child(
                        Select::new(
                            "gallery.select-empty",
                            vec![SelectOption::new("Evee"), SelectOption::new("Scout")],
                        )
                        .placeholder("Choose an agent")
                        .width(200.)
                        .build(
                            &self.hover,
                            |_, _, _| {},
                            |_, _, _, _| {},
                            cx,
                        ),
                    )
                    .child(caption("Selects show a check beside the current option.")),
            ))
            .child(specimen(
                "Toggles and checkboxes",
                "The on-state is white. Toggles spring; checkboxes fill.",
                row()
                    .gap(px(SPACE_6))
                    .items_center()
                    .child(
                        row()
                            .gap(px(SPACE_3))
                            .child(toggle(
                                "gallery.toggle",
                                "Automatic checks",
                                self.toggle_on,
                                true,
                                |this, _, cx| {
                                    this.toggle_on = !this.toggle_on;
                                    cx.notify();
                                },
                                cx,
                            ))
                            .child(
                                div()
                                    .text_size(type_size(LABEL_SIZE))
                                    .child("Automatic checks"),
                            ),
                    )
                    .child(
                        row()
                            .gap(px(SPACE_3))
                            .child(toggle(
                                "gallery.toggle-off",
                                "Off",
                                false,
                                true,
                                |_, _, _| {},
                                cx,
                            ))
                            .child(div().text_size(type_size(LABEL_SIZE)).child("Off")),
                    )
                    .child(checkbox(
                        "gallery.checkbox",
                        "Include done Tickets",
                        self.checked,
                        true,
                        |this, _, cx| {
                            this.checked = !this.checked;
                            cx.notify();
                        },
                        cx,
                    ))
                    .child(checkbox(
                        "gallery.checkbox-disabled",
                        "Disabled",
                        false,
                        false,
                        |_, _, _| {},
                        cx,
                    )),
            ))
            .child(specimen(
                "Segmented control and chips",
                "Segments switch views or scales; chips pick one value from a short set.",
                column()
                    .gap(px(SPACE_3))
                    .child(segmented(
                        "gallery.segmented",
                        ["Small", "Default", "Large", "Larger"],
                        self.segment,
                        true,
                        &self.hover,
                        |this, index, _, cx| {
                            this.segment = index;
                            cx.notify();
                        },
                        cx,
                    ))
                    .child(
                        row().flex_wrap().gap(px(CHIP_GAP)).children(
                            ["Backlog", "To do", "In progress", "Done"]
                                .into_iter()
                                .enumerate()
                                .map(|(index, label)| {
                                    chip(
                                        ElementId::NamedInteger(
                                            "gallery.chip".into(),
                                            index as u64,
                                        ),
                                        label,
                                        index == self.chip,
                                        true,
                                        &self.hover,
                                        move |this, _, cx| {
                                            this.chip = index;
                                            cx.notify();
                                        },
                                        cx,
                                    )
                                }),
                        ),
                    ),
            ))
    }

    fn data(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let columns = columns();
        let rows = [
            ("Reconcile weekly budget", "Running", Tone::Info, "2m ago"),
            ("Morning review", "Completed", Tone::Success, "1h ago"),
            ("House check", "Timed out", Tone::Danger, "1d ago"),
        ];
        column()
            .gap(px(SECTION_GAP))
            .child(specimen(
                "Badges and status",
                "Filled badges for counts and states; bordered pills with a dot inside tables.",
                column().gap(px(SPACE_3))
                    .child(row().flex_wrap().gap(px(SPACE_2)).children([
                        ("Neutral", Tone::Neutral), ("Success", Tone::Success), ("Info", Tone::Info),
                        ("Warning", Tone::Warning), ("Danger", Tone::Danger), ("Accent", Tone::Accent),
                    ].into_iter().map(|(label, tone)| badge(label, tone))).child(count_badge(3)))
                    .child(row().flex_wrap().gap(px(SPACE_2)).children([
                        ("Backlog", Tone::Neutral), ("Running", Tone::Info), ("Completed", Tone::Success),
                        ("Paused", Tone::Warning), ("Failed", Tone::Danger),
                    ].into_iter().map(|(label, tone)| status_pill(label, tone)))),
            ))
            .child(specimen(
                "Avatars and shortcuts",
                "Initials stand in for a photo. Shortcut hints sit inside buttons, rows and footers.",
                row().gap(px(SPACE_6)).items_center()
                    .child(row().gap(px(SPACE_2)).child(avatar("Calum Webb", None, AVATAR_SIZE)).child(avatar("Evee", None, AVATAR_SIZE_LG)))
                    .child(row().gap(px(SPACE_2)).child(kbd("⌘K")).child(kbd("↵")).child(kbd("esc")))
                    .child(kbd_hint("↑ ↓", "Navigate")),
            ))
            .child(specimen(
                "List rows",
                "Title, subtitle and a trailing element. Rows fade to their hover surface.",
                column().gap(px(SPACE_HALF))
                    .child(ListRow::new("gallery.row.1", "Reconcile weekly budget and receipts").leading(status_dot(Tone::Info)).subtitle("Evee").trailing(badge("Running", Tone::Info)).on_surface(SURFACE_RAISED).build(&self.hover, |_, _, _| {}, cx))
                    .child(ListRow::new("gallery.row.2", "Plan the week").leading(status_dot(Tone::Neutral)).subtitle("Unassigned").trailing(icon("chevronRight", ICON_SIZE_SM)).selected(true).build(&self.hover, |_, _, _| {}, cx)),
            ))
            .child(specimen(
                "Table",
                "An eyebrow header, aligned cells and clickable rows.",
                table_container()
                    .child(table_header(&columns))
                    .children(rows.into_iter().enumerate().map(|(index, (name, status, tone, when))| {
                        table_row(
                            ElementId::NamedInteger("gallery.table".into(), index as u64),
                            name,
                            &columns,
                            vec![
                                div().text_color(rgb(TEXT)).child(name).into_any_element(),
                                row().child(status_pill(status, tone)).into_any_element(),
                                caption(when).into_any_element(),
                            ],
                            true,
                            &self.hover,
                            |_, _, _| {},
                            cx,
                        )
                    })),
            ))
            .child(specimen(
                "Empty and loading",
                "Empty states offer the one next action. Skeletons hold the layout while data loads.",
                column().gap(px(SPACE_4))
                    .child(EmptyState::new("tasks", "No Tickets yet").description("Create a Ticket and assign it to an agent to start work.").action(Button::new("gallery.empty-action", "New Ticket").primary().icon("plus").build(&self.hover, |_, _, _| {}, cx)).build())
                    .child(skeleton_rows("gallery.skeleton", 3))
                    .child(row().gap(px(SPACE_6)).child(LoadingFrame::new(self.started, window).inline("Evee is thinking…")))
                    .child(LoadingFrame::new(self.started, window).page("Loading Tickets…")),
            ))
    }

    fn overlays(&self, cx: &mut Context<Self>) -> Div {
        let noop = |_: &mut Self, _: &mut Window, _: &mut Context<Self>| {};
        column()
            .gap(px(SECTION_GAP))
            .child(specimen(
                "Dialog",
                "Centered on a scrim with a title, body and right-aligned actions.",
                row().child(dialog_shell(
                    "Delete “Plan the week”?",
                    caption("This Ticket and its Comments will be removed."),
                    row_gap(CONTROL_GAP).justify_end()
                        .child(Button::new("gallery.dialog-cancel", "Cancel").secondary().build(&self.hover, noop, cx))
                        .child(Button::new("gallery.dialog-confirm", "Delete").destructive().build(&self.hover, noop, cx)),
                )),
            ))
            .child(specimen(
                "Sheet",
                "A full-height panel from the right edge for editing without leaving the page.",
                div().relative().h(px(300.)).w_full().child(
                    div().absolute().top_0().right_0().bottom_0().child(sheet_shell(
                        "Edit rule",
                        column().gap(px(SPACE_3)).child(caption("Sheets keep the page visible beside them.")),
                        row_gap(CONTROL_GAP).justify_end()
                            .child(Button::new("gallery.sheet-cancel", "Cancel").secondary().build(&self.hover, noop, cx))
                            .child(Button::new("gallery.sheet-save", "Save").primary().build(&self.hover, noop, cx)),
                    )),
                ),
            ))
            .child(specimen(
                "Menus and popovers",
                "Dropdowns and the user menu share one floating surface, item rhythm and shortcut hints.",
                row().items_start().gap(px(SPACE_6))
                    .child(popover_shell(POPOVER_WIDTH)
                        .child(row().px(px(SPACE_2)).py(px(SPACE_2)).gap(px(SPACE_3)).child(avatar("Calum", None, AVATAR_SIZE_LG)).child(column().child(div().text_size(type_size(HEADING_SIZE)).font_weight(FontWeight::MEDIUM).child("Calum")).child(caption("@calum"))))
                        .child(menu_divider())
                        .child(MenuEntry::new("gallery.menu.updates", "Check for Updates").icon("refresh").build(&self.hover, noop, cx))
                        .child(MenuEntry::new("gallery.menu.settings", "Settings").icon("settings").shortcut("⌘,").build(&self.hover, noop, cx))
                        .child(MenuEntry::new("gallery.menu.support", "Support").icon("help").trailing(icon("chevronRight", ICON_SIZE_SM)).build(&self.hover, noop, cx))
                        .child(menu_divider())
                        .child(MenuEntry::new("gallery.menu.delete", "Delete").icon("trash").destructive().build(&self.hover, noop, cx)))
                    .child(menu_shell(MENU_WIDTH)
                        .child(menu_label("Model"))
                        .child(MenuEntry::new("gallery.menu.default", "Codex default").checked(true).build(&self.hover, noop, cx))
                        .child(MenuEntry::new("gallery.menu.one", "Codex One").build(&self.hover, noop, cx))
                        .child(MenuEntry::new("gallery.menu.two", "Codex Two").enabled(false).build(&self.hover, noop, cx))),
            ))
            .child(specimen(
                "Toasts",
                "Transient notices stack above the status bar and slide in; sticky ones wait to be dismissed.",
                column().gap(px(SPACE_3))
                    .child(row().child(Button::new("gallery.toast", "Show a toast").secondary().build(
                        &self.hover,
                        |this, _, cx| {
                            this.toasts.push("Saved", Some("Your changes are on the daemon.".into()), Tone::Info);
                            cx.notify();
                        },
                        cx,
                    )))
                    .child(div().relative().w_full().h(px(200.)).child(self.toasts.render(
                        &self.hover,
                        |this, id, _, cx| {
                            this.toasts.dismiss(id);
                            cx.notify();
                        },
                        cx,
                    ))),
            ))
    }
}

impl Render for GalleryPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.hover.animate(window);
        let body = match self.section {
            0 => self.buttons(cx),
            1 => self.inputs(window, cx),
            2 => self.data(window, cx),
            _ => self.overlays(cx),
        };
        Page::document(PageHeader::new("Components").description(
            "The AgentInc design system, rendered by the app itself. See docs/design-system.md.",
        ))
        .child(
            column()
                .id("gallery.page")
                .gap(px(SECTION_GAP))
                .child(tabs(
                    "gallery.tabs",
                    SECTIONS,
                    self.section,
                    &self.hover,
                    |this, index, _, cx| {
                        this.section = index;
                        cx.notify();
                    },
                    cx,
                ))
                .child(body),
        )
        .build()
    }
}
