use crate::{
    input::{Submit, TextInput},
    storage::{Store, Todo},
    style::*,
};
use gpui::{prelude::*, *};
use std::{rc::Rc, time::Instant};

pub struct Tasks {
    store: Option<Rc<Store>>,
    todos: Vec<Todo>,
    input: Entity<TextInput>,
    error: Option<String>,
    appearance: Option<Instant>,
    reduced_motion: bool,
    hover: HoverFade,
    _subscriptions: Vec<Subscription>,
}
impl Tasks {
    pub fn new(
        store: Option<Rc<Store>>,
        storage_error: Option<String>,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| TextInput::field("Add a task…", false, cx));
        let subscriptions = vec![
            cx.subscribe(&input, |this, _, _: &Submit, cx| this.add(cx)),
            cx.observe(&input, |_, _, cx| cx.notify()),
        ];
        let mut this = Self {
            store,
            todos: vec![],
            input,
            error: storage_error,
            appearance: None,
            reduced_motion: reduced_motion(),
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
                        Some("Could not read tasks. Check database access and restart.".into())
                }
            }
        }
    }
    pub fn summary(&self) -> String {
        let open = self.todos.iter().filter(|t| !t.completed).count();
        if open == 0 {
            "All caught up".into()
        } else {
            format!("{open} task{} to do", if open == 1 { "" } else { "s" })
        }
    }
    fn add(&mut self, cx: &mut Context<Self>) {
        let Some(store) = &self.store else {
            return;
        };
        let title = self.input.read(cx).content.trim().to_owned();
        if title.is_empty() {
            return;
        }
        match store.add_todo(&title) {
            Ok(()) => {
                self.input.update(cx, |input, cx| {
                    input.reset();
                    cx.notify();
                });
                self.error = None;
                self.appearance = Some(Instant::now());
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
        f: impl Fn(&mut Self, &mut Context<Self>) + Clone + 'static,
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
            .focus(|s| s.bg(rgb(0x1d2520)))
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                f(this, cx);
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "enter" || event.keystroke.key == "space" {
                    cx.stop_propagation();
                    key(this, cx);
                }
            }))
    }
}
impl Render for Tasks {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.hover.animate(window);
        let progress = if self.reduced_motion {
            1.
        } else if let Some(start) = self.appearance {
            let t = (start.elapsed().as_secs_f32() / 0.22).min(1.);
            if t < 1. {
                window.request_animation_frame();
            } else {
                self.appearance = None;
            }
            1. - (1. - t).powi(3)
        } else {
            1.
        };
        column()
            .gap(px(24.))
            .child(
                row()
                    .child(
                        div()
                            .text_size(px(26.))
                            .font_weight(FontWeight::MEDIUM)
                            .child("Tasks"),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(rgb(MUTED))
                            .child(self.summary()),
                    ),
            )
            .child(
                row()
                    .gap(px(12.))
                    .p(px(10.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .child(div().flex_1().min_w_0().child(self.input.clone()))
                    .child(
                        self.button("add-task", Self::add, cx)
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
                                    .enumerate()
                                    .map(|(index, todo)| {
                                        let id = todo.id;
                                        row()
                                            .gap(px(12.))
                                            .min_h(px(52.))
                                            .py(px(8.))
                                            .border_b_1()
                                            .border_color(rgb(0x1a1a1a))
                                            .when(!completed && index == 0, |s| {
                                                s.relative()
                                                    .top(px(3. * (1. - progress)))
                                                    .opacity(0.4 + 0.6 * progress)
                                            })
                                            .child(
                                                self.button(
                                                    ("complete", id as u64),
                                                    move |this, cx| {
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
                                                    ("delete", id as u64),
                                                    move |this, cx| this.change(id, None, cx),
                                                    cx,
                                                )
                                                .text_size(px(11.))
                                                .text_color(rgb(0xdaa7a7))
                                                .child("Delete"),
                                            )
                                    }),
                            )
                    }),
            )
    }
}
