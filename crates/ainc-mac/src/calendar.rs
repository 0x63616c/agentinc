//! Calendar: Events as a list, a week or a month, and the dialog that creates one.
mod grid;

use crate::{
    action::{Pending, Run},
    daemon::Daemon,
    input::TextInput,
    overlay::Overlay,
    page::{Drafts, Page, PageOverlays},
    routes::{Destination, Route},
    sync::{SliceChanged, Sync},
    ui::*,
};
use ainc_client::types::{CalendarCommand, CalendarEvent, CalendarSnapshot};
use chrono::{Datelike, Local, NaiveDate, NaiveDateTime, TimeZone, Timelike};
use gpui::{prelude::*, *};
use std::{cell::RefCell, rc::Rc, sync::Arc};

/// How a start or end is typed and shown in the form.
const FORM_TIME: &str = "%Y-%m-%d %H:%M";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Dialog {
    Add,
}

/// How the Events are laid out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum View {
    List,
    Week,
    Month,
}

impl View {
    const ALL: [View; 3] = [View::List, View::Week, View::Month];
    fn label(self) -> &'static str {
        match self {
            View::List => "List",
            View::Week => "Week",
            View::Month => "Month",
        }
    }
}

/// How tall a week's day columns are before their Events need more.
const WEEK_MIN_HEIGHT: f32 = 360.;
/// A month cell's height; it shows this many Events and counts the rest.
const MONTH_CELL_HEIGHT: f32 = 96.;
const MONTH_CELL_EVENTS: usize = 3;
/// The size of the disc behind today's number.
const TODAY_DISC: f32 = 22.;

