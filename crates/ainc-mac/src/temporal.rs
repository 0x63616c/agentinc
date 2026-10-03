//! The Temporal page: the retained work table (runs, Conversations, Automation
//! occurrences) with status filters and timing columns.
use crate::{
    action::{Pending, Run},
    daemon::Daemon,
    page::Page,
    routes::{Destination, Route},
    ui::*,
};
use ainc_client::types::{WorkKind, WorkStatus, WorkView};
use gpui::{prelude::*, *};
use std::sync::Arc;

/// The app's reading of a work status.
fn work_state(status: WorkStatus) -> WorkState {
    match status {
        WorkStatus::Running => WorkState::Running,
        WorkStatus::Completed => WorkState::Done,
        WorkStatus::Failed => WorkState::Failed,
        WorkStatus::Cancelled => WorkState::Cancelled,
    }
}

/// `(status filter, label)`: every status the work history reports, after "All".
fn filters() -> Vec<(Option<WorkStatus>, &'static str)> {
    std::iter::once((None, "All"))
        .chain(
            [
                WorkStatus::Running,
                WorkStatus::Completed,
                WorkStatus::Failed,
                WorkStatus::Cancelled,
            ]
            .into_iter()
            .map(|status| (Some(status), work_state(status).label())),
        )
        .collect()
}

fn kind_label(kind: WorkKind) -> &'static str {
    match kind {
        WorkKind::Run => "Work",
        WorkKind::Session => "Conversation",
        WorkKind::Occurrence => "Occurrence",
    }
}

fn columns() -> [TableColumn; 4] {
    [
        TableColumn::new("Workflow"),
        TableColumn::new("Status").width(150.),
        TableColumn::new("Started").width(170.),
        TableColumn::new("Duration").width(90.).right(),
    ]
}

pub struct TemporalPage {
    daemon: Arc<Daemon>,
    rows: Vec<WorkView>,
    filter: Option<WorkStatus>,
    next_page: Option<String>,
    loading: Pending,
    loaded: bool,
    error: Option<String>,
    hover: HoverFade,
}

impl HoverHost for TemporalPage {
    fn hover_fade(&mut self) -> &mut HoverFade {
        &mut self.hover
    }
}

