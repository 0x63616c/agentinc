//! Todos: the list of things to do, each ticked off in place, and the dialog
//! that adds one.
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
use ainc_client::types::{Todo, TodoCommand, TodoSnapshot};
use gpui::{prelude::*, *};
use std::{cell::RefCell, rc::Rc, sync::Arc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Dialog {
    Add,
}

pub struct TodosPage {
    daemon: Arc<Daemon>,
    sync: Entity<Sync>,
    overlays: PageOverlays<Dialog>,
    state: Arc<TodoSnapshot>,
    title: Entity<TextInput>,
    show_done: bool,
    form_error: Option<String>,
    pending: Pending,
    cancel_focus: FocusHandle,
    submit_focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<Destination> for TodosPage {}

impl TodosPage {
    pub fn new(
        daemon: Arc<Daemon>,
        sync: Entity<Sync>,
        overlays: Rc<RefCell<OverlayHost<Overlay>>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let title =
            cx.new(|cx| TextInput::field("What needs doing?", false, cx).identified("todos.title"));
        let subscriptions = vec![
            cx.observe(&title, |_, _, cx| cx.notify()),
            cx.observe(&sync, |_, _, cx| cx.notify()),
            cx.subscribe(&sync, |this, _, event: &SliceChanged, cx| {
                if *event == SliceChanged::Todos {
                    this.reload();
                    cx.notify();
                }
            }),
        ];
        let mut this = Self {
            state: daemon.todos(),
            daemon,
            sync,
            overlays: PageOverlays::new(overlays, Route::Todos),
            title,
            show_done: false,
            form_error: None,
            pending: Pending::default(),
            cancel_focus: cx.focus_handle(),
            submit_focus: cx.focus_handle(),
            _subscriptions: subscriptions,
        };
        this.reload();
        this
    }
    pub(crate) fn reload(&mut self) {
        self.state = self.daemon.todos();
    }
    pub fn open_add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.form_error = None;
        self.title.update(cx, |input, cx| {
            input.reset();
            cx.notify();
        });
        let focus = self.title.focus_handle(cx);
        self.overlays.open_dialog(Dialog::Add, focus, window, cx);
        cx.notify();
    }
    fn send(&mut self, command: TodoCommand, thing: &'static str, cx: &mut Context<Self>) {
        let daemon = self.daemon.clone();
        cx.run(
            &self.pending.clone(),
            move || {
                daemon.send(command)?;
                Ok(())
            },
            move |this, result, _| {
                this.reload();
                match result {
                    Ok(()) => {
                        this.overlays.close();
                        this.form_error = None;
                    }
                    Err(failure) => this.form_error = Some(failure.message(thing)),
                }
            },
        );
    }
    fn add(&mut self, cx: &mut Context<Self>) {
        let title = self.title.read(cx).content.trim().to_owned();
        if title.is_empty() {
            return;
        }
        self.send(TodoCommand::Add { title }, "The Todo", cx);
    }
    fn add_button(&self, id: &'static str, secondary: bool, ui: &mut Ui<Self>) -> Stateful<Div> {
        let button = Button::new(id, "New Todo")
            .icon(Icon::Plus)
            .enabled(!self.pending.busy());
        if secondary {
            button.secondary()
        } else {
            button.primary()
        }
        .build(ui, Self::open_add)
    }
    fn todo_row(&self, index: usize, count: usize, todo: &Todo, ui: &mut Ui<Self>) -> Div {
        let id = todo.id;
        let completed = todo.completed;
        let busy = self.pending.busy();
        row()
            .group(SharedString::from(format!("todo.{id}")))
            .min_h(px(LIST_ROW_HEIGHT))
            .px(px(SPACE_3))
            .py(px(SPACE_1))
            .gap(px(SPACE_2))
            .items_center()
            .when(index + 1 < count, |s| {
                s.border_b_1().border_color(rgb(theme().border_subtle))
            })
            .child(
                div().flex_1().min_w_0().child(
                    Checkbox::new(
                        SharedString::from(format!("todo.{id}.done")),
                        todo.title.clone(),
                    )
                    .checked(completed)
                    .enabled(!busy)
                    .build(ui, move |this, _, cx| {
                        this.send(
                            TodoCommand::Complete {
                                id,
                                completed: !completed,
                            },
                            "The Todo",
                            cx,
                        )
                    }),
                ),
            )
            .child(
                div()
                    .opacity(0.)
                    .group_hover(SharedString::from(format!("todo.{id}")), |s| s.opacity(1.))
                    .child(
                        Button::new(SharedString::from(format!("todo.{id}.delete")), "Delete")
                            .icon(Icon::Trash)
                            .icon_only()
                            .ghost()
                            .small()
                            .enabled(!busy)
                            .build(ui, move |this, _, cx| {
                                this.send(TodoCommand::Delete { id }, "The Todo", cx)
                            }),
                    ),
            )
    }
}

impl Page for TodosPage {
    const ROUTE: Route = Route::Todos;
    fn overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.overlays.active()?;
        let ui = &mut Ui::new(window, cx);
        let body = column_gap(FORM_STACK_GAP)
            .child(text_field("Title", self.title.clone(), ui))
            .when_some(self.form_error.clone(), |s, error| {
                s.child(error_text(error))
            });
        let footer = DialogFooter::new(Verb::Create)
            .ids("todos.cancel", "todos.submit")
            .enabled(!self.title.read(ui.cx).content.trim().is_empty())
            .pending(self.pending.busy())
            .focus(&self.cancel_focus, &self.submit_focus)
            .build(
                ui,
                |this: &mut Self, window, cx| {
                    this.overlays.dismiss(window, cx);
                    cx.notify();
                },
                |this: &mut Self, _, cx| this.add(cx),
            );
        Some(dialog_shell("New Todo", body, footer).into_any_element())
    }
    fn focus_handles(&self, cx: &App) -> Vec<FocusHandle> {
        vec![
            self.title.focus_handle(cx),
            self.cancel_focus.clone(),
            self.submit_focus.clone(),
        ]
    }
    fn drafts(&self, cx: &App) -> anyhow::Result<Drafts> {
        anyhow::ensure!(
            !self.pending.busy(),
            "Wait for the current change to finish before installing"
        );
        let mut drafts = Drafts::default();
        drafts.text("title", &self.title, cx);
        Ok(drafts)
    }
    fn restore(&mut self, drafts: Drafts, cx: &mut Context<Self>) {
        drafts.restore_text("title", &self.title, cx);
    }
}