pub struct CalendarPage {
    daemon: Arc<Daemon>,
    sync: Entity<Sync>,
    overlays: PageOverlays<Dialog>,
    state: Arc<CalendarSnapshot>,
    view: View,
    /// The day the week or month view is showing.
    anchor: NaiveDate,
    title: Entity<TextInput>,
    starts_at: Entity<TextInput>,
    ends_at: Entity<TextInput>,
    form_error: Option<String>,
    pending: Pending,
    cancel_focus: FocusHandle,
    submit_focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<Destination> for CalendarPage {}

fn parse_time(text: &str) -> Option<i64> {
    let naive = NaiveDateTime::parse_from_str(text.trim(), FORM_TIME).ok()?;
    Local
        .from_local_datetime(&naive)
        .earliest()
        .map(|t| t.timestamp())
}

impl CalendarPage {
    pub fn new(
        daemon: Arc<Daemon>,
        sync: Entity<Sync>,
        overlays: Rc<RefCell<OverlayHost<Overlay>>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let field = |placeholder: &str, id: &'static str, cx: &mut Context<Self>| {
            cx.new(|cx| TextInput::field(placeholder, false, cx).identified(id))
        };
        let title = field("Title", "calendar.title", cx);
        let starts_at = field("2026-10-12 14:00", "calendar.starts", cx);
        let ends_at = field("2026-10-12 15:00", "calendar.ends", cx);
        let subscriptions = vec![
            cx.observe(&title, |_, _, cx| cx.notify()),
            cx.observe(&starts_at, |_, _, cx| cx.notify()),
            cx.observe(&ends_at, |_, _, cx| cx.notify()),
            cx.observe(&sync, |_, _, cx| cx.notify()),
            cx.subscribe(&sync, |this, _, event: &SliceChanged, cx| {
                if *event == SliceChanged::Calendar {
                    this.reload();
                    cx.notify();
                }
            }),
        ];
        Self {
            state: daemon.calendar(),
            daemon,
            sync,
            overlays: PageOverlays::new(overlays, Route::Calendar),
            view: View::Week,
            anchor: grid::today(),
            title,
            starts_at,
            ends_at,
            form_error: None,
            pending: Pending::default(),
            cancel_focus: cx.focus_handle(),
            submit_focus: cx.focus_handle(),
            _subscriptions: subscriptions,
        }
    }
    pub(crate) fn reload(&mut self) {
        self.state = self.daemon.calendar();
    }
    fn inputs(&self) -> [(&'static str, &Entity<TextInput>); 3] {
        [
            ("title", &self.title),
            ("starts_at", &self.starts_at),
            ("ends_at", &self.ends_at),
        ]
    }
    pub fn open_add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.form_error = None;
        let next_hour = Local::now()
            .with_minute(0)
            .and_then(|t| t.with_second(0))
            .unwrap_or_else(Local::now)
            + chrono::Duration::hours(1);
        let start = next_hour.format(FORM_TIME).to_string();
        let end = (next_hour + chrono::Duration::hours(1))
            .format(FORM_TIME)
            .to_string();
        self.title.update(cx, |input, cx| {
            input.reset();
            cx.notify();
        });
        self.starts_at
            .update(cx, |input, cx| input.set_text(&start, cx));
        self.ends_at
            .update(cx, |input, cx| input.set_text(&end, cx));
        let focus = self.title.focus_handle(cx);
        self.overlays.open_dialog(Dialog::Add, focus, window, cx);
        cx.notify();
    }
    fn form(&self, cx: &App) -> Result<CalendarCommand, String> {
        let title = self.title.read(cx).content.trim().to_owned();
        if title.is_empty() {
            return Err("Give the Event a title.".into());
        }
        let starts_at = parse_time(&self.starts_at.read(cx).content)
            .ok_or("Type the start as year-month-day hour:minute, like 2026-10-12 14:00.")?;
        let ends_at = parse_time(&self.ends_at.read(cx).content)
            .ok_or("Type the end as year-month-day hour:minute, like 2026-10-12 15:00.")?;
        if ends_at <= starts_at {
            return Err("The Event has to end after it starts.".into());
        }
        Ok(CalendarCommand::Create {
            title,
            starts_at,
            ends_at,
        })
    }
    fn send(&mut self, command: CalendarCommand, cx: &mut Context<Self>) {
        let daemon = self.daemon.clone();
        cx.run(
            &self.pending.clone(),
            move || {
                daemon.send(command)?;
                Ok(())
            },
            |this, result, _| {
                this.reload();
                match result {
                    Ok(()) => {
                        this.overlays.close();
                        this.form_error = None;
                    }
                    Err(failure) => this.form_error = Some(failure.message("The Event")),
                }
            },
        );
    }
    fn create(&mut self, cx: &mut Context<Self>) {
        match self.form(cx) {
            Ok(command) => self.send(command, cx),
            Err(message) => {
                self.form_error = Some(message);
                cx.notify();
            }
        }
    }
    fn add_button(&self, id: &'static str, secondary: bool, ui: &mut Ui<Self>) -> Stateful<Div> {
        let button = Button::new(id, "New Event")
            .icon(Icon::Plus)
            .enabled(!self.pending.busy());
        if secondary {
            button.secondary()
        } else {
            button.primary()
        }
        .build(ui, Self::open_add)
    }
    fn event_row(
        &self,
        index: usize,
        count: usize,
        event: &CalendarEvent,
        now: i64,
        ui: &mut Ui<Self>,
    ) -> Div {
        let id = event.id;
        let live = event.starts_at <= now && now < event.ends_at;
        let past = event.ends_at <= now;
        row()
            .group(SharedString::from(format!("event.{id}")))
            .min_h(px(LIST_ROW_HEIGHT))
            .px(px(SPACE_3))
            .py(px(SPACE_2))
            .gap(px(SPACE_3))
            .items_center()
            .when(index + 1 < count, |s| {
                s.border_b_1().border_color(rgb(theme().border_subtle))
            })
            .when(past, |s| s.opacity(0.6))
            .child(
                div()
                    .w(px(2.))
                    .h(px(SPACE_6))
                    .rounded_full()
                    .bg(rgb(if live {
                        theme().primary
                    } else {
                        theme().border_strong
                    })),
            )
            .child(
                div()
                    .w(px(LIST_UPDATED_WIDTH + SPACE_6))
                    .flex_shrink_0()
                    .text_size(type_size(LABEL_SIZE))
                    .text_color(rgb(theme().text_secondary))
                    .child(time::span(event.starts_at, event.ends_at)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(type_size(BODY_SIZE))
                    .child(event.title.clone()),
            )
            .when(live, |s| s.child(badge("Now", Tone::Accent)))
            .child(
                div()
                    .opacity(0.)
                    .group_hover(SharedString::from(format!("event.{id}")), |s| s.opacity(1.))
                    .child(
                        Button::new(SharedString::from(format!("event.{id}.delete")), "Delete")
                            .icon(Icon::Trash)
                            .icon_only()
                            .ghost()
                            .small()
                            .enabled(!self.pending.busy())
                            .build(ui, move |this, _, cx| {
                                this.send(CalendarCommand::Delete { id }, cx)
                            }),
                    ),
            )
    }
}

impl CalendarPage {
    fn set_view(&mut self, view: View, cx: &mut Context<Self>) {
        self.view = view;
        cx.notify();
    }
    fn step(&mut self, delta: i32, cx: &mut Context<Self>) {
        self.anchor = match self.view {
            View::List => self.anchor,
            View::Week => grid::step_week(self.anchor, delta),
            View::Month => grid::step_month(self.anchor, delta),
        };
        cx.notify();
    }
    /// The view switcher and, for the week and month, where they are and how to move.
    fn toolbar(&self, today: NaiveDate, ui: &mut Ui<Self>) -> Div {
        let selected = View::ALL
            .iter()
            .position(|v| *v == self.view)
            .unwrap_or_default();
        let label = match self.view {
            View::List => None,
            View::Week => {
                let week = grid::week_of(self.anchor);
                Some(time::week_label(week[0], week[6]))
            }
            View::Month => Some(time::month_label(self.anchor)),
        };
        let at_today = match self.view {
            View::List => true,
            View::Week => grid::week_of(self.anchor).contains(&today),
            View::Month => {
                (self.anchor.year(), self.anchor.month()) == (today.year(), today.month())
            }
        };
        row()
            .items_center()
            .gap(px(CONTROL_GAP))
            .child(
                Segmented::new("calendar.view", View::ALL.map(View::label))
                    .selected(selected)
                    .build(ui, |this, index, _, cx| this.set_view(View::ALL[index], cx)),
            )
            .child(div().flex_1())
            .when_some(label, |s, label| {
                s.child(
                    Button::new("calendar.today", "Today")
                        .secondary()
                        .small()
                        .enabled(!at_today)
                        .build(ui, |this, _, cx| {
                            this.anchor = grid::today();
                            cx.notify();
                        }),
                )
                .child(
                    Button::new("calendar.previous", "Previous")
                        .icon(Icon::ChevronLeft)
                        .icon_only()
                        .ghost()
                        .small()
                        .build(ui, |this, _, cx| this.step(-1, cx)),
                )
                .child(
                    div()
                        .debug_selector(|| "calendar.range".into())
                        .min_w(px(WORK_STARTED_WIDTH))
                        .text_align(TextAlign::Center)
                        .text_size(type_size(BODY_SIZE))
                        .font_weight(FontWeight::MEDIUM)
                        .child(label),
                )
                .child(
                    Button::new("calendar.next", "Next")
                        .icon(Icon::ChevronRight)
                        .icon_only()
                        .ghost()
                        .small()
                        .build(ui, |this, _, cx| this.step(1, cx)),
                )
            })
    }
    /// A day's number, on a disc when it is today.
    fn day_number(day: NaiveDate, today: NaiveDate, muted: bool) -> Div {
        let is_today = day == today;
        row()
            .items_center()
            .justify_center()
            .size(px(TODAY_DISC))
            .rounded_full()
            .text_size(type_size(LABEL_SIZE))
            .font_weight(FontWeight::MEDIUM)
            .when(is_today, |s| {
                s.bg(rgb(theme().primary))
                    .text_color(rgb(theme().text_on_primary))
            })
            .when(!is_today && muted, |s| {
                s.text_color(rgb(theme().text_tertiary))
            })
            .child(day.day().to_string())
    }
    /// An Event inside a day cell: its start and title on a soft tile.
    fn chip(event: &CalendarEvent, now: i64, with_time: bool) -> Div {
        let live = event.starts_at <= now && now < event.ends_at;
        let past = event.ends_at <= now;
        column()
            .w_full()
            .px(px(SPACE_2))
            .py(px(SPACE_1))
            .rounded(px(RADIUS_SM))
            .bg(rgb(theme().surface_control))
            .border_l_2()
            .border_color(rgb(if live {
                theme().primary
            } else {
                theme().border_strong
            }))
            .when(past, |s| s.opacity(0.6))
            .when(with_time, |s| s.child(hint(time::clock(event.starts_at))))
            .child(
                div()
                    .text_size(type_size(LABEL_SIZE))
                    .truncate()
                    .child(event.title.clone()),
            )
    }
    fn week(&self, today: NaiveDate, now: i64) -> Div {
        let week = grid::week_of(self.anchor);
        let events = &self.state.events;
        card()
            .debug_selector(|| "calendar.week".into())
            .p_0()
            .gap_0()
            .overflow_hidden()
            .child(
                row()
                    .border_b_1()
                    .border_color(rgb(theme().border_subtle))
                    .children(week.iter().enumerate().map(|(index, day)| {
                        column()
                            .flex_1()
                            .min_w_0()
                            .items_center()
                            .gap(px(SPACE_1))
                            .py(px(SPACE_2))
                            .when(index > 0, |s| {
                                s.border_l_1().border_color(rgb(theme().border_subtle))
                            })
                            .child(eyebrow(time::weekday(*day)))
                            .child(Self::day_number(*day, today, grid::is_weekend(*day)))
                    })),
            )
            .child(row().items_stretch().min_h(px(WEEK_MIN_HEIGHT)).children(
                week.iter().enumerate().map(|(index, day)| {
                    column()
                        .flex_1()
                        .min_w_0()
                        .p(px(SPACE_1))
                        .gap(px(SPACE_1))
                        .when(index > 0, |s| {
                            s.border_l_1().border_color(rgb(theme().border_subtle))
                        })
                        .when(*day == today, |s| s.bg(rgb(theme().hover)))
                        .children(
                            grid::on_day(events, *day)
                                .into_iter()
                                .map(|event| Self::chip(event, now, true)),
                        )
                }),
            ))
    }
    fn month(&self, today: NaiveDate, now: i64) -> Div {
        let weeks = grid::month_of(self.anchor);
        let month = self.anchor.month();
        let events = &self.state.events;
        card()
            .debug_selector(|| "calendar.month".into())
            .p_0()
            .gap_0()
            .overflow_hidden()
            .child(
                row()
                    .border_b_1()
                    .border_color(rgb(theme().border_subtle))
                    .children(weeks[0].iter().map(|day| {
                        div()
                            .flex_1()
                            .min_w_0()
                            .py(px(SPACE_2))
                            .px(px(SPACE_2))
                            .child(eyebrow(time::weekday(*day)))
                    })),
            )
            .children(weeks.iter().enumerate().map(|(row_index, week)| {
                row()
                    .items_stretch()
                    .when(row_index > 0, |s| {
                        s.border_t_1().border_color(rgb(theme().border_subtle))
                    })
                    .children(week.iter().enumerate().map(|(index, day)| {
                        let on_day = grid::on_day(events, *day);
                        let more = on_day.len().saturating_sub(MONTH_CELL_EVENTS);
                        let outside = day.month() != month;
                        column()
                            .flex_1()
                            .min_w_0()
                            .min_h(px(MONTH_CELL_HEIGHT))
                            .p(px(SPACE_1))
                            .gap(px(SPACE_HALF))
                            .when(index > 0, |s| {
                                s.border_l_1().border_color(rgb(theme().border_subtle))
                            })
                            .when(*day == today, |s| s.bg(rgb(theme().hover)))
                            .when(outside, |s| s.opacity(0.5))
                            .child(Self::day_number(
                                *day,
                                today,
                                outside || grid::is_weekend(*day),
                            ))
                            .children(
                                on_day
                                    .into_iter()
                                    .take(MONTH_CELL_EVENTS)
                                    .map(|event| Self::chip(event, now, false)),
                            )
                            .when(more > 0, |s| {
                                s.child(div().px(px(SPACE_2)).child(hint(format!("+{more} more"))))
                            })
                    }))
            }))
    }
    fn list(&self, days: Vec<(String, Vec<&CalendarEvent>)>, now: i64, ui: &mut Ui<Self>) -> Div {
        column()
            .gap(px(SECTION_GAP))
            .children(days.into_iter().map(|(label, events)| {
                let count = events.len();
                column().gap(px(SPACE_2)).child(eyebrow(label)).child(
                    card().p(px(SPACE_1)).gap_0().children(
                        events
                            .into_iter()
                            .enumerate()
                            .map(|(index, event)| self.event_row(index, count, event, now, ui)),
                    ),
                )
            }))
    }
}

impl Page for CalendarPage {
    const ROUTE: Route = Route::Calendar;
    fn overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.overlays.active()?;
        let ui = &mut Ui::new(window, cx);
        let body = column_gap(FORM_STACK_GAP)
            .child(text_field("Title", self.title.clone(), ui))
            .child(
                row_gap(SPACE_3)
                    .child(
                        div()
                            .flex_1()
                            .child(Field::new(self.starts_at.clone()).label("Starts").build(ui)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .child(Field::new(self.ends_at.clone()).label("Ends").build(ui)),
                    ),
            )
            .child(hint("Local time, as year-month-day hour:minute."))
            .when_some(self.form_error.clone(), |s, error| {
                s.child(error_text(error))
            });
        let footer = DialogFooter::new(Verb::Create)
            .ids("calendar.cancel", "calendar.submit")
            .enabled(!self.title.read(ui.cx).content.trim().is_empty())
            .pending(self.pending.busy())
            .focus(&self.cancel_focus, &self.submit_focus)
            .build(
                ui,
                |this: &mut Self, window, cx| {
                    this.overlays.dismiss(window, cx);
                    cx.notify();
                },
                |this: &mut Self, _, cx| this.create(cx),
            );
        Some(dialog_shell("New Event", body, footer).into_any_element())
    }
    fn focus_handles(&self, cx: &App) -> Vec<FocusHandle> {
        let mut handles: Vec<FocusHandle> = self
            .inputs()
            .into_iter()
            .map(|(_, input)| input.focus_handle(cx))
            .collect();
        handles.extend([self.cancel_focus.clone(), self.submit_focus.clone()]);
        handles
    }
    fn drafts(&self, cx: &App) -> anyhow::Result<Drafts> {
        anyhow::ensure!(
            !self.pending.busy(),
            "Wait for the current change to finish before installing"
        );
        let mut drafts = Drafts::default();
        for (key, input) in self.inputs() {
            drafts.text(key, input, cx);
        }
        Ok(drafts)
    }
    fn restore(&mut self, drafts: Drafts, cx: &mut Context<Self>) {
        for (key, input) in self.inputs() {
            drafts.restore_text(key, input, cx);
        }
    }
}

impl Render for CalendarPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sync = self.sync.read(cx);
        let state = sync.load_state();
        let fetching = sync.fetching();
        let snapshot = self.state.clone();
        let now = time::now();
        let today = grid::today();
        // Days in order, each with its Events (already sorted by start).
        let mut days: Vec<(String, Vec<&CalendarEvent>)> = Vec::new();
        for event in snapshot
            .events
            .iter()
            .filter(|e| e.ends_at > now - 24 * 3600)
        {
            let label = time::day(event.starts_at, now);
            match days.last_mut() {
                Some((last, events)) if *last == label => events.push(event),
                _ => days.push((label, vec![event])),
            }
        }
        let ui = &mut Ui::new(window, cx);
        PageFrame::document(
            PageHeader::new(self.title()).actions(
                row_gap(CONTROL_GAP)
                    .child(
                        Button::new("calendar.refresh", "Refresh")
                            .icon(Icon::Refresh)
                            .icon_only()
                            .secondary()
                            .enabled(!fetching)
                            .build(ui, |this, _, cx| {
                                this.sync.update(cx, |sync, cx| sync.wake(cx))
                            }),
                    )
                    .child(self.add_button("calendar.create", false, ui)),
            ),
        )
        .child(column().gap(px(SPACE_4)).children(page_frame(
            "calendar",
            &state,
            SKELETON_ROWS,
            ui,
            |ui| {
                let body = match self.view {
                    View::List if days.is_empty() => match state.error {
                        Some(_) => div().into_any_element(),
                        None => EmptyState::new(Icon::Calendar, "Nothing coming up.")
                            .description("Add an Event and it shows here under its day.")
                            .selector("calendar.empty")
                            .action(self.add_button("calendar.create.empty", true, ui))
                            .build()
                            .into_any_element(),
                    },
                    View::List => self.list(days, now, ui).into_any_element(),
                    View::Week => self.week(today, now).into_any_element(),
                    View::Month => self.month(today, now).into_any_element(),
                };
                column()
                    .gap(px(SPACE_4))
                    .child(self.toolbar(today, ui))
                    .child(body)
                    .into_any_element()
            },
        )))
        .build()
    }
}

#[cfg(test)]
mod tests {
    use super::{FORM_TIME, parse_time};
    use chrono::{DateTime, Local};

    #[test]
    fn form_times_round_trip_in_local_time() {
        let seconds = parse_time("2026-10-12 14:30").unwrap();
        assert_eq!(
            DateTime::from_timestamp(seconds, 0)
                .unwrap()
                .with_timezone(&Local)
                .format(FORM_TIME)
                .to_string(),
            "2026-10-12 14:30"
        );
        assert!(parse_time("tomorrow").is_none());
    }
}
