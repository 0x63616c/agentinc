//! The Calendar page: an agenda, a week on a time grid and a month with its
//! day beside it, plus the event editor.
use crate::{
    calendar::{
        CalendarModel, WEEKDAYS, ago, at, date_text, day_label, day_start, lanes, local,
        month_grid, now, on_day, parse_date, parse_time, span, time_text, today, week,
    },
    calendar_store::Access,
    input::TextInput,
    model::Overlay,
    ui::*,
};
use ainc_client::types::{CalendarCommand, CalendarEvent, EventSource};
use chrono::{Datelike, Duration, Months, NaiveDate, NaiveTime, Timelike};
use gpui::{prelude::*, *};
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Agenda,
    Week,
    Month,
}
const VIEWS: [(View, &str); 3] = [
    (View::Agenda, "Agenda"),
    (View::Week, "Week"),
    (View::Month, "Month"),
];
/// The week grid's first and last hour.
const FIRST_HOUR: u32 = 6;
const LAST_HOUR: u32 = 24;
/// New events start inside waking hours.
const DAY_STARTS: u32 = 9;
const DAY_ENDS: u32 = 20;
/// How many days the agenda reads ahead.
const AGENDA_DAYS: i64 = 42;

/// A valid event read from the editor's fields.
struct Draft {
    title: String,
    starts_at: i64,
    ends_at: i64,
    all_day: bool,
    location: Option<String>,
    notes: Option<String>,
}

/// What the editor dialog is showing.
#[derive(Clone, Debug, PartialEq)]
enum Editing {
    New,
    Own { id: String, revision: i64 },
    Mirror(String),
}

/// An event's color, softened so calendar colors never outshout the text.
pub fn event_color(event: &CalendarEvent) -> Rgba {
    let parsed = event
        .color
        .as_deref()
        .and_then(|c| u32::from_str_radix(c.trim_start_matches('#'), 16).ok());
    match (event.source, parsed) {
        (EventSource::Agentinc, _) => rgb(PRIMARY),
        (_, Some(color)) => blend(color, STATUS_NEUTRAL, 0.45),
        (_, None) => rgb(STATUS_NEUTRAL),
    }
}

pub struct CalendarPage {
    calendar: Entity<CalendarModel>,
    overlays: Rc<RefCell<OverlayHost<Overlay>>>,
    view: View,
    anchor: NaiveDate,
    selected: NaiveDate,
    editing: Option<Editing>,
    title: Entity<TextInput>,
    date: Entity<TextInput>,
    start: Entity<TextInput>,
    end: Entity<TextInput>,
    location: Entity<TextInput>,
    notes: Entity<TextInput>,
    all_day: bool,
    form_error: Option<String>,
    pending: bool,
    cancel_focus: FocusHandle,
    submit_focus: FocusHandle,
    hover: HoverFade,
    _subscriptions: Vec<Subscription>,
}
impl HoverHost for CalendarPage {
    fn hover_fade(&mut self) -> &mut HoverFade {
        &mut self.hover
    }
}

