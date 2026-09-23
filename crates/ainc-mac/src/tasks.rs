use crate::{
    input::{Submit, TextInput},
    storage::{Store, Todo},
    style::*,
};
use gpui::{prelude::*, *};
use std::rc::Rc;

#[derive(Clone)]
enum Dialog {
    Add,
    Confirm(Todo),
}

pub struct Tasks {
    store: Option<Rc<Store>>,
    todos: Vec<Todo>,
    input: Entity<TextInput>,
    error: Option<String>,
    dialog: Option<Dialog>,
    menu: Option<i64>,
    hovered_row: Option<i64>,
    hover: HoverFade,
    _subscriptions: Vec<Subscription>,
}
impl Tasks {
    pub fn new(
        store: Option<Rc<Store>>,
        storage_error: Option<String>,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| TextInput::field("Task title", false, cx));
        let subscriptions = vec![
            cx.subscribe(&input, |this, _, _: &Submit, cx| this.add(cx)),
            cx.observe(&input, |_, _, cx| cx.notify()),
        ];
        let mut this = Self {
            store,
            todos: vec![],
            input,
            error: storage_error,
            dialog: None,
            menu: None,
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
    fn open_add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.menu = None;
        self.hovered_row = None;
        self.dialog = Some(Dialog::Add);
        self.input.update(cx, |input, cx| {
            input.reset();
            cx.notify();
        });
        window.focus(&self.input.focus_handle(cx));
        cx.notify();
    }
    pub fn dismiss(&mut self, cx: &mut Context<Self>) -> bool {
        let open = self.dialog.take().is_some() || self.menu.take().is_some();
        if open {
            cx.notify();
        }
        open
    }
    fn add(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.dialog, Some(Dialog::Add)) {
            return;
        }
        let Some(store) = &self.store else {
            return;
        };
        let title = self.input.read(cx).content.trim().to_owned();
        if title.is_empty() {
            return;
        }
        match store.add_todo(&title) {
            Ok(()) => {
                self.dialog = None;
                self.error = None;
                self.reload();
            }
            Err(error) => self.error = Some(format!("Task was not saved: {error}")),
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
                self.dialog = None;
                self.menu = None;
                self.error = None;
                self.reload();
            }
            Err(_) => self.error = Some("Task change could not be saved. Please try again.".into()),
        }
        cx.notify();
    }
    fn button(
        &self,
        id: impl Into<ElementId>,
        f: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + Clone + 'static,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let key = f.clone();
        let id = id.into();
        let hover_id = id.clone();
        let background = self.hover.color(&id);
        row()
            .id(id)
            .tab_index(0)
            .cursor_pointer()
            .justify_center()
            .rounded(px(6.))
            .h(px(32.))
            .px(px(10.))
            .bg(background)
            .on_hover(cx.listener(move |this, over, _, cx| {
                this.hover.set(hover_id.clone(), *over);
                cx.notify();
            }))
            .focus(|s| s.bg(rgb(0x252525)))
            .on_click(cx.listener(move |this, _, window, cx| {
                cx.stop_propagation();
                f(this, window, cx);
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "enter" || event.keystroke.key == "space" {
                    cx.stop_propagation();
                    key(this, window, cx);
                }
            }))
    }
    pub fn overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let dialog = self.dialog.clone()?;
        let is_add = matches!(dialog, Dialog::Add);
        let title = match &dialog {
            Dialog::Add => "Add task".to_owned(),
            Dialog::Confirm(todo) => format!("Delete “{}”?", todo.title),
        };
        let target = match dialog {
            Dialog::Confirm(todo) => Some(todo.id),
            Dialog::Add => None,
        };
        Some(
            div()
                .id("task-dialog-backdrop")
                .absolute()
                .inset_0()
                .cursor_default()
                .bg(rgba(0x000000aa))
                .flex()
                .items_center()
                .justify_center()
                .on_mouse_move(|_, _, cx| cx.stop_propagation())
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.dismiss(cx);
                }))
                .child(
                    panel()
                        .id("task-dialog")
                        .w(px(440.))
                        .p(px(24.))
                        .gap(px(20.))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(|_, _, cx| cx.stop_propagation())
                        .child(
                            div()
                                .text_size(px(18.))
                                .font_weight(FontWeight::MEDIUM)
                                .child(title),
                        )
                        .when(is_add, |s| {
                            s.child(
                                row()
                                    .h(px(42.))
                                    .px(px(12.))
                                    .border_1()
                                    .border_color(rgb(BORDER))
                                    .rounded(px(7.))
                                    .child(self.input.clone()),
                            )
                        })
                        .when(!is_add, |s| {
                            s.child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(rgb(MUTED))
                                    .child("This task will be permanently removed."),
                            )
                        })
                        .child(
                            row()
                                .justify_end()
                                .gap(px(8.))
                                .child(
                                    self.button(
                                        "task-cancel",
                                        |this, _, cx| {
                                            this.dismiss(cx);
                                        },
                                        cx,
                                    )
                                    .border_1()
                                    .border_color(rgb(BORDER))
                                    .child("Cancel"),
                                )
                                .child(
                                    self.button(
                                        "task-submit",
                                        move |this, _, cx| {
                                            if let Some(id) = target {
                                                this.change(id, None, cx);
                                            } else {
                                                this.add(cx);
                                            }
                                        },
                                        cx,
                                    )
                                    .bg(rgb(if is_add { 0xe8e8e8 } else { 0x5b2b2b }))
                                    .text_color(rgb(if is_add { 0x141414 } else { TEXT }))
                                    .child(if is_add {
                                        "Create"
                                    } else {
                                        "Delete task"
                                    }),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }
}
impl Render for Tasks {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.hover.animate(window);
        column()
            .id("tasks-page")
            .gap(px(24.))
            .on_click(cx.listener(|this, _, _, cx| {
                if this.menu.take().is_some() {
                    cx.notify();
                }
            }))
            .child(
                row().child(div().flex_1()).child(
                    self.button("add-task", Self::open_add, cx)
                        .border_1()
                        .border_color(rgb(0x555555))
                        .text_size(px(12.))
                        .child("Add task"),
                ),
            )
            .when_some(self.error.clone(), |s, error| {
                s.child(
                    div()
                        .text_size(px(12.))
                        .text_color(rgb(0xe6acac))
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
                                    .text_size(px(11.))
                                    .text_color(rgb(MUTED))
                                    .child(if completed { "Completed" } else { "To do" }),
                            )
                            .children(
                                self.todos
                                    .iter()
                                    .filter(move |todo| todo.completed == completed)
                                    .map(|todo| {
                                        let id = todo.id;
                                        let menu_open = self.menu == Some(id);
                                        row()
                                            .id(("task-row", id as u64))
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
                                                    move |this, _, cx| {
                                                        this.change(id, Some(!completed), cx)
                                                    },
                                                    cx,
                                                )
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
                                                        .text_size(px(12.))
                                                        .text_color(rgb(0x88b69b))
                                                        .child(if completed { "✓" } else { "" }),
                                                ),
                                            )
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .text_size(px(12.))
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
                                                    move |this, _, cx| {
                                                        this.menu = if this.menu == Some(id) {
                                                            None
                                                        } else {
                                                            Some(id)
                                                        };
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
                                                    panel()
                                                        .absolute()
                                                        .right(px(4.))
                                                        .top(px(42.))
                                                        .w(px(140.))
                                                        .p(px(4.))
                                                        .child(
                                                            self.button(
                                                                ("delete", id as u64),
                                                                move |this, _, cx| {
                                                                    this.menu = None;
                                                                    this.dialog = this
                                                                        .todos
                                                                        .iter()
                                                                        .find(|t| t.id == id)
                                                                        .cloned()
                                                                        .map(Dialog::Confirm);
                                                                    cx.notify();
                                                                },
                                                                cx,
                                                            )
                                                            .w_full()
                                                            .justify_start()
                                                            .text_size(px(12.))
                                                            .text_color(rgb(0xdaa7a7))
                                                            .child("Delete"),
                                                        ),
                                                )
                                            })
                                    }),
                            )
                    }),
            )
    }
}
