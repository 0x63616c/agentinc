use crate::{storage, ui::*};
use ainc_client::types::{ExecutionPage, ExecutionView};
use gpui::{prelude::*, *};

const FILTERS: &[(&str, &str)] = &[
    ("All", "All"),
    ("Running", "Running"),
    ("Completed", "Completed"),
    ("Failed", "Failed"),
    ("Canceled", "Cancelled"),
    ("Terminated", "Terminated"),
    ("TimedOut", "Timed out"),
    ("ContinuedAsNew", "Continued as new"),
];

fn status_label(status: &str) -> &str {
    FILTERS
        .iter()
        .find(|(value, _)| *value == status)
        .map_or(status, |(_, label)| label)
}

fn workflow_label<'a>(workflow_type: &'a str, workflow_id: &str) -> &'a str {
    match workflow_type {
        "agentinc.run" => "Agent run",
        "agentinc.session" => "Conversation",
        // Durable product actions share the occurrence workflow; their IDs say which.
        "turnkeel.occurrence" if workflow_id.starts_with("home-") => "Smart Home action",
        "turnkeel.occurrence" if workflow_id.starts_with("calendar-import-") => "Calendar import",
        "turnkeel.occurrence" => "Automation occurrence",
        other => other,
    }
}

fn status_tone(status: &str) -> Tone {
    match status {
        "Running" => Tone::Info,
        "Completed" => Tone::Success,
        "Failed" | "Terminated" | "TimedOut" => Tone::Danger,
        "ContinuedAsNew" => Tone::Accent,
        _ => Tone::Warning,
    }
}

fn relative_time(started_at: i64, now: i64) -> String {
    let seconds = (now - started_at).max(0) / 1000;
    if seconds < 60 {
        "just now".into()
    } else if seconds < 3600 {
        format!("{}m ago", seconds / 60)
    } else if seconds < 86_400 {
        format!("{}h ago", seconds / 3600)
    } else {
        format!("{}d ago", seconds / 86_400)
    }
}

