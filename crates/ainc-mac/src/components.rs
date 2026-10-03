//! The living component gallery: every design-system piece rendered from the
//! same code the app uses. Reachable from the command palette as "Components".
use crate::{
    input::TextInput,
    overlay::Overlay,
    page::{Page, PageOverlays},
    routes::{Destination, Route},
    ui::*,
};
use gpui::{prelude::*, *};
use std::{cell::RefCell, rc::Rc, time::Instant};

const SELECT_ID: &str = "components.select";

pub const SECTIONS: [&str; 4] = ["Buttons", "Inputs", "Data", "Overlays"];

pub struct ComponentsPage {
    section: usize,
    toggle_on: bool,
    checked: bool,
    segment: usize,
    chip: usize,
    select_value: Option<usize>,
    overlays: PageOverlays<()>,
    text: Entity<TextInput>,
    error_text: Entity<TextInput>,
    search: Entity<TextInput>,
    area: Entity<TextInput>,
    toasts: Toasts,
    started: Instant,
    _subscriptions: Vec<Subscription>,
}

fn specimen(title: &'static str, note: &'static str, content: impl IntoElement) -> Div {
    column()
        .debug_selector(move || format!("components.{}", title.to_lowercase().replace(' ', "-")))
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

impl ComponentsPage {
    pub fn new(overlays: Rc<RefCell<OverlayHost<Overlay>>>, cx: &mut Context<Self>) -> Self {
        let overlays = PageOverlays::new(overlays, Route::Components);
        let text =
            cx.new(|cx| TextInput::field("Ticket title", false, cx).identified("components.text"));
        let error_text = cx.new(|cx| {
            let mut input =
                TextInput::field("Automation name", false, cx).identified("components.error");
            input.content = "Every 0 minutes".into();
            input
        });
        let search = cx.new(|cx| {
            TextInput::field("Search Tickets", false, cx).identified("components.search")
        });
        let area = cx.new(|cx| {
            let mut input = TextInput::composer(cx).identified("components.area");
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
            overlays,
            text,
            error_text,
            search,
            area,
            toasts,
            started: Instant::now(),
            _subscriptions: subscriptions,
        }
    }

    #[cfg(any(test, feature = "fixtures"))]
    pub fn fixture_section(&mut self, section: usize, cx: &mut Context<Self>) {
        self.section = section.min(SECTIONS.len() - 1);
        self.overlays.close_popover();
        cx.notify();
    }

    #[cfg(any(test, feature = "fixtures"))]
    pub fn fixture_select_open(&mut self, open: bool, cx: &mut Context<Self>) {
        self.overlays.close_popover();
        if open {
            self.overlays.toggle_popover(SELECT_ID);
        }
        cx.notify();
    }

    fn buttons(&self, ui: &mut Ui<Self>) -> Div {
        let noop = |_: &mut Self, _: &mut Window, _: &mut Context<Self>| {};
        column()
            .gap(px(SECTION_GAP))
            .child(specimen(
                "Variants",
                "One white primary per surface. Secondary beside it, ghost for quiet rows, destructive only when something is removed.",
                row().flex_wrap().gap(px(CONTROL_GAP))
                    .child(Button::new("components.primary", "Primary").primary().build(ui, noop))
                    .child(Button::new("components.secondary", "Secondary").secondary().build(ui, noop))
                    .child(Button::new("components.ghost", "Ghost").ghost().build(ui, noop))
                    .child(Button::new("components.destructive", "Delete").destructive().build(ui, noop)),
            ))
            .child(specimen(
                "Sizes and icons",
                "Small for dense rows, regular everywhere else, large for the one action on an empty page.",
                row().flex_wrap().items_center().gap(px(CONTROL_GAP))
                    .child(Button::new("components.small", "Small").secondary().small().icon(Icon::Plus).build(ui, noop))
                    .child(Button::new("components.regular", "New Ticket").primary().icon(Icon::Plus).build(ui, noop))
                    .child(Button::new("components.large", "Get Started").primary().large().build(ui, noop))
                    .child(Button::new("components.icon-ghost", "More").icon(Icon::More).icon_only().ghost().build(ui, noop))
                    .child(Button::new("components.icon-secondary", "Refresh").icon(Icon::Refresh).icon_only().secondary().build(ui, noop))
                    .child(Button::new("components.icon-primary", "Send").icon(Icon::Send).icon_only().primary().build(ui, noop).rounded_full()),
            ))
            .child(specimen(
                "States",
                "Disabled controls keep their layout at reduced opacity. Selected ghosts and secondaries pick up the selected surface.",
                row().flex_wrap().gap(px(CONTROL_GAP))
                    .child(Button::new("components.disabled-primary", "Primary").primary().enabled(false).build(ui, noop))
                    .child(Button::new("components.disabled-secondary", "Secondary").secondary().enabled(false).build(ui, noop))
                    .child(Button::new("components.selected-secondary", "Selected").secondary().selected(true).build(ui, noop))
                    .child(Button::new("components.selected-ghost", "Selected Ghost").ghost().selected(true).build(ui, noop))
                    .child(Button::new("components.trailing", "Search").secondary().icon(Icon::Search).trailing(kbd(shortcuts::GO_TO.glyph)).build(ui, noop)),
            ))
    }

    fn inputs(&self, ui: &mut Ui<Self>) -> Div {
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
                                .build(ui),
                        ),
                    )
                    .child(
                        div().flex_1().child(
                            Field::new(self.error_text.clone())
                                .label("Every (minutes)")
                                .error(Some("Enter an interval in whole minutes."))
                                .build(ui),
                        ),
                    )
                    .child(
                        div().flex_1().child(
                            Field::new(self.search.clone())
                                .label("Search")
                                .leading_icon(Icon::Search)
                                .build(ui),
                        ),
                    ),
            ))
            .child(specimen(
                "Text area",
                "Multiline fields grow with their content and start top-aligned.",
                Field::new(self.area.clone())
                    .label("Instructions")
                    .multiline()
                    .build(ui),
            ))
            .child(specimen(
                "Select",
                "The menu floats above the page, so opening it never resizes its row.",
                row()
                    .gap(px(SPACE_4))
                    .items_center()
                    .child(
                        Select::new(
                            SELECT_ID,
                            vec![
                                SelectOption::new("Codex Default"),
                                SelectOption::new("Codex One").description("Fast"),
                                SelectOption::new("Codex Two").description("Careful"),
                            ],
                        )
                        .value(self.select_value)
                        .open(self.overlays.popover_open(SELECT_ID))
                        .width(SELECT_WIDTH)
                        .build(
                            ui,
                            |this, _, cx| {
                                this.overlays.toggle_popover(SELECT_ID);
                                cx.notify();
                            },
                            |this, index, _, cx| {
                                this.select_value = Some(index);
                                this.overlays.close_popover();
                                cx.notify();
                            },
                        ),
                    )
                    .child(
                        Select::new(
                            "components.select-empty",
                            vec![SelectOption::new("Evee"), SelectOption::new("Scout")],
                        )
                        .placeholder("Choose an agent")
                        .width(200.)
                        .build(ui, |_, _, _| {}, |_, _, _, _| {}),
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
                            .child(
                                Toggle::new("components.toggle", "Automatic checks")
                                    .on(self.toggle_on)
                                    .build(ui, |this, _, cx| {
                                        this.toggle_on = !this.toggle_on;
                                        cx.notify();
                                    }),
                            )
                            .child(
                                div()
                                    .text_size(type_size(LABEL_SIZE))
                                    .child("Automatic checks"),
                            ),
                    )
                    .child(
                        row()
                            .gap(px(SPACE_3))
                            .child(
                                Toggle::new("components.toggle-off", "Off").build(ui, |_, _, _| {}),
                            )
                            .child(div().text_size(type_size(LABEL_SIZE)).child("Off")),
                    )
                    .child(
                        Checkbox::new("components.checkbox", "Include done Tickets")
                            .checked(self.checked)
                            .build(ui, |this, _, cx| {
                                this.checked = !this.checked;
                                cx.notify();
                            }),
                    )
                    .child(
                        Checkbox::new("components.checkbox-disabled", "Disabled")
                            .enabled(false)
                            .build(ui, |_, _, _| {}),
                    ),
            ))
            .child(specimen(
                "Segmented control and chips",
                "Segments switch views or scales; chips pick one value from a short set.",
                column()
                    .gap(px(SPACE_3))
                    .child(
                        Segmented::new(
                            "components.segmented",
                            ["Small", "Default", "Large", "Larger"],
                        )
                        .selected(self.segment)
                        .build(ui, |this, index, _, cx| {
                            this.segment = index;
                            cx.notify();
                        }),
                    )
                    .child(
                        row().flex_wrap().gap(px(CHIP_GAP)).children(
                            ["Backlog", "To do", "In progress", "Done"]
                                .into_iter()
                                .enumerate()
                                .map(|(index, label)| {
                                    Chip::new(
                                        ElementId::NamedInteger(
                                            "components.chip".into(),
                                            index as u64,
                                        ),
                                        label,
                                    )
                                    .selected(index == self.chip)
                                    .build(
                                        ui,
                                        move |this, _, cx| {
                                            this.chip = index;
                                            cx.notify();
                                        },
                                    )
                                }),
                        ),
                    ),
            ))
    }

    fn data(&self, ui: &mut Ui<Self>) -> Div {
        let columns = columns();
        let rows = [
            ("Reconcile weekly budget", "Running", Tone::Info, "2m"),
            ("Morning review", "Completed", Tone::Success, "1h"),
            ("House check", "Timed out", Tone::Danger, "1d"),
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
                    ].into_iter().map(|(label, tone)| status_pill(label, tone))))
                    .child(row().flex_wrap().gap(px(SPACE_2)).children(
                        ["Home", "Money", "Travel", "Health"].into_iter().enumerate()
                            .map(|(index, label)| tag(label, LABEL_COLORS[index])),
                    )),
            ))
            .child(specimen(
                "Avatars and shortcuts",
                "Initials stand in for a photo. Shortcut hints sit inside buttons, rows and footers.",
                row().gap(px(SPACE_6)).items_center()
                    .child(row().gap(px(SPACE_2)).child(avatar("Calum Webb", None, AVATAR_SIZE)).child(avatar("Evee", None, AVATAR_SIZE_LG)).child(agent_avatar("Scout", AVATAR_SIZE)))
                    .child(row().gap(px(SPACE_2)).child(kbd(shortcuts::GO_TO.glyph)).child(kbd(shortcuts::SEND.glyph)).child(kbd(shortcuts::DISMISS.glyph)))
                    .child(kbd_hint("↑ ↓", "Navigate")),
            ))
            .child(specimen(
                "Properties",
                "A fixed label column beside a value or a quiet select, as in a record's side panel.",
                column()
                    .w(px(PROPERTIES_WIDTH))
                    .child(property_row("Status", caption("In progress")))
                    .child(property_row("Labels", row().flex_wrap().gap(px(CHIP_GAP)).child(tag("Money", LABEL_COLORS[1])).child(tag("Admin", LABEL_COLORS[3])))),
            ))
            .child(specimen(
                "List rows",
                "Title, subtitle and a trailing element. Rows fade to their hover surface.",
                column().gap(px(SPACE_HALF))
                    .child(ListRow::new("components.row.1", "Reconcile weekly budget and receipts").leading(status_dot(Tone::Info)).subtitle("Evee").trailing(badge("Running", Tone::Info)).build(ui, |_, _, _| {}))
                    .child(ListRow::new("components.row.2", "Plan the week").leading(status_dot(Tone::Neutral)).subtitle("Unassigned").trailing(icon(Icon::ChevronRight, ICON_SIZE_SM)).selected(true).build(ui, |_, _, _| {})),
            ))
            .child(specimen(
                "Table",
                "An eyebrow header, aligned cells and clickable rows.",
                table_container()
                    .child(table_header(&columns))
                    .children(rows.into_iter().enumerate().map(|(index, (name, status, tone, when))| {
                        TableRow::new(
                            ElementId::NamedInteger("components.table".into(), index as u64),
                            name,
                            &columns,
                            vec![
                                div().text_color(rgb(TEXT)).child(name).into_any_element(),
                                row().child(status_pill(status, tone)).into_any_element(),
                                caption(when).into_any_element(),
                            ],
                        )
                        .build(ui, |_, _, _| {})
                    })),
            ))
            .child(specimen(
                "Empty and loading",
                "Empty states offer the one next action. Skeletons hold the layout while data loads.",
                column().gap(px(SPACE_4))
                    .child(EmptyState::new(Icon::Tasks, "No Tickets yet.").description("Create a Ticket and assign it to an agent to start work.").action(Button::new("components.empty-action", "New Ticket").primary().icon(Icon::Plus).build(ui, |_, _, _| {})).build())
                    .child(skeleton_rows("components.skeleton", SKELETON_ROWS))
                    .child(row().gap(px(SPACE_6)).child(LoadingFrame::new(self.started, ui.window).inline("Evee is thinking…")))
                    .child(LoadingFrame::new(self.started, ui.window).page("Loading Tickets…")),
            ))
    }

    fn overlays(&self, ui: &mut Ui<Self>) -> Div {
        let noop = |_: &mut Self, _: &mut Window, _: &mut Context<Self>| {};
        column()
            .gap(px(SECTION_GAP))
            .child(specimen(
                "Dialog",
                "Centered on a scrim with a title, body and right-aligned actions.",
                row().justify_center().child(dialog_shell(
                    copy::confirm_delete("Plan the week", "This Ticket, its Comments and its relationships").0,
                    caption(copy::confirm_delete("Plan the week", "This Ticket, its Comments and its relationships").1),
                    row_gap(CONTROL_GAP).justify_end()
                        .child(Button::new("components.dialog-cancel", "Cancel").secondary().build(ui, noop))
                        .child(Button::new("components.dialog-confirm", "Delete").destructive().build(ui, noop)),
                )),
            ))
            .child(specimen(
                "Sheet",
                "A full-height panel from the right edge for editing without leaving the page.",
                div().relative().h(px(300.)).w_full().child(
                    div().absolute().top_0().right_0().bottom_0().child(sheet_shell(
                        "Edit Automation",
                        column().gap(px(SPACE_3)).child(caption("Sheets keep the page visible beside them.")),
                        row_gap(CONTROL_GAP).justify_end()
                            .child(Button::new("components.sheet-cancel", "Cancel").secondary().build(ui, noop))
                            .child(Button::new("components.sheet-save", "Save").primary().build(ui, noop)),
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
                        .child(MenuEntry::new("components.menu.updates", "Check for Updates").icon(Icon::Refresh).build(ui, noop))
                        .child(MenuEntry::new("components.menu.settings", "Settings").icon(Icon::Settings).shortcut(shortcuts::SETTINGS.glyph).build(ui, noop))
                        .child(MenuEntry::new("components.menu.support", "Support").icon(Icon::Help).trailing(icon(Icon::ChevronRight, ICON_SIZE_SM)).build(ui, noop))
                        .child(menu_divider())
                        .child(MenuEntry::new("components.menu.delete", "Delete").icon(Icon::Trash).destructive().build(ui, noop)))
                    .child(MenuButton::new("components.menu-button", "Priority").icon(Icon::Filter).active(true).build(ui, vec![], noop))
                    .child(menu_shell(MENU_WIDTH)
                        .child(menu_label("Model"))
                        .child(MenuEntry::new("components.menu.default", "Codex Default").checked(true).build(ui, noop))
                        .child(MenuEntry::new("components.menu.one", "Codex One").build(ui, noop))
                        .child(MenuEntry::new("components.menu.two", "Codex Two").enabled(false).build(ui, noop))),
            ))
            .child(specimen(
                "Toasts",
                "Transient notices stack above the status bar and slide in; sticky ones wait to be dismissed.",
                column().gap(px(SPACE_3))
                    .child(row().child(Button::new("components.toast", "Show a Toast").secondary().build(ui, |this, _, cx| {
                            let id = this.toasts.push("Saved", Some("Your changes are on the daemon.".into()), Tone::Info);
                            Toasts::dismiss_later(id, |this: &mut Self| &mut this.toasts, cx);
                            cx.notify();
                        })))
                    .child(div().relative().w_full().h(px(200.)).child(
                        self.toasts.stack(SPACE_4, SPACE_4).build(ui, |this, id, _, cx| {
                            this.toasts.dismiss(id);
                            cx.notify();
                        }),
                    )),
            ))
    }
}

impl EventEmitter<Destination> for ComponentsPage {}
impl Page for ComponentsPage {
    const ROUTE: Route = Route::Components;
}

impl Render for ComponentsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ui = &mut Ui::new(window, cx);
        let body = match self.section {
            0 => self.buttons(ui),
            1 => self.inputs(ui),
            2 => self.data(ui),
            _ => self.overlays(ui),
        };
        PageFrame::document(PageHeader::new("Components").description(
            "The AgentInc design system, rendered by the app itself. See docs/design-system.md.",
        ))
        .child(
            column()
                .id("components.page")
                .gap(px(SECTION_GAP))
                .child(
                    Tabs::new("components.tabs", SECTIONS)
                        .selected(self.section)
                        .build(ui, |this, index, _, cx| {
                            this.section = index;
                            cx.notify();
                        }),
                )
                .child(body),
        )
        .build()
    }
}