impl CalendarPage {
    pub fn new(
        calendar: Entity<CalendarModel>,
        overlays: Rc<RefCell<OverlayHost<Overlay>>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = |placeholder: &str, id: &'static str, cx: &mut Context<Self>| {
            cx.new(|cx| TextInput::field(placeholder, false, cx).identified(id))
        };
        let title = input("Title", "calendar.title", cx);
        let date = input("Sep 26, 2026", "calendar.date", cx);
        let start = input("9:00 AM", "calendar.start", cx);
        let end = input("10:00 AM", "calendar.end", cx);
        let location = input("Add a place", "calendar.location", cx);
        let notes = input("Add notes", "calendar.notes", cx);
        let mut subscriptions = vec![cx.observe(&calendar, |_, _, cx| cx.notify())];
        for field in [&title, &date, &start, &end, &location, &notes] {
            subscriptions.push(cx.observe(field, |_, _, cx| cx.notify()));
        }
        let today = today();
        Self {
            calendar,
            overlays,
            view: View::Month,
            anchor: today,
            selected: today,
            editing: None,
            title,
            date,
            start,
            end,
            location,
            notes,
            all_day: false,
            form_error: None,
            pending: false,
            cancel_focus: cx.focus_handle(),
            submit_focus: cx.focus_handle(),
            hover: HoverFade::default(),
            _subscriptions: subscriptions,
        }
    }

    /// The Unix range the current view shows.
    fn range(&self) -> (i64, i64) {
        let (first, last) = match self.view {
            View::Month => {
                let grid = month_grid(self.anchor);
                (grid[0], grid[grid.len() - 1])
            }
            View::Week => {
                let days = week(self.anchor);
                (days[0], days[6])
            }
            View::Agenda => (self.anchor, self.anchor + Duration::days(AGENDA_DAYS)),
        };
        (day_start(first), day_start(last + Duration::days(1)))
    }
    fn go(&mut self, anchor: NaiveDate, cx: &mut Context<Self>) {
        self.anchor = anchor;
        let (from, to) = self.range();
        self.calendar.update(cx, |c, cx| c.cover(from, to, cx));
        cx.notify();
    }
    fn step(&mut self, forward: bool, cx: &mut Context<Self>) {
        let anchor = match (self.view, forward) {
            (View::Month, true) => self
                .anchor
                .with_day(1)
                .and_then(|d| d.checked_add_months(Months::new(1))),
            (View::Month, false) => self
                .anchor
                .with_day(1)
                .and_then(|d| d.checked_sub_months(Months::new(1))),
            (View::Week, true) => Some(self.anchor + Duration::days(7)),
            (View::Week, false) => Some(self.anchor - Duration::days(7)),
            (View::Agenda, true) => Some(self.anchor + Duration::days(14)),
            (View::Agenda, false) => Some(self.anchor - Duration::days(14)),
        };
        if let Some(anchor) = anchor {
            if self.view == View::Month {
                self.selected = anchor;
            }
            self.go(anchor, cx);
        }
    }
    fn period(&self) -> String {
        match self.view {
            View::Month => self.anchor.format("%B %Y").to_string(),
            View::Week => {
                let days = week(self.anchor);
                if days[0].month() == days[6].month() {
                    format!(
                        "{} – {}",
                        days[0].format("%b %-d"),
                        days[6].format("%-d, %Y")
                    )
                } else {
                    format!(
                        "{} – {}",
                        days[0].format("%b %-d"),
                        days[6].format("%b %-d, %Y")
                    )
                }
            }
            View::Agenda => {
                let last = self.anchor + Duration::days(AGENDA_DAYS);
                format!(
                    "{} – {}",
                    self.anchor.format("%b %-d"),
                    last.format("%b %-d, %Y")
                )
            }
        }
    }

    // ----- Editor -------------------------------------------------------

    pub fn open_new(&mut self, date: NaiveDate, window: &mut Window, cx: &mut Context<Self>) {
        let start = if date == today() {
            // The next round hour, kept inside waking hours.
            let next = (local(now()).hour() + 1).clamp(DAY_STARTS, DAY_ENDS);
            NaiveTime::from_hms_opt(next, 0, 0).unwrap_or(NaiveTime::MIN)
        } else {
            NaiveTime::from_hms_opt(DAY_STARTS, 0, 0).unwrap_or(NaiveTime::MIN)
        };
        let starts_at = at(date, start).unwrap_or_else(now);
        self.fill("", date, starts_at, starts_at + 3600, false, "", "", cx);
        self.editing = Some(Editing::New);
        self.open_dialog(window, cx);
    }
    fn open_event(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(event) = self
            .calendar
            .read(cx)
            .events
            .iter()
            .find(|e| e.id == id)
            .cloned()
        else {
            return;
        };
        if event.source == EventSource::Macos {
            self.editing = Some(Editing::Mirror(event.id));
        } else {
            self.fill(
                &event.title,
                local(event.starts_at).date_naive(),
                event.starts_at,
                event.ends_at,
                event.all_day,
                event.location.as_deref().unwrap_or(""),
                event.notes.as_deref().unwrap_or(""),
                cx,
            );
            self.editing = Some(Editing::Own {
                id: event.id,
                revision: event.revision,
            });
        }
        self.open_dialog(window, cx);
    }
    #[allow(clippy::too_many_arguments)]
    fn fill(
        &mut self,
        title: &str,
        date: NaiveDate,
        starts_at: i64,
        ends_at: i64,
        all_day: bool,
        location: &str,
        notes: &str,
        cx: &mut Context<Self>,
    ) {
        let set = |input: &Entity<TextInput>, text: &str, cx: &mut Context<Self>| {
            input.update(cx, |i, cx| i.set_text(text, cx))
        };
        set(&self.title, title, cx);
        set(&self.date, &date_text(date), cx);
        set(&self.start, &time_text(starts_at), cx);
        set(&self.end, &time_text(ends_at), cx);
        set(&self.location, location, cx);
        set(&self.notes, notes, cx);
        self.all_day = all_day;
        self.form_error = None;
        self.pending = false;
    }
    fn open_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let focus = match self.editing {
            Some(Editing::Mirror(_)) => self.submit_focus.clone(),
            _ => self.title.focus_handle(cx),
        };
        self.overlays
            .borrow_mut()
            .open(Overlay::CalendarEvent, window, cx, Some(focus));
        cx.notify();
    }
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.overlays.borrow_mut().dismiss(window, cx);
        self.editing = None;
        cx.notify();
    }
    pub fn focus_handles(&self, cx: &App) -> Vec<FocusHandle> {
        let mut handles = match self.editing {
            Some(Editing::Mirror(_)) | None => vec![],
            _ => {
                let mut fields = vec![self.title.focus_handle(cx), self.date.focus_handle(cx)];
                if !self.all_day {
                    fields.extend([self.start.focus_handle(cx), self.end.focus_handle(cx)]);
                }
                fields.extend([self.location.focus_handle(cx), self.notes.focus_handle(cx)]);
                fields
            }
        };
        handles.extend([self.cancel_focus.clone(), self.submit_focus.clone()]);
        handles
    }
    /// The event the form describes, or why it cannot be saved.
    fn draft(&self, cx: &App) -> Result<Draft, String> {
        let text = |input: &Entity<TextInput>| input.read(cx).content.trim().to_owned();
        let title = text(&self.title);
        if title.is_empty() {
            return Err("Give the event a title.".into());
        }
        let date = parse_date(&text(&self.date), today())
            .ok_or("Use a date such as Sep 26 or 2026-09-26.")?;
        let (starts_at, ends_at) = if self.all_day {
            (day_start(date), day_start(date + Duration::days(1)))
        } else {
            let start =
                parse_time(&text(&self.start)).ok_or("Use a start time such as 3:00 PM.")?;
            let end = parse_time(&text(&self.end)).ok_or("Use an end time such as 4:00 PM.")?;
            let starts_at = at(date, start).ok_or("That time does not exist on this date.")?;
            let mut ends_at = at(date, end).ok_or("That time does not exist on this date.")?;
            if ends_at <= starts_at {
                // An end before the start runs past midnight.
                ends_at += 86_400;
            }
            (starts_at, ends_at)
        };
        let optional = |value: String| (!value.is_empty()).then_some(value);
        Ok(Draft {
            title,
            starts_at,
            ends_at,
            all_day: self.all_day,
            location: optional(text(&self.location)),
            notes: optional(text(&self.notes)),
        })
    }
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editing) = self.editing.clone() else {
            return;
        };
        if let Editing::Mirror(_) = editing {
            self.close(window, cx);
            return;
        }
        let Draft {
            title,
            starts_at,
            ends_at,
            all_day,
            location,
            notes,
        } = match self.draft(cx) {
            Ok(draft) => draft,
            Err(error) => {
                self.form_error = Some(error);
                cx.notify();
                return;
            }
        };
        let command = match editing {
            Editing::Own { id, revision } => CalendarCommand::Update {
                id,
                revision,
                title,
                starts_at,
                ends_at,
                all_day,
                location,
                notes,
            },
            _ => CalendarCommand::Create {
                title,
                starts_at,
                ends_at,
                all_day,
                location,
                notes,
            },
        };
        self.selected = local(starts_at).date_naive();
        self.run(command, window, cx);
    }
    fn delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(Editing::Own { id, revision }) = self.editing.clone() {
            self.run(CalendarCommand::Delete { id, revision }, window, cx);
        }
    }
    fn run(&mut self, command: CalendarCommand, _: &mut Window, cx: &mut Context<Self>) {
        self.pending = true;
        self.form_error = None;
        cx.notify();
        let task = self.calendar.update(cx, |c, cx| c.command(command, cx));
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.pending = false;
                match result {
                    Ok(()) => {
                        this.overlays.borrow_mut().close();
                        this.editing = None;
                    }
                    Err(error) => this.form_error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn overlay(&self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.overlays.borrow().active() != Some(Overlay::CalendarEvent) {
            return None;
        }
        let editing = self.editing.clone()?;
        if let Editing::Mirror(id) = &editing {
            let event = self
                .calendar
                .read(cx)
                .events
                .iter()
                .find(|e| &e.id == id)?
                .clone();
            return Some(self.mirror(&event, cx).into_any_element());
        }
        let field = |input: &Entity<TextInput>,
                     label: &'static str,
                     glyph: Option<&'static str>,
                     window: &mut Window,
                     cx: &mut Context<Self>| {
            let mut field = Field::new(input.clone()).label(label);
            if let Some(glyph) = glyph {
                field = field.leading_icon(glyph);
            }
            div().flex_1().min_w_0().child(field.build(window, cx))
        };
        let times = row()
            .gap(px(CONTROL_GAP))
            .child(field(&self.start, "Starts", Some("clock"), window, cx))
            .child(field(&self.end, "Ends", Some("clock"), window, cx));
        let body = column()
            .gap(px(FORM_STACK_GAP))
            .child(field(&self.title, "Title", None, window, cx))
            .child(
                row()
                    .items_end()
                    .gap(px(SPACE_4))
                    .child(field(&self.date, "Date", Some("calendar"), window, cx))
                    .child(row().h(px(FIELD_HEIGHT)).child(checkbox(
                        "calendar.all-day",
                        "All day",
                        self.all_day,
                        true,
                        |this: &mut Self, _, cx| {
                            this.all_day = !this.all_day;
                            cx.notify();
                        },
                        cx,
                    ))),
            )
            .when(!self.all_day, |s| s.child(times))
            .child(field(
                &self.location,
                "Location",
                Some("mapPin"),
                window,
                cx,
            ))
            .child(
                Field::new(self.notes.clone())
                    .label("Notes")
                    .multiline()
                    .build(window, cx),
            )
            .when_some(self.form_error.clone(), |s, error| {
                s.child(error_text(error))
            });
        let save = Button::new(
            "calendar.save",
            if self.pending {
                "Saving…"
            } else if editing == Editing::New {
                "Create"
            } else {
                "Save"
            },
        )
        .primary()
        .enabled(!self.pending && !self.title.read(cx).content.trim().is_empty())
        .track_focus(&self.submit_focus)
        .build(
            &self.hover,
            |this: &mut Self, window, cx| this.save(window, cx),
            cx,
        );
        let cancel = Button::new("calendar.cancel", "Cancel")
            .secondary()
            .track_focus(&self.cancel_focus)
            .build(
                &self.hover,
                |this: &mut Self, window, cx| this.close(window, cx),
                cx,
            );
        // Delete sits at the far end of the footer, away from Save.
        let footer = row()
            .w_full()
            .gap(px(CONTROL_GAP))
            .when(matches!(editing, Editing::Own { .. }), |s| {
                s.child(
                    Button::new("calendar.delete", "Delete")
                        .secondary()
                        .icon("trash")
                        .tint(DESTRUCTIVE_TEXT)
                        .enabled(!self.pending)
                        .build(
                            &self.hover,
                            |this: &mut Self, window, cx| this.delete(window, cx),
                            cx,
                        ),
                )
            })
            .child(div().flex_1())
            .child(cancel)
            .child(save);
        Some(
            dialog_shell(
                if editing == Editing::New {
                    "New event"
                } else {
                    "Edit event"
                },
                body,
                footer,
            )
            .into_any_element(),
        )
    }
    /// A read-only look at an event mirrored from the Mac's calendars.
    fn mirror(&self, event: &CalendarEvent, cx: &mut Context<Self>) -> Stateful<Div> {
        let date = local(event.starts_at).date_naive();
        let line = |glyph: &'static str, text: String| {
            row()
                .gap(px(SPACE_3))
                .child(icon(glyph, ICON_SIZE_SM))
                .child(div().flex_1().min_w_0().child(text))
        };
        let body = column()
            .gap(px(SPACE_3))
            .text_size(type_size(BODY_SIZE))
            .child(line(
                "clock",
                format!("{} · {}", day_label(date, today()), span(event)),
            ))
            .child(
                row()
                    .gap(px(SPACE_3))
                    .child(
                        row().size(px(ICON_SIZE_SM)).justify_center().child(
                            div()
                                .size(px(SPACE_2))
                                .rounded_full()
                                .bg(event_color(event)),
                        ),
                    )
                    .child(event.calendar.clone()),
            )
            .when_some(event.location.clone(), |s, location| {
                s.child(line("mapPin", location))
            })
            .when_some(event.notes.clone(), |s, notes| {
                s.child(
                    div()
                        .pt(px(SPACE_1))
                        .text_size(type_size(LABEL_SIZE))
                        .text_color(rgb(TEXT_SECONDARY))
                        .child(notes),
                )
            })
            .child(hint(
                "Mirrored from your Mac. Change it in the Calendar app.",
            ));
        dialog_shell(
            event.title.clone(),
            body,
            dialog_footer(
                div(),
                Button::new("calendar.done", "Done")
                    .primary()
                    .track_focus(&self.submit_focus)
                    .build(
                        &self.hover,
                        |this: &mut Self, window, cx| this.close(window, cx),
                        cx,
                    ),
            ),
        )
    }

    // ----- Views ----------------------------------------------------------

    /// One event as a compact line: dot, optional time, title.
    fn chip(&self, event: &CalendarEvent, cx: &mut Context<Self>) -> Stateful<Div> {
        let id = event.id.clone();
        let (progress, on_hover) = self.hover.track(
            &ElementId::from(SharedString::from(format!("calendar.chip.{id}"))),
            true,
            cx,
        );
        action_button(
            ButtonSpec {
                id: SharedString::from(format!("calendar.chip.{id}")).into(),
                label: event.title.clone().into(),
                enabled: true,
            },
            |button| {
                button
                    .w_full()
                    .min_w_0()
                    .h(px(CONTROL_HEIGHT_SM - SPACE_1))
                    .px(px(SPACE_1))
                    .gap(px(SPACE_1 + SPACE_HALF))
                    .rounded(px(RADIUS_XS))
                    .text_size(type_size(CAPTION_SIZE))
                    .on_hover(on_hover)
                    .bg(if event.all_day {
                        blend(SURFACE_CONTROL, HOVER_STRONG, progress)
                    } else {
                        rgba((HOVER_STRONG << 8) | (progress * 255.) as u32)
                    })
                    .child(
                        div()
                            .size(px(SPACE_1 + SPACE_HALF))
                            .flex_shrink_0()
                            .rounded_full()
                            .bg(event_color(event)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(rgb(TEXT))
                            .child(event.title.clone()),
                    )
            },
            move |this: &mut Self, window, cx| this.open_event(&id, window, cx),
            cx,
        )
    }

    /// The date numeral, circled in white for today.
    fn numeral(date: NaiveDate, dim: bool) -> Div {
        let is_today = date == today();
        row()
            .size(px(TODAY_MARK))
            .justify_center()
            .rounded_full()
            .text_size(type_size(LABEL_SIZE))
            .font_weight(if is_today {
                FontWeight::SEMIBOLD
            } else {
                FontWeight::NORMAL
            })
            .when(is_today, |s| {
                s.bg(rgb(PRIMARY)).text_color(rgb(TEXT_ON_PRIMARY))
            })
            .when(!is_today, |s| {
                s.text_color(rgb(if dim { TEXT_TERTIARY } else { TEXT }))
            })
            .child(date.day().to_string())
    }

    fn month(
        &self,
        events: &[CalendarEvent],
        wide: bool,
        cell_height: f32,
        dots: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let grid = month_grid(self.anchor);
        let month = self.anchor.month();
        let mut header = row()
            .w_full()
            .h(px(CONTROL_HEIGHT))
            .border_b_1()
            .border_color(rgb(BORDER_SUBTLE));
        for day in WEEKDAYS {
            header = header.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .px(px(SPACE_2))
                    .child(eyebrow(day.to_string())),
            );
        }
        let mut weeks = column().w_full();
        for (index, days) in grid.chunks(7).enumerate() {
            let mut line = row().w_full().items_stretch().when(index > 0, |s| {
                s.border_t_1().border_color(rgb(BORDER_SUBTLE))
            });
            for (column_index, date) in days.iter().copied().enumerate() {
                let day = on_day(events, date);
                let selected = date == self.selected;
                let id = ElementId::from(SharedString::from(format!("calendar.day.{date}")));
                let (progress, on_hover) = self.hover.track(&id, true, cx);
                let mut cell = column()
                    .w_full()
                    .h_full()
                    .overflow_hidden()
                    .gap(px(SPACE_HALF))
                    .child(Self::numeral(date, date.month() != month));
                let outside = date.month() != month;
                // Only as many lines as the cell holds; the rest become "N more".
                // Narrow columns, or a busy day with room for one line, show one
                // mark per event rather than clipped titles.
                // Lines are separated by gaps, so n lines need n lines less one gap.
                let fits = ((cell_height - TODAY_MARK - SPACE_2 * 2. + SPACE_HALF) / MONTH_LINE)
                    .floor() as usize;
                if dots || (day.len() > fits && fits < 2) {
                    cell = cell.child(
                        row()
                            .flex_wrap()
                            .gap(px(SPACE_1))
                            .px(px(SPACE_1))
                            .pt(px(SPACE_1))
                            .children(day.iter().take(MONTH_DOTS).map(|event| {
                                div()
                                    .size(px(SPACE_1 + SPACE_HALF))
                                    .rounded_full()
                                    .bg(event_color(event))
                            })),
                    );
                } else {
                    let shown = if day.len() > fits {
                        fits - 1
                    } else {
                        day.len()
                    };
                    for event in day.iter().take(shown) {
                        cell = cell.child(self.chip(event, cx).when(outside, |s| s.opacity(0.5)));
                    }
                    if day.len() > shown {
                        cell = cell.child(
                            div()
                                .px(px(SPACE_1))
                                .child(hint(format!("{} more", day.len() - shown))),
                        );
                    }
                }
                line = line.child(action_button(
                    ButtonSpec {
                        id,
                        label: format!("{}, {} events", date.format("%A %B %-d"), day.len()).into(),
                        enabled: true,
                    },
                    |button| {
                        button
                            .flex_1()
                            .min_w_0()
                            .h(px(cell_height))
                            .items_start()
                            .p(px(SPACE_2))
                            .rounded(px(0.))
                            .when(column_index > 0, |s| {
                                s.border_l_1().border_color(rgb(BORDER_SUBTLE))
                            })
                            .on_hover(on_hover)
                            .bg(if selected {
                                blend(SELECTED, HOVER_STRONG, progress)
                            } else {
                                rgba((HOVER << 8) | (progress * 255.) as u32)
                            })
                            .child(cell)
                    },
                    move |this: &mut Self, _, cx| {
                        this.selected = date;
                        cx.notify();
                    },
                    cx,
                ));
            }
            weeks = weeks.child(line);
        }
        let grid = column()
            .debug_selector(|| "calendar.month".into())
            .flex_1()
            .min_w_0()
            .overflow_hidden()
            .rounded(px(RADIUS_LG))
            .border_1()
            .border_color(rgb(BORDER))
            .bg(rgb(SURFACE_RAISED))
            .child(header)
            .child(weeks);
        row()
            .w_full()
            .items_stretch()
            .gap(px(SPACE_4))
            .child(grid)
            // The selected day sits beside the month when there is room for both.
            .when(wide, |s| s.child(self.day_panel(events, cx)))
    }

    /// The selected day beside the month.
    fn day_panel(&self, events: &[CalendarEvent], cx: &mut Context<Self>) -> Div {
        let date = self.selected;
        let day = on_day(events, date);
        // Rows carry their own inset; pull them out so their marks align with the heading.
        let mut list = column().gap(px(SPACE_1)).mx(px(-SPACE_3));
        if day.is_empty() {
            list = list.child(div().px(px(SPACE_3)).child(caption("Nothing scheduled.")));
        }
        for event in day {
            let id = event.id.clone();
            list = list.child(
                ListRow::new(
                    SharedString::from(format!("calendar.panel.{}", event.id)),
                    event.title.clone(),
                )
                .subtitle(match &event.location {
                    Some(location) => format!("{} · {location}", span(event)),
                    None => span(event),
                })
                .leading(
                    div()
                        .w(px(EVENT_BAR_WIDTH))
                        .h(px(SPACE_8))
                        .rounded_full()
                        .bg(event_color(event)),
                )
                .build(
                    &self.hover,
                    move |this: &mut Self, window, cx| this.open_event(&id, window, cx),
                    cx,
                ),
            );
        }
        let relative = match (date - today()).num_days() {
            0 => Some("Today"),
            1 => Some("Tomorrow"),
            -1 => Some("Yesterday"),
            _ => None,
        };
        column()
            .debug_selector(|| "calendar.day-panel".into())
            .w(px(DAY_PANEL_WIDTH))
            .flex_shrink_0()
            .p(px(CARD_INSET))
            .pt(px(CARD_INSET - EYEBROW_OPTICAL_LIFT))
            .gap(px(SPACE_4))
            .rounded(px(RADIUS_LG))
            .border_1()
            .border_color(rgb(BORDER))
            .bg(rgb(SURFACE_RAISED))
            .child(
                column()
                    .gap(px(SPACE_1))
                    .child(eyebrow(relative.unwrap_or("Selected day")))
                    .child(
                        div()
                            .text_size(type_size(TITLE_SIZE))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(date.format("%A, %b %-d").to_string()),
                    ),
            )
            .child(list)
    }

    fn week_view(&self, events: &[CalendarEvent], cx: &mut Context<Self>) -> Div {
        let days = week(self.anchor);
        let hours = LAST_HOUR - FIRST_HOUR;
        let grid_height = hours as f32 * HOUR_HEIGHT;
        let mut header = row()
            .w_full()
            .border_b_1()
            .border_color(rgb(BORDER_SUBTLE))
            .child(div().w(px(TIME_GUTTER)).flex_shrink_0());
        let mut all_day = row()
            .w_full()
            .items_stretch()
            .min_h(px(CONTROL_HEIGHT))
            .border_b_1()
            .border_color(rgb(BORDER_SUBTLE))
            .child(
                div()
                    .w(px(TIME_GUTTER))
                    .flex_shrink_0()
                    .pt(px(SPACE_2))
                    .pr(px(SPACE_2))
                    .flex()
                    .justify_end()
                    .child(hint("All day")),
            );
        let mut columns = row().w_full().relative().h(px(grid_height));
        let mut gutter = div().w(px(TIME_GUTTER)).flex_shrink_0().h_full().relative();
        for hour in FIRST_HOUR..LAST_HOUR {
            let label = NaiveTime::from_hms_opt(hour, 0, 0)
                .map(|t| t.format("%-I %p").to_string())
                .unwrap_or_default();
            gutter = gutter.child(
                div()
                    .absolute()
                    .top(px(
                        ((hour - FIRST_HOUR) as f32 * HOUR_HEIGHT - SPACE_2).max(0.)
                    ))
                    .right(px(SPACE_2))
                    .when(hour > FIRST_HOUR, |s| s.child(hint(label))),
            );
        }
        columns = columns.child(gutter);
        let now = now();
        for date in days.iter().copied() {
            header = header.child(
                column()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .py(px(SPACE_2))
                    .gap(px(SPACE_1))
                    .child(eyebrow(date.format("%a").to_string()))
                    .child(Self::numeral(date, false)),
            );
            let day = on_day(events, date);
            let (untimed, timed): (Vec<_>, Vec<_>) = day.into_iter().partition(|e| e.all_day);
            let mut strip = column()
                .flex_1()
                .min_w_0()
                .p(px(SPACE_1))
                .gap(px(SPACE_HALF))
                .border_l_1()
                .border_color(rgb(BORDER_SUBTLE));
            for event in untimed {
                strip = strip.child(self.chip(event, cx));
            }
            all_day = all_day.child(strip);
            let mut lane = div()
                .flex_1()
                .min_w_0()
                .h_full()
                .relative()
                .border_l_1()
                .border_color(rgb(BORDER_SUBTLE));
            for hour in FIRST_HOUR + 1..LAST_HOUR {
                lane = lane.child(
                    div()
                        .absolute()
                        .top(px((hour - FIRST_HOUR) as f32 * HOUR_HEIGHT))
                        .left_0()
                        .right_0()
                        .h(px(1.))
                        .bg(rgb(BORDER_SUBTLE)),
                );
            }
            let grid_start = day_start(date) + FIRST_HOUR as i64 * 3600;
            let grid_end = day_start(date) + LAST_HOUR as i64 * 3600;
            let placement = lanes(&timed);
            for (event, (index, count)) in timed.iter().zip(placement) {
                let from = event.starts_at.clamp(grid_start, grid_end);
                let to = event
                    .ends_at
                    .max(event.starts_at + 15 * 60)
                    .clamp(grid_start, grid_end);
                if to <= from {
                    continue;
                }
                let top = (from - grid_start) as f32 / 3600. * HOUR_HEIGHT;
                let height =
                    ((to - from) as f32 / 3600. * HOUR_HEIGHT).max(CONTROL_HEIGHT_SM + SPACE_1);
                // Overlaps cascade: each later lane starts further in and sits on
                // top, so every event keeps most of the column for its title.
                let offset = if count > 1 {
                    CASCADE_SPAN / (count - 1) as f32 * index as f32
                } else {
                    0.
                };
                let id = event.id.clone();
                let tall = height >= HOUR_HEIGHT * 0.75;
                let color = event_color(event);
                lane = lane.child(
                    div()
                        .absolute()
                        .top(px(top + 1.))
                        .left(relative(offset))
                        .w(relative(1. - offset))
                        .h(px(height - 2.))
                        .px(px(SPACE_HALF))
                        .child(action_button(
                            ButtonSpec {
                                id: SharedString::from(format!("calendar.block.{id}")).into(),
                                label: event.title.clone().into(),
                                enabled: true,
                            },
                            |button| {
                                // The title always shows. A tall block alone in its
                                // column adds the time; a shared or short one gives
                                // its room to the title, which wraps when it can.
                                // Only a block with the column to itself shows its time, so a
                                // cascaded event never leaves a fragment peeking out.
                                let with_time = tall && count == 1;
                                let title = div()
                                    .w_full()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(TEXT))
                                    .when(!tall, |s| s.truncate())
                                    .child(event.title.clone());
                                button
                                    .size_full()
                                    .items_stretch()
                                    .overflow_hidden()
                                    .gap(px(SPACE_2))
                                    .p(px(SPACE_HALF))
                                    .pr(px(SPACE_1))
                                    .rounded(px(RADIUS_SM))
                                    // A hairline in the grid's color separates cascaded blocks.
                                    .border_1()
                                    .border_color(rgb(SURFACE_RAISED))
                                    .bg(rgb(SURFACE_CONTROL))
                                    .hover(|s| s.bg(rgb(HOVER_STRONG)))
                                    .text_size(type_size(CAPTION_SIZE))
                                    .child(
                                        div()
                                            .w(px(EVENT_BAR_WIDTH))
                                            .flex_shrink_0()
                                            .rounded_full()
                                            .bg(color),
                                    )
                                    .child(
                                        column()
                                            .flex_1()
                                            .min_w_0()
                                            .when(tall, |s| s.py(px(SPACE_HALF)))
                                            .when(!tall, |s| s.justify_center())
                                            .child(title)
                                            .when(with_time, |s| {
                                                s.child(
                                                    div()
                                                        .w_full()
                                                        .truncate()
                                                        .text_color(rgb(TEXT_SECONDARY))
                                                        .child(span(event)),
                                                )
                                            }),
                                    )
                            },
                            move |this: &mut Self, window, cx| this.open_event(&id, window, cx),
                            cx,
                        )),
                );
            }
            if date == today() && now >= grid_start && now < grid_end {
                let top = (now - grid_start) as f32 / 3600. * HOUR_HEIGHT;
                lane = lane.child(
                    div()
                        .debug_selector(|| "calendar.now".into())
                        .absolute()
                        .top(px(top - NOW_DOT / 2. + 1.))
                        .left_0()
                        .right_0()
                        .h(px(NOW_DOT))
                        .flex()
                        .items_center()
                        // The conventional current-time marker: a dot where the line starts.
                        .child(
                            div()
                                .size(px(NOW_DOT))
                                .flex_shrink_0()
                                .rounded_full()
                                .bg(rgb(PRIMARY)),
                        )
                        .child(div().flex_1().h(px(2.)).bg(rgb(PRIMARY))),
                );
            }
            columns = columns.child(lane);
        }
        column()
            .debug_selector(|| "calendar.week".into())
            .w_full()
            .overflow_hidden()
            .rounded(px(RADIUS_LG))
            .border_1()
            .border_color(rgb(BORDER))
            .bg(rgb(SURFACE_RAISED))
            .child(header)
            .child(all_day)
            .child(columns)
    }

    fn agenda(&self, events: &[CalendarEvent], cx: &mut Context<Self>) -> Div {
        let mut days = column()
            .debug_selector(|| "calendar.agenda".into())
            .w_full()
            .gap(px(SPACE_5));
        let mut any = false;
        let today = today();
        for offset in 0..AGENDA_DAYS {
            let date = self.anchor + Duration::days(offset);
            let day = on_day(events, date);
            if day.is_empty() {
                continue;
            }
            any = true;
            let mut rows = column();
            for (index, event) in day.iter().enumerate() {
                let id = event.id.clone();
                let element = ElementId::from(SharedString::from(format!("calendar.row.{id}")));
                let (progress, on_hover) = self.hover.track(&element, true, cx);
                rows = rows
                    .when(index > 0, |s| s.child(divider()))
                    .child(action_button(
                        ButtonSpec {
                            id: element,
                            label: event.title.clone().into(),
                            enabled: true,
                        },
                        |button| {
                            button
                                .w_full()
                                .min_h(px(TABLE_ROW_HEIGHT + SPACE_2))
                                .px(px(CARD_INSET))
                                .gap(px(SPACE_4))
                                .rounded(px(0.))
                                .on_hover(on_hover)
                                .bg(rgba((HOVER_STRONG << 8) | (progress * 255.) as u32))
                                .child(
                                    div()
                                        .w(px(AGENDA_TIME_WIDTH))
                                        .flex_shrink_0()
                                        .text_size(type_size(LABEL_SIZE))
                                        .text_color(rgb(TEXT_SECONDARY))
                                        .child(span(event)),
                                )
                                .child(
                                    div()
                                        .w(px(EVENT_BAR_WIDTH))
                                        .h(px(SPACE_6))
                                        .flex_shrink_0()
                                        .rounded_full()
                                        .bg(event_color(event)),
                                )
                                .child(
                                    column()
                                        .flex_1()
                                        .min_w_0()
                                        .gap(px(SPACE_HALF))
                                        .child(
                                            div()
                                                .truncate()
                                                .font_weight(FontWeight::MEDIUM)
                                                .child(event.title.clone()),
                                        )
                                        .when_some(event.location.clone(), |s, location| {
                                            s.child(
                                                div()
                                                    .truncate()
                                                    .text_size(type_size(CAPTION_SIZE))
                                                    .text_color(rgb(TEXT_TERTIARY))
                                                    .child(location),
                                            )
                                        }),
                                )
                                .child(
                                    div()
                                        .flex_shrink_0()
                                        .text_size(type_size(CAPTION_SIZE))
                                        .text_color(rgb(TEXT_TERTIARY))
                                        .child(event.calendar.clone()),
                                )
                        },
                        move |this: &mut Self, window, cx| this.open_event(&id, window, cx),
                        cx,
                    ));
            }
            let is_today = date == today;
            let label = match (date - today).num_days() {
                0 => format!("{} · Today", date.format("%a")),
                1 => format!("{} · Tomorrow", date.format("%a")),
                _ => date.format("%a · %b").to_string(),
            };
            days = days.child(
                row()
                    .w_full()
                    .items_start()
                    .gap(px(SPACE_4))
                    .child(
                        column()
                            .w(px(AGENDA_DATE_WIDTH))
                            .flex_shrink_0()
                            // The numeral's cap height lines up with the first row's title.
                            .pt(px(SPACE_3))
                            .gap(px(SPACE_1))
                            .child(
                                div()
                                    .text_size(type_size(DISPLAY_SIZE))
                                    .line_height(relative(1.))
                                    .font_weight(if is_today {
                                        FontWeight::SEMIBOLD
                                    } else {
                                        FontWeight::LIGHT
                                    })
                                    .text_color(rgb(TEXT))
                                    .child(date.day().to_string()),
                            )
                            .child(eyebrow(label)),
                    )
                    .child(
                        column()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .rounded(px(RADIUS_LG))
                            .border_1()
                            .border_color(rgb(BORDER))
                            .bg(rgb(SURFACE_RAISED))
                            .child(rows),
                    ),
            );
        }
        if !any {
            return days.child(
                EmptyState::new("calendar", "Nothing scheduled")
                    .description("Events in the next six weeks appear here, soonest first.")
                    .selector("calendar.empty")
                    .build(),
            );
        }
        days
    }

    fn sync_control(&self, cx: &mut Context<Self>) -> AnyElement {
        let calendar = self.calendar.read(cx);
        match calendar.access {
            Access::Granted => {
                let syncing = calendar.syncing();
                Button::new(
                    "calendar.sync",
                    if syncing { "Syncing…" } else { "Sync now" },
                )
                .ghost()
                .small()
                .icon("refresh")
                .enabled(!syncing)
                .build(
                    &self.hover,
                    |this: &mut Self, _, cx| this.calendar.update(cx, |c, cx| c.sync(cx)),
                    cx,
                )
                .into_any_element()
            }
            Access::NotAsked => Button::new("calendar.allow", "Show Mac calendars")
                .secondary()
                .small()
                .icon("calendar")
                .build(
                    &self.hover,
                    |this: &mut Self, _, cx| this.calendar.update(cx, |c, cx| c.request_access(cx)),
                    cx,
                )
                .into_any_element(),
            Access::Denied => div().into_any_element(),
        }
    }
    fn status(&self, cx: &App) -> String {
        let calendar = self.calendar.read(cx);
        match (calendar.access, &calendar.last_import) {
            (Access::Granted, Some(import))
                if import.state == ainc_client::types::ActionState::Failed =>
            {
                "Your Mac's calendars could not be imported.".into()
            }
            (Access::Granted, Some(import)) => format!(
                "Your events and your Mac's calendars · synced {}",
                ago(import.finished_at.unwrap_or(import.created_at), now())
            ),
            (Access::Granted, None) => "Your events and your Mac's calendars.".into(),
            _ => "Your AgentInc events.".into(),
        }
    }
}