impl TemporalPage {
    pub fn new(daemon: Arc<Daemon>, cx: &mut Context<Self>) -> Self {
        let page = Self {
            daemon,
            rows: Vec::new(),
            filter: None,
            next_page: None,
            loading: Pending::default(),
            loaded: false,
            error: None,
            hover: HoverFade::default(),
        };
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(1))
                    .await;
                if this
                    .update(cx, |this, cx| {
                        if this
                            .rows
                            .iter()
                            .any(|row| row.status == WorkStatus::Running)
                        {
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        page
    }

    fn ensure_loaded(&mut self, cx: &mut Context<Self>) {
        if !self.loaded && !self.loading.busy() {
            self.load(false, cx);
        }
    }

    #[cfg(all(test, feature = "rendered-tests"))]
    #[allow(dead_code)]
    pub(crate) fn fixture(&mut self, page: ainc_client::types::WorkPage, cx: &mut Context<Self>) {
        self.rows = page.work;
        self.next_page = page.next_page;
        self.loaded = true;
        self.error = None;
        cx.notify();
    }

    #[cfg(all(test, feature = "rendered-tests"))]
    #[allow(dead_code)]
    pub(crate) fn fixture_error(&mut self, cx: &mut Context<Self>) {
        self.rows.clear();
        self.loaded = true;
        self.error = Some(copy::unavailable("Temporal", "Try again"));
        cx.notify();
    }

    #[cfg(all(test, feature = "rendered-tests"))]
    #[allow(dead_code)]
    pub(crate) fn fixture_loading(&mut self, cx: &mut Context<Self>) {
        self.rows.clear();
        self.next_page = None;
        self.loading.hold();
        self.loaded = false;
        self.error = None;
        cx.notify();
    }

    fn load(&mut self, more: bool, cx: &mut Context<Self>) {
        if self.loading.busy() {
            return;
        }
        let status = self
            .filter
            .map_or_else(|| "all".to_owned(), |status| status.to_string());
        let page = if more { self.next_page.clone() } else { None };
        if more && page.is_none() {
            return;
        }
        if !more {
            self.rows.clear();
            self.next_page = None;
            self.loaded = false;
        }
        self.error = None;
        let daemon = self.daemon.clone();
        cx.run(
            &self.loading.clone(),
            move || anyhow::Ok(daemon.fetch(crate::daemon::Executions { status, page })?),
            |this, result, _| {
                this.loaded = true;
                match result {
                    Ok(page) => {
                        this.rows.extend(page.work);
                        this.next_page = page.next_page;
                    }
                    Err(failure) => this.error = Some(failure.message("Work history")),
                }
            },
        );
    }

    fn table(&self, now: i64, cx: &mut Context<Self>) -> Div {
        let columns = columns();
        let rows = self
            .rows
            .iter()
            .map(|execution| {
                let id = execution.id.clone();
                let state = work_state(execution.status);
                let selector = format!("temporal.row.{id}");
                let cells = vec![
                    column()
                        .gap(px(SPACE_HALF))
                        .child(
                            div()
                                .truncate()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(TEXT))
                                .child(kind_label(execution.kind).to_owned()),
                        )
                        .child(
                            div()
                                .truncate()
                                .text_size(type_size(CAPTION_SIZE))
                                .text_color(rgb(TEXT_SECONDARY))
                                .child(id.clone()),
                        )
                        .into_any_element(),
                    row()
                        .child(status_pill(state.label().to_owned(), state.tone()))
                        .into_any_element(),
                    column()
                        .gap(px(SPACE_HALF))
                        .child(
                            div()
                                .text_color(rgb(TEXT))
                                .child(time::relative(execution.started_at / 1000, now)),
                        )
                        .child(caption(time::absolute(execution.started_at / 1000)))
                        .into_any_element(),
                    div()
                        .font_family("SF Mono")
                        .text_size(type_size(LABEL_SIZE))
                        .text_color(rgb(TEXT))
                        .child(time::duration(
                            execution.closed_at.unwrap_or(now * 1000) - execution.started_at,
                        ))
                        .into_any_element(),
                ];
                table_row(
                    SharedString::from(selector.clone()),
                    id,
                    &columns,
                    cells,
                    false,
                    &self.hover,
                    |_, _, _| {},
                    cx,
                )
                .debug_selector(move || selector.clone())
                .py(px(SPACE_2))
            })
            .collect::<Vec<_>>();
        table_container()
            .debug_selector(|| "temporal.table".into())
            .child(table_header(&columns))
            .children(rows)
    }
}

impl EventEmitter<Destination> for TemporalPage {}
impl Page for TemporalPage {
    const ROUTE: Route = Route::Temporal;
    fn shown(&mut self, shown: bool, cx: &mut Context<Self>) {
        if shown {
            self.ensure_loaded(cx);
        }
    }
}

impl Render for TemporalPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.hover.animate(window);
        let now = time::now();
        let filters = filters();
        let filter = filters
            .iter()
            .position(|(value, _)| *value == self.filter)
            .unwrap_or(0);
        let header = PageHeader::new(self.title())
            .description(
                "Every run, Conversation and Automation occurrence in this AgentInc runtime.",
            )
            .actions(
                Button::new("temporal.refresh", "Refresh")
                    .icon("refresh")
                    .icon_only()
                    .secondary()
                    .enabled(!self.loading.busy())
                    .build(&self.hover, |this, _, cx| this.load(false, cx), cx),
            );
        let mut content = column()
            .w_full()
            .gap(px(SPACE_5))
            .child(row().child(segmented(
                "temporal.filter",
                filters.iter().map(|(_, label)| *label),
                filter,
                !self.loading.busy(),
                &self.hover,
                |this, index, _, cx| {
                    this.filter = self::filters()[index].0;
                    this.load(false, cx);
                },
                cx,
            )));
        if let Some(error) = &self.error {
            content = content.child(
                banner(Tone::Danger, error.clone())
                    .id("temporal.error")
                    .accessibility_id("temporal.error")
                    .debug_selector(|| "temporal.error".into()),
            );
        }
        if !self.loaded {
            content = content.child(
                div()
                    .id("temporal.loading")
                    .accessibility_id("temporal.loading")
                    .child(skeleton_rows("temporal.loading", SKELETON_ROWS)),
            );
        } else if self.rows.is_empty() && self.error.is_none() {
            content = content.child(
                EmptyState::new(
                    "temporal",
                    if self.filter.is_none() {
                        "No work yet."
                    } else {
                        "No matching work."
                    },
                )
                .description(if self.filter.is_none() {
                    "Work appears here the moment an Agent starts."
                } else {
                    "Try another status or refresh the list."
                })
                .selector("temporal.empty")
                .build(),
            );
        }
        if !self.rows.is_empty() {
            content = content.child(self.table(now, cx));
        }
        if self.next_page.is_some() {
            content = content.child(
                row().child(
                    Button::new("temporal.more", "Load More")
                        .secondary()
                        .enabled(!self.loading.busy())
                        .build(&self.hover, |this, _, cx| this.load(true, cx), cx),
                ),
            );
        }
        PageFrame::document(header)
            .child(
                div()
                    .id("temporal.page")
                    .accessibility_id("temporal.page")
                    .w_full()
                    .child(content),
            )
            .build()
    }
}