fn absolute_time(timestamp: i64) -> String {
    chrono::DateTime::from_timestamp_millis(timestamp)
        .map(|time| {
            time.with_timezone(&chrono::Local)
                .format("%b %-d, %Y · %H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|| "Time unavailable".into())
}

fn duration(started_at: i64, closed_at: Option<i64>, now: i64) -> String {
    let seconds = (closed_at.unwrap_or(now) - started_at).max(0) / 1000;
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3600 {
        format!("{}m {:02}s", seconds / 60, seconds % 60)
    } else {
        format!("{}h {:02}m", seconds / 3600, (seconds % 3600) / 60)
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
    rows: Vec<ExecutionView>,
    filter: &'static str,
    next_page: Option<String>,
    ui_available: bool,
    loading: bool,
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
    pub fn new(cx: &mut Context<Self>) -> Self {
        let page = Self {
            rows: Vec::new(),
            filter: "All",
            next_page: None,
            ui_available: false,
            loading: false,
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
                        if this.rows.iter().any(|row| row.status == "Running") {
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

    pub(crate) fn ensure_loaded(&mut self, cx: &mut Context<Self>) {
        if !self.loaded && !self.loading {
            self.load(false, cx);
        }
    }

    #[cfg(all(test, feature = "rendered-tests"))]
    #[allow(dead_code)]
    pub(crate) fn fixture(&mut self, page: ExecutionPage, cx: &mut Context<Self>) {
        self.rows = page.executions;
        self.next_page = page.next_page;
        self.ui_available = page.ui_available;
        self.loading = false;
        self.loaded = true;
        self.error = None;
        cx.notify();
    }

    #[cfg(all(test, feature = "rendered-tests"))]
    #[allow(dead_code)]
    pub(crate) fn fixture_error(&mut self, cx: &mut Context<Self>) {
        self.rows.clear();
        self.loading = false;
        self.loaded = true;
        self.error = Some("Temporal is unavailable. Try again.".into());
        cx.notify();
    }

    #[cfg(all(test, feature = "rendered-tests"))]
    #[allow(dead_code)]
    pub(crate) fn fixture_loading(&mut self, cx: &mut Context<Self>) {
        self.rows.clear();
        self.next_page = None;
        self.loading = true;
        self.loaded = false;
        self.error = None;
        cx.notify();
    }

    fn load(&mut self, more: bool, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        let status = self.filter.to_owned();
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
        self.loading = true;
        cx.notify();
        let work = cx.background_executor().spawn(async move {
            storage::background(async {
                let client = storage::client().await?;
                let mut request = client.temporal_executions().status(status);
                if let Some(page) = page {
                    request = request.page(page);
                }
                let response = request.send().await?;
                anyhow::Ok(response.into_inner())
            })
        });
        cx.spawn(async move |this, cx| {
            let result: anyhow::Result<ExecutionPage> = work.await;
            let _ = this.update(cx, |this, cx| {
                this.loading = false;
                this.loaded = true;
                match result {
                    Ok(page) => {
                        this.rows.extend(page.executions);
                        this.next_page = page.next_page;
                        this.ui_available = page.ui_available;
                    }
                    Err(error) => {
                        this.error = Some(format!("Workflow history unavailable: {error}"))
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn table(&self, now: i64, cx: &mut Context<Self>) -> Div {
        let columns = columns();
        let rows = self
            .rows
            .iter()
            .map(|execution| {
                let id = execution.workflow_id.clone();
                let run_id = execution.run_id.clone();
                let url = execution.url.clone();
                let status = execution.status.clone();
                let short_id = run_id
                    .get(run_id.len().saturating_sub(8)..)
                    .unwrap_or(&run_id)
                    .to_owned();
                let selector = format!("temporal.row.{run_id}");
                let cells = vec![
                    column()
                        .gap(px(SPACE_HALF))
                        .child(
                            div()
                                .truncate()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(TEXT))
                                .child(workflow_label(&execution.workflow_type, &id).to_owned()),
                        )
                        .child(
                            div()
                                .truncate()
                                .text_size(type_size(CAPTION_SIZE))
                                .text_color(rgb(TEXT_SECONDARY))
                                .child(format!("{short_id} · {id}")),
                        )
                        .into_any_element(),
                    row()
                        .child(status_pill(
                            status_label(&status).to_owned(),
                            status_tone(&status),
                        ))
                        .into_any_element(),
                    column()
                        .gap(px(SPACE_HALF))
                        .child(
                            div()
                                .text_color(rgb(TEXT))
                                .child(relative_time(execution.started_at, now)),
                        )
                        .child(caption(absolute_time(execution.started_at)))
                        .into_any_element(),
                    div()
                        .font_family("SF Mono")
                        .text_size(type_size(LABEL_SIZE))
                        .text_color(rgb(TEXT))
                        .child(duration(execution.started_at, execution.closed_at, now))
                        .into_any_element(),
                ];
                table_row(
                    SharedString::from(selector.clone()),
                    format!("Open {id} in Temporal"),
                    &columns,
                    cells,
                    url.is_some(),
                    &self.hover,
                    move |_, _, cx| {
                        if let Some(url) = &url {
                            cx.open_url(url);
                        }
                    },
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

impl Render for TemporalPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.hover.animate(window);
        let now = chrono::Utc::now().timestamp_millis();
        let filter = FILTERS
            .iter()
            .position(|(value, _)| *value == self.filter)
            .unwrap_or(0);
        let header = PageHeader::new("Temporal")
            .description("Every workflow execution in this AgentInc runtime, newest first.")
            .actions(
                Button::new("temporal.refresh", "Refresh")
                    .secondary()
                    .icon("refresh")
                    .enabled(!self.loading)
                    .build(&self.hover, |this, _, cx| this.load(false, cx), cx),
            );
        let mut content = column()
            .w_full()
            .gap(px(SPACE_5))
            .child(row().child(segmented(
                "temporal.filter",
                FILTERS.iter().map(|(_, label)| *label),
                filter,
                !self.loading,
                &self.hover,
                |this, index, _, cx| {
                    this.filter = FILTERS[index].0;
                    this.load(false, cx);
                },
                cx,
            )));
        if !self.ui_available && self.loaded && self.error.is_none() {
            content = content.child(
                banner(
                    Tone::Info,
                    "Workflow links unavailable: Temporal Web UI is not configured for this deployment.",
                )
                .debug_selector(|| "temporal.no-ui".into()),
            );
        }
        if let Some(error) = &self.error {
            content = content.child(
                banner(Tone::Danger, error.clone())
                    .id("temporal.error")
                    .accessibility_id("temporal.error")
                    .debug_selector(|| "temporal.error".into()),
            );
        }
        if !self.loaded && self.loading {
            content = content.child(
                div()
                    .id("temporal.loading")
                    .accessibility_id("temporal.loading")
                    .child(skeleton_rows("temporal.loading", 5)),
            );
        } else if self.rows.is_empty() && self.error.is_none() {
            content = content.child(
                EmptyState::new(
                    "temporal",
                    if self.filter == "All" {
                        "No workflows yet"
                    } else {
                        "No matching workflows"
                    },
                )
                .description(if self.filter == "All" {
                    "Runs appear here the moment AgentInc starts work."
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
                    Button::new("temporal.more", "Load more")
                        .secondary()
                        .enabled(!self.loading)
                        .build(&self.hover, |this, _, cx| this.load(true, cx), cx),
                ),
            );
        }
        Page::document(header)
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