impl Render for CalendarPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.hover.animate(window);
        let view = VIEWS.iter().position(|(v, _)| *v == self.view).unwrap_or(0);
        let header = PageHeader::new("Calendar")
            .description(self.status(cx))
            .actions(
                row()
                    .gap(px(CONTROL_GAP))
                    .child(segmented(
                        "calendar.view",
                        VIEWS.iter().map(|(_, label)| *label),
                        view,
                        true,
                        &self.hover,
                        |this: &mut Self, index, _, cx| {
                            this.view = VIEWS[index].0;
                            let anchor = if this.view == View::Agenda && this.anchor < today() {
                                today()
                            } else {
                                this.anchor
                            };
                            this.go(anchor, cx);
                        },
                        cx,
                    ))
                    .child(
                        Button::new("calendar.new", "New event")
                            .primary()
                            .icon("plus")
                            .build(
                                &self.hover,
                                |this: &mut Self, window, cx| {
                                    let date = this.selected;
                                    this.open_new(date, window, cx)
                                },
                                cx,
                            ),
                    ),
            );
        let toolbar = row()
            .w_full()
            .justify_between()
            .gap(px(SPACE_4))
            .child(
                row()
                    .gap(px(SPACE_3))
                    .child(
                        row()
                            .gap(px(SPACE_1))
                            .child(
                                Button::new("calendar.previous", "Previous")
                                    .secondary()
                                    .small()
                                    .icon("chevronLeft")
                                    .icon_only()
                                    .build(
                                        &self.hover,
                                        |this: &mut Self, _, cx| this.step(false, cx),
                                        cx,
                                    ),
                            )
                            .child(
                                Button::new("calendar.today", "Today")
                                    .secondary()
                                    .small()
                                    .build(
                                        &self.hover,
                                        |this: &mut Self, _, cx| {
                                            this.selected = today();
                                            this.go(today(), cx)
                                        },
                                        cx,
                                    ),
                            )
                            .child(
                                Button::new("calendar.next", "Next")
                                    .secondary()
                                    .small()
                                    .icon("chevronRight")
                                    .icon_only()
                                    .build(
                                        &self.hover,
                                        |this: &mut Self, _, cx| this.step(true, cx),
                                        cx,
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .debug_selector(|| "calendar.period".into())
                            .text_size(type_size(HEADING_SIZE))
                            .font_weight(FontWeight::MEDIUM)
                            .child(self.period()),
                    ),
            )
            .child(self.sync_control(cx));
        let (events, error, loaded) = {
            let calendar = self.calendar.read(cx);
            (
                calendar.events.clone(),
                calendar.error.clone(),
                calendar.loaded,
            )
        };
        let mut page = Page::document(header).child(toolbar);
        if self.calendar.read(cx).access == Access::Denied {
            page = page.child(banner(
                Tone::Neutral,
                "Calendar access is off. Turn on AgentInc under System Settings → Privacy & Security → Calendars to see your Mac's events here.",
            ));
        }
        if let Some(error) = error {
            page = page.child(banner(Tone::Danger, error));
        }
        page = page.child(if !loaded {
            skeleton_rows("calendar.loading", 5).into_any_element()
        } else {
            match self.view {
                View::Month => {
                    let viewport = window.viewport_size();
                    let wide = viewport.width >= px(MONTH_WITH_DAY_MIN);
                    // The weeks fill the window, down to a readable minimum.
                    let rows = (month_grid(self.anchor).len() / 7) as f32;
                    let cell_height = ((f32::from(viewport.height) - MONTH_CHROME) / rows)
                        .max(MONTH_CELL_MIN_HEIGHT);
                    let dots = viewport.width < px(MONTH_TITLES_MIN);
                    self.month(&events, wide, cell_height, dots, cx)
                        .into_any_element()
                }
                View::Week => self.week_view(&events, cx).into_any_element(),
                View::Agenda => self.agenda(&events, cx).into_any_element(),
            }
        });
        div()
            .id("calendar.page")
            .debug_selector(|| "calendar.page".into())
            .size_full()
            .child(page.build())
    }
}

#[cfg(test)]
impl CalendarPage {
    #[allow(dead_code)]
    pub(crate) fn fixture_view(&mut self, view: View, cx: &mut Context<Self>) {
        self.view = view;
        self.anchor = today();
        self.selected = today();
        cx.notify();
    }
}
