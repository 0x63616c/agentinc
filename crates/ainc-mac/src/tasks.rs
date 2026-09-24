use crate::{
    input::{Submit, TextInput},
    overlay::{Overlay, OverlayHost, dialog_shell, menu_shell},
    storage::{Store, Todo},
    style::*,
};
use gpui::{prelude::*, *};
use std::{cell::RefCell, rc::Rc};

pub struct TasksPage {
    store: Option<Rc<Store>>,
    overlays: Rc<RefCell<OverlayHost>>,
    todos: Vec<Todo>,
    input: Entity<TextInput>,
    error: Option<String>,
    form_error: Option<String>,
    add_focus: FocusHandle,
    cancel_focus: FocusHandle,
    submit_focus: FocusHandle,
    hovered_row: Option<i64>,
    hover: HoverFade,
    _subscriptions: Vec<Subscription>,
}
impl TasksPage {
    pub fn new(
        store: Option<Rc<Store>>,
        storage_error: Option<String>,
        overlays: Rc<RefCell<OverlayHost>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let input =
            cx.new(|cx| TextInput::field("Task title", false, cx).identified("tasks.title"));
        let subscriptions = vec![
            cx.subscribe(&input, |this, _, _: &Submit, cx| this.add(cx)),
            cx.observe(&input, |this, input, cx| {
                this.form_error = Self::title_error(&input.read(cx).content).map(str::to_owned);
                cx.notify();
            }),
        ];
        let mut this = Self {
            store,
            overlays,
            todos: vec![],
            input,
            error: storage_error,
            form_error: None,
            add_focus: cx.focus_handle(),
            cancel_focus: cx.focus_handle(),
            submit_focus: cx.focus_handle(),
            hovered_row: None,
            hover: HoverFade::default(),
            _subscriptions: subscriptions,
        };
        this.reload();
        this
    }
    fn reload(&mut self) {
        if let Some(store) = &self.store {
            match store.todos() {
                Ok(todos) => self.todos = todos,
                Err(_) => {
                    self.store = None;
                    self.error =
                        Some("Could not read tasks. Check database access and restart.".into());
                }
            }
        }
    }
    pub fn summary(&self) -> String {
        if self.todos.iter().any(|todo| !todo.completed) {
            "Tasks to do".into()
        } else {
            "All caught up".into()
        }
    }
    fn title_error(title: &str) -> Option<&'static str> {
        (title.trim().chars().count() > 500).then_some("Use 500 characters or fewer.")
    }
    fn open_add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.hovered_row = None;
        self.form_error = None;
        self.input.update(cx, |input, cx| {
            input.reset();
            cx.notify();
        });
        let initial_focus = self.input.focus_handle(cx);
        self.overlays
            .borrow_mut()
            .open(Overlay::AddTask, window, cx, Some(initial_focus));
        cx.notify();
    }
    pub fn focus_handles(&self, cx: &App) -> Vec<FocusHandle> {
        if self.overlays.borrow().active() == Some(Overlay::AddTask) {
            vec![
                self.input.focus_handle(cx),
                self.cancel_focus.clone(),
                self.submit_focus.clone(),
            ]
        } else {
            vec![self.cancel_focus.clone(), self.submit_focus.clone()]
        }
    }
    fn add(&mut self, cx: &mut Context<Self>) {
        if self.overlays.borrow().active() != Some(Overlay::AddTask) {
            return;
        }
        let Some(store) = &self.store else {
            return;
        };
        let title = self.input.read(cx).content.trim().to_owned();
        if title.is_empty() {
            return;
        }
        if let Some(error) = Self::title_error(&title) {
            self.form_error = Some(error.into());
            cx.notify();
            return;
        }
        match store.add_todo(&title) {
            Ok(()) => {
                self.overlays.borrow_mut().close();
                self.error = None;
                self.form_error = None;
                self.reload();
            }
            Err(error) => self.form_error = Some(format!("Task was not saved: {error}")),
        }
        cx.notify();
    }
    fn change(&mut self, id: i64, completed: Option<bool>, cx: &mut Context<Self>) {
        let Some(store) = &self.store else {
            return;
        };
        let result = match completed {
            Some(done) => store.set_completed(id, done),
            None => store.delete_todo(id),
        };
        match result {
            Ok(()) => {
                self.overlays.borrow_mut().close();
                self.error = None;
                self.reload();
            }
            Err(_) => {
                self.form_error = Some("Task change could not be saved. Please try again.".into())
            }
        }
        cx.notify();
    }
    fn button(
        &self,
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        enabled: bool,
        kind: ButtonKind,
        f: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + Clone + 'static,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let id = id.into();
        let hover_id = id.clone();
        let background = self.hover.color(&id);
        let on_hover = cx.listener(move |this, over, _, cx| {
            if enabled {
                this.hover.set(hover_id.clone(), *over);
                cx.notify();
            }
        });
        action_button(
            ButtonSpec {
                id,
                label: label.into(),
                kind,
                enabled,
            },
            |button| {
                button
                    .justify_center()
                    .h(px(CONTROL_HEIGHT))
                    .px(px(10.))
                    .bg(background)
                    .on_hover(on_hover)
            },
            f,
            cx,
        )
    }
    pub fn overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let active = self.overlays.borrow().active()?;
        let (is_add, target, title) = match active {
            Overlay::AddTask => (true, None, "Add task".to_owned()),
            Overlay::DeleteTask(id) => {
                let todo = self.todos.iter().find(|todo| todo.id == id)?;
                (false, Some(id), format!("Delete “{}”?", todo.title))
            }
            _ => return None,
        };
        let invalid =
            is_add && (self.form_error.is_some() || self.input.read(cx).content.trim().is_empty());
        let body = if is_add {
            column()
                .gap(px(6.))
                .child(
                    div()
                        .text_size(px(LABEL_SIZE))
                        .text_color(rgb(MUTED))
                        .child("Task title"),
                )
                .child(
                    row()
                        .h(px(FIELD_HEIGHT))
                        .px(px(12.))
                        .border_1()
                        .border_color(rgb(if self.form_error.is_some() {
                            ERROR_BORDER
                        } else {
                            BORDER
                        }))
                        .rounded(px(FIELD_RADIUS))
                        .child(self.input.clone()),
                )
                .when_some(self.form_error.clone(), |s, error| {
                    s.child(
                        div()
                            .text_size(px(LABEL_SIZE))
                            .text_color(rgb(ERROR))
                            .child(error),
                    )
                })
                .into_any_element()
        } else {
            div()
                .text_size(px(LABEL_SIZE))
                .text_color(rgb(MUTED))
                .child("This task will be permanently removed.")
                .into_any_element()
        };
        let footer = row()
            .justify_end()
            .gap(px(8.))
            .child(
                self.button(
                    "tasks.cancel",
                    "Cancel",
                    true,
                    ButtonKind::Secondary,
                    |this, window, cx| {
                        this.overlays.borrow_mut().dismiss(window, cx);
                        cx.notify();
                    },
                    cx,
                )
                .track_focus(&self.cancel_focus)
                .border_1()
                .border_color(rgb(BORDER))
                .child("Cancel"),
            )
            .child(
                self.button(
                    "tasks.submit",
                    if is_add { "Create task" } else { "Delete task" },
                    !invalid,
                    if is_add {
                        ButtonKind::Primary
                    } else {
                        ButtonKind::Destructive
                    },
                    move |this, _, cx| {
                        if let Some(id) = target {
                            this.change(id, None, cx);
                        } else {
                            this.add(cx);
                        }
                    },
                    cx,
                )
                .track_focus(&self.submit_focus)
                .bg(rgb(if is_add { PRIMARY } else { DESTRUCTIVE }))
                .text_color(rgb(if is_add { PRIMARY_INK } else { TEXT }))
                .child(if is_add { "Create" } else { "Delete task" }),
            );
        Some(dialog_shell(title, body, footer).into_any_element())
    }
}
impl Render for TasksPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.hover.animate(window);
        column()
            .id("tasks-page")
            .gap(px(24.))
            .on_click(cx.listener(|this, _, window, cx| {
                let active = this.overlays.borrow().active();
                if active.is_some_and(Overlay::is_menu) {
                    this.overlays.borrow_mut().dismiss(window, cx);
                    cx.notify();
                }
            }))
            .child(
                row().child(div().flex_1()).child(
                    self.button(
                        "tasks.create",
                        "Add task",
                        true,
                        ButtonKind::Secondary,
                        Self::open_add,
                        cx,
                    )
                    .track_focus(&self.add_focus)
                    .border_1()
                    .border_color(rgb(0x555555))
                    .text_size(px(LABEL_SIZE))
                    .child("Add task"),
                ),
            )
            .when_some(self.error.clone(), |s, error| {
                s.child(
                    div()
                        .text_size(px(LABEL_SIZE))
                        .text_color(rgb(ERROR))
                        .child(error),
                )
            })
            .when(self.todos.is_empty(), |s| {
                s.child(
                    div()
                        .py(px(24.))
                        .text_color(rgb(MUTED))
                        .child("No tasks yet."),
                )
            })
            .children(
                [false, true]
                    .into_iter()
                    .filter(|completed| self.todos.iter().any(|t| t.completed == *completed))
                    .map(|completed| {
                        column()
                            .gap(px(8.))
                            .child(
                                div()
                                    .text_size(px(CAPTION_SIZE))
                                    .text_color(rgb(MUTED))
                                    .child(if completed { "Completed" } else { "To do" }),
                            )
                            .children(
                                self.todos
                                    .iter()
                                    .filter(move |todo| todo.completed == completed)
                                    .map(|todo| {
                                        let id = todo.id;
                                        let menu_open = self.overlays.borrow().active()
                                            == Some(Overlay::TaskMenu(id));
                                        row()
                                            .id(("task-row", id as u64))
                                            .accessibility_id(format!("task.{id}"))
                                            .role(accesskit::Role::ListItem)
                                            .aria_label(todo.title.clone())
                                            .on_hover(cx.listener(move |this, hovered, _, cx| {
                                                if *hovered {
                                                    this.hovered_row = Some(id);
                                                } else if this.hovered_row == Some(id) {
                                                    this.hovered_row = None;
                                                }
                                                cx.notify();
                                            }))
                                            .relative()
                                            .gap(px(12.))
                                            .min_h(px(52.))
                                            .py(px(8.))
                                            .border_b_1()
                                            .border_color(rgb(0x1a1a1a))
                                            .child(
                                                self.button(
                                                    ("complete", id as u64),
                                                    if completed {
                                                        "Reopen task"
                                                    } else {
                                                        "Complete task"
                                                    },
                                                    true,
                                                    ButtonKind::Quiet,
                                                    move |this, _, cx| {
                                                        this.change(id, Some(!completed), cx)
                                                    },
                                                    cx,
                                                )
                                                .accessibility_id(format!("task.{id}.complete"))
                                                .role(accesskit::Role::CheckBox)
                                                .aria_label(todo.title.clone())
                                                .aria_toggled(if completed {
                                                    accesskit::Toggled::True
                                                } else {
                                                    accesskit::Toggled::False
                                                })
                                                .w(px(32.))
                                                .px_0()
                                                .child(
                                                    row()
                                                        .size(px(17.))
                                                        .justify_center()
                                                        .rounded(px(4.))
                                                        .border_1()
                                                        .border_color(rgb(if completed {
                                                            0x88b69b
                                                        } else {
                                                            0x555555
                                                        }))
                                                        .text_size(px(LABEL_SIZE))
                                                        .text_color(rgb(0x88b69b))
                                                        .child(if completed { "✓" } else { "" }),
                                                ),
                                            )
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .text_size(px(LABEL_SIZE))
                                                    .text_color(rgb(if completed {
                                                        MUTED
                                                    } else {
                                                        TEXT
                                                    }))
                                                    .child(todo.title.clone()),
                                            )
                                            .child(
                                                self.button(
                                                    ("more", id as u64),
                                                    "Task actions",
                                                    true,
                                                    ButtonKind::Quiet,
                                                    move |this, window, cx| {
                                                        let active =
                                                            this.overlays.borrow().active();
                                                        if active == Some(Overlay::TaskMenu(id)) {
                                                            this.overlays
                                                                .borrow_mut()
                                                                .dismiss(window, cx);
                                                        } else {
                                                            this.overlays.borrow_mut().open(
                                                                Overlay::TaskMenu(id),
                                                                window,
                                                                cx,
                                                                None,
                                                            );
                                                        }
                                                        cx.notify();
                                                    },
                                                    cx,
                                                )
                                                .w(px(32.))
                                                .px_0()
                                                .opacity(
                                                    if menu_open || self.hovered_row == Some(id) {
                                                        1.
                                                    } else {
                                                        0.
                                                    },
                                                )
                                                .focus(|s| s.opacity(1.))
                                                .text_size(px(18.))
                                                .child("···"),
                                            )
                                            .when(menu_open, |s| {
                                                s.child(
                                                    menu_shell(
                                                        self.button(
                                                            ("delete", id as u64),
                                                            "Delete task",
                                                            true,
                                                            ButtonKind::Quiet,
                                                            move |this, window, cx| {
                                                                if this
                                                                    .todos
                                                                    .iter()
                                                                    .any(|t| t.id == id)
                                                                {
                                                                    this.overlays
                                                                        .borrow_mut()
                                                                        .open(
                                                                            Overlay::DeleteTask(id),
                                                                            window,
                                                                            cx,
                                                                            Some(
                                                                                this.cancel_focus
                                                                                    .clone(),
                                                                            ),
                                                                        );
                                                                }
                                                                cx.notify();
                                                            },
                                                            cx,
                                                        )
                                                        .w_full()
                                                        .justify_start()
                                                        .text_size(px(LABEL_SIZE))
                                                        .text_color(rgb(DESTRUCTIVE_TEXT))
                                                        .child("Delete"),
                                                    )
                                                    .absolute()
                                                    .right(px(4.))
                                                    .top(px(42.)),
                                                )
                                            })
                                    }),
                            )
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::TasksPage;
    #[test]
    fn overlong_title_is_rejected_before_store_submit() {
        assert_eq!(
            TasksPage::title_error(&"A".repeat(501)),
            Some("Use 500 characters or fewer.")
        );
        assert_eq!(TasksPage::title_error(&"A".repeat(500)), None);
    }
}

#[cfg(test)]
mod interaction_tests {
    use super::TasksPage;
    use crate::{
        input,
        overlay::{Overlay, OverlayHost},
        storage::Store,
    };
    use gpui::{
        AppContext, Context, Entity, IntoElement, ParentElement, Render, TestAppContext, Window,
        div,
    };
    use std::{cell::RefCell, rc::Rc};

    struct Harness {
        page: Entity<TasksPage>,
    }
    impl Render for Harness {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let overlay = self.page.update(cx, |page, cx| page.overlay(cx));
            div().child(self.page.clone()).children(overlay)
        }
    }

    #[gpui::test]
    fn task_input_submit_complete_reopen_and_delete(cx: &mut TestAppContext) {
        cx.update(input::bind_keys);
        let dir = tempfile::tempdir_in("target").unwrap();
        let store = Rc::new(Store::open(&dir.path().join("tasks.sqlite3")).unwrap());
        let overlays = Rc::new(RefCell::new(OverlayHost::default()));
        let page = cx.new(|cx| TasksPage::new(Some(store.clone()), None, overlays.clone(), cx));
        let (_, cx) = cx.add_window_view(|_, _| Harness { page: page.clone() });
        cx.update(|window, cx| page.update(cx, |page, cx| page.open_add(window, cx)));
        cx.simulate_input("  Ship café 👋  ");
        cx.simulate_keystrokes("enter");
        let todos = store.todos().unwrap();
        assert_eq!(todos.len(), 1);
        assert_eq!(todos[0].title, "Ship café 👋");
        assert_eq!(overlays.borrow().active(), None);
        page.update(cx, |page, cx| page.change(todos[0].id, Some(true), cx));
        assert!(store.todos().unwrap()[0].completed);
        page.update(cx, |page, cx| page.change(todos[0].id, Some(false), cx));
        assert!(!store.todos().unwrap()[0].completed);
        page.update(cx, |page, cx| page.change(todos[0].id, None, cx));
        assert!(store.todos().unwrap().is_empty());
        cx.update(|window, cx| page.update(cx, |page, cx| page.open_add(window, cx)));
        cx.simulate_input("   ");
        cx.simulate_keystrokes("enter");
        assert!(store.todos().unwrap().is_empty());
        assert_eq!(overlays.borrow().active(), Some(Overlay::AddTask));
    }
}
