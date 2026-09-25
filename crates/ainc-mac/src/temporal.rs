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

fn workflow_label(workflow_type: &str) -> &str {
    match workflow_type {
        "agentinc.run" => "Agent run",
        "agentinc.session" => "Conversation",
        "turnkeel.occurrence" => "Automation occurrence",
        other => other,
    }
}

fn status_colors(status: &str) -> (u32, u32) {
    match status {
        "Running" => (STATUS_BLUE, STATUS_BLUE_SURFACE),
        "Completed" => (STATUS_GREEN, STATUS_GREEN_SURFACE),
        "Failed" | "Terminated" | "TimedOut" => (DESTRUCTIVE_TEXT, SURFACE_ERROR),
        _ => (STATUS_AMBER, STATUS_AMBER_SURFACE),
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
    pub(crate) fn fixture_error(&mut self, cx: &mut Context<Self>) {
        self.rows.clear();
        self.loading = false;
        self.loaded = true;
        self.error = Some("Temporal is unavailable. Try again.".into());
        cx.notify();
    }

    #[cfg(all(test, feature = "rendered-tests"))]
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

    // Keep the shared action_button contract; this only adds the page's hover fade.
    fn hover_action(
        &self,
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        enabled: bool,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + Clone + 'static,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let id = id.into();
        let hover_id = id.clone();
        let background = self.hover.color(&id);
        let on_hover = cx.listener(move |this, over, _, cx| {
            this.hover.set(hover_id.clone(), *over);
            cx.notify();
        });
        action_button(
            ButtonSpec {
                id,
                label: label.into(),
                kind: ButtonKind::Secondary,
                enabled,
            },
            |button| {
                standard_button(button)
                    .bg(background)
                    .border_1()
                    .border_color(rgb(BORDER))
                    .on_hover(on_hover)
            },
            action,
            cx,
        )
    }
}

impl Render for TemporalPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.hover.animate(window);
        let now = chrono::Utc::now().timestamp_millis();
        let filters = FILTERS
            .chunks(4)
            .map(|group| {
                row()
                    .gap(px(7.))
                    .children(group.iter().map(|(value, label)| {
                        let selected = self.filter == *value;
                        self.hover_action(
                            format!("temporal.filter.{value}"),
                            *label,
                            !self.loading,
                            move |this, _, cx| {
                                this.filter = value;
                                this.load(false, cx);
                            },
                            cx,
                        )
                        .min_h(px(30.))
                        .px(px(11.))
                        .when(selected, |button| {
                            button
                                .bg(rgb(SELECTED_SEGMENT))
                                .border_color(rgb(SELECTED_BORDER))
                                .text_color(rgb(TEXT))
                        })
                        .child(*label)
                    }))
            })
            .collect::<Vec<_>>();
        let header = PageHeader::new("Temporal")
            .description("Workflow executions in this AgentInc runtime · newest first")
            .actions(
                self.hover_action(
                    "temporal.refresh",
                    "Refresh workflows",
                    !self.loading,
                    |this, _, cx| this.load(false, cx),
                    cx,
                )
                .child("Refresh"),
            );
        let mut content = column()
            .w_full()
            .gap(px(22.))
            .child(column().gap(px(7.)).children(filters));
        if !self.ui_available && self.loaded && self.error.is_none() {
            content = content.child(
                column()
                    .debug_selector(|| "temporal.no-ui".into())
                    .gap(px(2.))
                    .px(px(14.))
                    .py(px(10.))
                    .rounded(px(7.))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .child("Workflow links unavailable")
                    .child(
                        div()
                            .text_color(rgb(MUTED))
                            .text_size(type_size(LABEL_SIZE))
                            .child("Temporal Web UI is not configured for this deployment."),
                    ),
            );
        }
        if let Some(error) = &self.error {
            content = content.child(
                div()
                    .id("temporal.error")
                    .accessibility_id("temporal.error")
                    .debug_selector(|| "temporal.error".into())
                    .p(px(18.))
                    .rounded(px(8.))
                    .bg(rgb(SURFACE_ERROR))
                    .text_color(rgb(DESTRUCTIVE_TEXT))
                    .child(error.clone()),
            );
        }
        if !self.loaded && self.loading {
            content = content.child(
                div()
                    .id("temporal.loading")
                    .accessibility_id("temporal.loading")
                    .debug_selector(|| "temporal.loading".into())
                    .py(px(40.))
                    .text_color(rgb(MUTED))
                    .child("Loading workflow executions…"),
            );
        } else if self.rows.is_empty() && self.error.is_none() {
            content = content.child(
                column()
                    .debug_selector(|| "temporal.empty".into())
                    .gap(px(5.))
                    .py(px(48.))
                    .child(if self.filter == "All" {
                        "No workflows yet"
                    } else {
                        "No matching workflows"
                    })
                    .child(div().text_color(rgb(MUTED)).child(if self.filter == "All" {
                        "Runs will appear here when AgentInc starts work."
                    } else {
                        "Try another status or refresh the list."
                    })),
            );
        }
        if !self.rows.is_empty() {
            let header = row()
                .w_full()
                .h(px(34.))
                .px(px(16.))
                .text_size(type_size(CAPTION_SIZE))
                .text_color(rgb(MUTED))
                .font_weight(FontWeight::MEDIUM)
                .child(div().flex_1().min_w_0().child("WORKFLOW"))
                .child(div().w(px(116.)).child("STATUS"))
                .child(div().w(px(163.)).child("STARTED"))
                .child(
                    div()
                        .w(px(92.))
                        .text_align(TextAlign::Right)
                        .child("DURATION"),
                );
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
                    let (foreground, surface) = status_colors(&status);
                    self.hover_action(
                        format!("temporal.row.{run_id}"),
                        format!("Open {id} in Temporal"),
                        url.is_some(),
                        move |_, _, cx| {
                            if let Some(url) = &url {
                                cx.open_url(url);
                            }
                        },
                        cx,
                    )
                    .debug_selector(move || format!("temporal.row.{run_id}"))
                    .w_full()
                    .h_auto()
                    .min_h(px(63.))
                    .px(px(16.))
                    .rounded(px(0.))
                    .border_0()
                    .border_b_1()
                    .border_color(rgb(BORDER_SUBTLE))
                    .child(
                        column()
                            .flex_1()
                            .min_w_0()
                            .gap(px(1.))
                            .child(
                                div()
                                    .truncate()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(workflow_label(&execution.workflow_type).to_owned()),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_size(type_size(CAPTION_SIZE))
                                    .text_color(rgb(MUTED))
                                    .child(format!("{short_id} · {id}")),
                            ),
                    )
                    .child(
                        row().w(px(116.)).child(
                            div()
                                .px(px(8.))
                                .py(px(3.))
                                .rounded(px(5.))
                                .bg(rgb(surface))
                                .text_color(rgb(foreground))
                                .text_size(type_size(CAPTION_SIZE))
                                .child(status_label(&status).to_owned()),
                        ),
                    )
                    .child(
                        column()
                            .w(px(163.))
                            .gap(px(1.))
                            .child(relative_time(execution.started_at, now))
                            .child(
                                div()
                                    .text_size(type_size(CAPTION_SIZE))
                                    .text_color(rgb(MUTED))
                                    .child(absolute_time(execution.started_at)),
                            ),
                    )
                    .child(
                        div()
                            .w(px(92.))
                            .text_align(TextAlign::Right)
                            .font_family("SF Mono")
                            .child(duration(execution.started_at, execution.closed_at, now)),
                    )
                })
                .collect::<Vec<_>>();
            content = content.child(
                column()
                    .debug_selector(|| "temporal.table".into())
                    .w_full()
                    .rounded(px(9.))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .overflow_hidden()
                    .child(header)
                    .children(rows),
            );
        }
        if self.next_page.is_some() {
            content = content.child(
                self.hover_action(
                    "temporal.more",
                    "Load more workflows",
                    !self.loading,
                    |this, _, cx| this.load(true, cx),
                    cx,
                )
                .child(if self.loading {
                    "Loading…"
                } else {
                    "Load more"
                }),
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
