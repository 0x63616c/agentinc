//! The model picker shared by Settings and the composer: grouped by provider,
//! featured models first. One element ID set, so tests find it on either page.
use crate::{
    providers::{ProviderModel, ProviderStatus, ProvidersState},
    ui::*,
};
use gpui::{prelude::*, *};
use std::time::Duration;

#[derive(Default)]
pub struct ModelMenu {
    open: bool,
    closing: bool,
    generation: u64,
}
impl ModelMenu {
    pub fn toggle<V: 'static>(
        &mut self,
        access: impl Fn(&mut V) -> &mut ModelMenu + 'static,
        cx: &mut Context<V>,
    ) {
        if self.open {
            self.close(access, cx);
        } else {
            self.generation += 1;
            self.open = true;
            self.closing = false;
            cx.notify();
        }
    }
    /// Fade out, then stop rendering once the fade has finished.
    pub fn close<V: 'static>(
        &mut self,
        access: impl Fn(&mut V) -> &mut ModelMenu + 'static,
        cx: &mut Context<V>,
    ) {
        if !self.open {
            return;
        }
        self.open = false;
        self.closing = true;
        self.generation += 1;
        let generation = self.generation;
        let timer = cx
            .background_executor()
            .timer(Duration::from_millis(PANEL_MS));
        cx.spawn(async move |this, cx| {
            timer.await;
            let _ = this.update(cx, |this, cx| {
                let menu = access(this);
                if menu.generation == generation {
                    menu.closing = false;
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }
}

pub fn models(providers: &Option<ProvidersState>) -> Vec<(&ProviderStatus, &ProviderModel)> {
    providers
        .iter()
        .flat_map(|state| state.providers.iter())
        .flat_map(|provider| provider.models.iter().map(move |model| (provider, model)))
        .collect()
}
pub fn label(providers: &Option<ProvidersState>, current: &Option<String>) -> String {
    current
        .as_ref()
        .and_then(|id| {
            models(providers)
                .into_iter()
                .find(|(_, model)| &model.id == id)
                .map(|(provider, model)| format!("{} · {}", model.name, provider.name))
        })
        .unwrap_or_else(|| "Choose a model".into())
}
/// The first connected model whose name or ID contains the text.
pub fn find(providers: &Option<ProvidersState>, needle: &str) -> Option<String> {
    let needle = needle.trim().to_lowercase();
    models(providers)
        .into_iter()
        .filter(|(provider, _)| provider.connected)
        .find(|(_, model)| {
            model.name.to_lowercase().contains(&needle) || model.id.to_lowercase().contains(&needle)
        })
        .map(|(_, model)| model.id.clone())
}

/// The trigger with the floating list anchored to it. Settings opens upward
/// from the right edge; the composer opens upward from the left edge.
#[allow(clippy::too_many_arguments)]
pub fn render<V: 'static>(
    menu: &ModelMenu,
    providers: &Option<ProvidersState>,
    current: &Option<String>,
    settings: bool,
    enabled: bool,
    trigger: Stateful<Div>,
    select: impl Fn(&mut V, Option<String>, &mut Window, &mut Context<V>) + Clone + 'static,
    cx: &mut Context<V>,
) -> Div {
    let menu_open = menu.open;
    column()
        .relative()
        .when(settings, |anchor| anchor.w(px(260.)).h(px(CONTROL_HEIGHT)))
        .child(trigger.debug_selector(|| "model-select".into()))
        .when(menu.open || menu.closing, |anchor| {
            let mut index = 0usize;
            let mut list = column()
                .id("model-menu")
                .debug_selector(|| "model-menu".into())
                .absolute()
                .when(settings, |menu| menu.right(px(0.)).bottom(px(36.)))
                .when(!settings, |menu| menu.left(px(0.)).bottom(px(36.)))
                .w(px(300.))
                .max_h(px(340.))
                .overflow_y_scroll()
                .p(px(4.))
                .rounded(px(CONTROL_RADIUS))
                .border_1()
                .border_color(rgb(BORDER_OVERLAY))
                .bg(rgb(SURFACE_MENU))
                .shadow(vec![
                    BoxShadow::new(px(0.), px(8.), rgba(SCRIM).into()).blur_radius(px(24.)),
                ]);
            for provider in providers.iter().flat_map(|state| state.providers.iter()) {
                list = list.child(
                    row()
                        .px(px(10.))
                        .pt(px(8.))
                        .pb(px(4.))
                        .gap(px(6.))
                        .items_center()
                        .text_size(type_size(CAPTION_SIZE))
                        .text_color(rgb(MUTED))
                        .child(
                            div()
                                .font_weight(FontWeight::MEDIUM)
                                .child(provider.name.clone()),
                        )
                        .when(!provider.connected, |header| {
                            header.child("· not connected")
                        }),
                );
                for model in &provider.models {
                    list = list.child(option(
                        ("model", index),
                        model,
                        current.as_deref() == Some(model.id.as_str()),
                        enabled && menu_open && provider.connected,
                        select.clone(),
                        cx,
                    ));
                    index += 1;
                }
            }
            if index == 0 {
                list = list.child(
                    div()
                        .px(px(10.))
                        .py(px(8.))
                        .text_size(type_size(CAPTION_SIZE))
                        .text_color(rgb(MUTED))
                        .child("Connect a provider in Settings to choose a model."),
                );
            }
            anchor.child(
                deferred(list.with_animation(
                    if menu_open {
                        "model-menu-open"
                    } else {
                        "model-menu-close"
                    },
                    Animation::new(Duration::from_millis(PANEL_MS)),
                    move |list, progress| {
                        list.opacity(if menu_open { progress } else { 1. - progress })
                    },
                ))
                .with_priority(1),
            )
        })
}
fn option<V: 'static>(
    id: impl Into<ElementId>,
    model: &ProviderModel,
    selected: bool,
    enabled: bool,
    select: impl Fn(&mut V, Option<String>, &mut Window, &mut Context<V>) + Clone + 'static,
    cx: &mut Context<V>,
) -> Stateful<Div> {
    let name = model.name.clone();
    let choice = Some(model.id.clone());
    let featured = model.featured;
    let selector = format!(
        "settings.model-option.{}",
        name.to_lowercase().replace(' ', "-")
    );
    action_button(
        ButtonSpec {
            id: id.into(),
            label: name.clone().into(),
            kind: ButtonKind::Quiet,
            enabled,
        },
        |button| {
            button
                .debug_selector(move || selector.clone())
                .w_full()
                .min_h(px(34.))
                .px(px(10.))
                .gap(px(10.))
                .rounded(px(CONTROL_RADIUS))
                .bg(rgb(if selected { SELECTED } else { SURFACE_MENU }))
                .hover(|item| item.bg(rgb(HOVER_CONTROL)))
                .text_size(type_size(LABEL_SIZE))
                .child(div().flex_1().min_w_0().truncate().child(name))
                .child(
                    div()
                        .w(px(96.))
                        .flex_shrink_0()
                        .text_size(type_size(CAPTION_SIZE))
                        .text_color(rgb(MUTED))
                        .child(if featured { "Recommended" } else { "" }),
                )
                .child(
                    div()
                        .w(px(16.))
                        .flex_shrink_0()
                        .child(if selected { "✓" } else { "" }),
                )
        },
        move |view: &mut V, window, cx| select(view, choice.clone(), window, cx),
        cx,
    )
}