impl Render for TodosPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sync = self.sync.read(cx);
        let state = sync.load_state();
        let fetching = sync.fetching();
        let todos = self.state.clone();
        let open: Vec<&Todo> = todos.todos.iter().filter(|t| !t.completed).collect();
        let done: Vec<&Todo> = todos.todos.iter().filter(|t| t.completed).collect();
        let show_done = self.show_done;
        let ui = &mut Ui::new(window, cx);
        PageFrame::document(
            PageHeader::new(self.title()).actions(
                row_gap(CONTROL_GAP)
                    .child(
                        Button::new("todos.refresh", "Refresh")
                            .icon(Icon::Refresh)
                            .icon_only()
                            .secondary()
                            .enabled(!fetching)
                            .build(ui, |this, _, cx| {
                                this.sync.update(cx, |sync, cx| sync.wake(cx))
                            }),
                    )
                    .child(self.add_button("todos.create", false, ui)),
            ),
        )
        .child(column().gap(px(SPACE_4)).children(page_frame(
            "todos",
            &state,
            SKELETON_ROWS,
            ui,
            |ui| {
                if todos.todos.is_empty() {
                    return match state.error {
                        Some(_) => div().into_any_element(),
                        None => EmptyState::new(Icon::Todos, "No Todos yet.")
                            .description(
                                "Add the first thing on your mind. Tick it off when it is done.",
                            )
                            .selector("todos.empty")
                            .action(self.add_button("todos.create.empty", true, ui))
                            .build()
                            .into_any_element(),
                    };
                }
                let open_count = open.len();
                let done_count = done.len();
                column()
                    .gap(px(SECTION_GAP))
                    .child(column().gap(px(SPACE_2)).child(eyebrow("Open")).child(
                        if open.is_empty() {
                            card()
                                .items_center()
                                .py(px(SPACE_6))
                                .child(caption("All done. Enjoy the quiet."))
                        } else {
                            card().p(px(SPACE_1)).gap_0().children(
                                open.iter().enumerate().map(|(index, todo)| {
                                    self.todo_row(index, open_count, todo, ui)
                                }),
                            )
                        },
                    ))
                    .when(done_count > 0, |s| {
                        s.child(
                            column()
                                .gap(px(SPACE_2))
                                .child(
                                    row()
                                        .justify_between()
                                        .items_center()
                                        .child(eyebrow("Done"))
                                        .child(
                                            Button::new(
                                                "todos.show-done",
                                                if show_done { "Hide" } else { "Show" },
                                            )
                                            .ghost()
                                            .small()
                                            .build(
                                                ui,
                                                |this, _, cx| {
                                                    this.show_done = !this.show_done;
                                                    cx.notify();
                                                },
                                            ),
                                        ),
                                )
                                .when(show_done, |s| {
                                    s.child(card().p(px(SPACE_1)).gap_0().children(
                                        done.iter().enumerate().map(|(index, todo)| {
                                            self.todo_row(index, done_count, todo, ui).opacity(0.7)
                                        }),
                                    ))
                                })
                                .when(!show_done, |s| {
                                    s.child(caption(copy::pluralize(
                                        done_count,
                                        "Todo ticked off",
                                        "Todos ticked off",
                                    )))
                                }),
                        )
                    })
                    .into_any_element()
            },
        )))
        .build()
    }
}
