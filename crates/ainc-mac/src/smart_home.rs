//! The Smart Home page: the thermostat and every light, arranged by room.
use crate::{
    calendar::{ago, now},
    home::{DEFAULT_HIGH, DEFAULT_LOW, DEFAULT_TARGET, HomeModel, glyph, tile_state},
    model::{OpenRoute, Route},
    ui::*,
};
use ainc_client::types::{
    ActionState, ActionView, ClimateMode, HomeClimate, HomeSnapshot, SwitchKey,
};
use gpui::{prelude::*, *};

/// Three cards of two lights each: the whole home, the kitchen's fixtures and
/// the lamps room by room. Equal cards keep every tile the same size.
type Room = (&'static str, &'static [(SwitchKey, &'static str)]);
const ROOMS: [Room; 3] = [
    (
        "Everywhere",
        &[
            (SwitchKey::All, "All lights"),
            (SwitchKey::Lamps, "All lamps"),
        ],
    ),
    (
        "Kitchen",
        &[
            (SwitchKey::KitchenCeiling, "Ceiling"),
            (SwitchKey::UnderCabinet, "Under cabinet"),
        ],
    ),
    (
        "Lamps",
        &[
            (SwitchKey::LivingRoomLamps, "Living room"),
            (SwitchKey::BedroomLamps, "Bedroom"),
        ],
    ),
];
pub const MODES: [(ClimateMode, &str); 4] = [
    (ClimateMode::Off, "Off"),
    (ClimateMode::Cool, "Cool"),
    (ClimateMode::Heat, "Heat"),
    (ClimateMode::HeatCool, "Auto"),
];

/// "72°" from a reading such as 71.6.
pub fn degrees(value: f64) -> String {
    format!("{}°", value.round() as i64)
}
/// What the thermostat is doing, and the tone that says it.
pub fn climate_status(climate: &HomeClimate) -> (String, Tone) {
    if climate.pending {
        return ("Updating…".into(), Tone::Neutral);
    }
    let low = climate.target_low.unwrap_or(DEFAULT_LOW);
    let high = climate.target_high.unwrap_or(DEFAULT_HIGH);
    let target = climate.target.unwrap_or(DEFAULT_TARGET);
    // In Auto, cooling heads for the top of the band and heating for the bottom.
    let (cool_to, heat_to, holding) = match climate.mode {
        ClimateMode::Off => return ("Thermostat off".into(), Tone::Neutral),
        ClimateMode::HeatCool => (high, low, format!("{low}–{high}°")),
        _ => (target, target, format!("{target}°")),
    };
    match climate.action.as_deref() {
        Some("Cooling") => (format!("Cooling to {cool_to}°"), Tone::Info),
        Some("Heating") => (format!("Heating to {heat_to}°"), Tone::Warning),
        _ => (format!("Holding {holding}"), Tone::Neutral),
    }
}
/// The host a connection points at.
pub fn host(base_url: &str) -> String {
    base_url
        .split("://")
        .nth(1)
        .unwrap_or(base_url)
        .trim_end_matches('/')
        .to_owned()
}
/// Three room cards side by side when they fit, then two, then one.
fn room_columns(window: &Window) -> usize {
    let width = window.viewport_size().width;
    if width >= px(ROOMS_THREE_ACROSS) {
        3
    } else if width >= px(CLIMATE_STACK_BELOW) {
        2
    } else {
        1
    }
}
/// How many of a room's lights are on: a group already counts its lights
/// (Everywhere counts the whole home); otherwise count the room's switches.
fn room_lights(snapshot: &HomeSnapshot, switches: &[(SwitchKey, &str)]) -> (i64, i64) {
    let states: Vec<_> = switches
        .iter()
        .filter_map(|(key, _)| snapshot.switches.iter().find(|s| s.key == *key))
        .collect();
    match states.iter().max_by_key(|s| s.total) {
        Some(group) if group.total > 1 => (group.lit, group.total),
        _ => (
            states.iter().filter(|s| s.on && !s.pending).count() as i64,
            states.len() as i64,
        ),
    }
}
/// One pattern for every room: "On", "N of M on" or "Off".
pub fn room_summary(lit: i64, total: i64) -> String {
    match (lit, total) {
        (0, _) => "Off".into(),
        (lit, total) if lit == total => "On".into(),
        (lit, total) => format!("{lit} of {total} on"),
    }
}

pub struct SmartHomePage {
    home: Entity<HomeModel>,
    hover: HoverFade,
    _observe: Subscription,
}
impl EventEmitter<OpenRoute> for SmartHomePage {}
impl HoverHost for SmartHomePage {
    fn hover_fade(&mut self) -> &mut HoverFade {
        &mut self.hover
    }
}

impl SmartHomePage {
    pub fn new(home: Entity<HomeModel>, cx: &mut Context<Self>) -> Self {
        Self {
            _observe: cx.observe(&home, |_, _, cx| cx.notify()),
            home,
            hover: HoverFade::default(),
        }
    }

    fn switch(&mut self, key: SwitchKey, on: bool, cx: &mut Context<Self>) {
        self.home.update(cx, |home, cx| home.switch(key, on, cx));
    }

    /// The thermostat's scale, a little wider than what it accepts, with the
    /// indoor reading as a hollow mark and the setpoints as handles.
    fn track(climate: &HomeClimate) -> Div {
        let (low_end, high_end) = ((climate.min - 3) as f64, (climate.max + 3) as f64);
        let along = |value: f64| ((value - low_end) / (high_end - low_end)).clamp(0., 1.) as f32;
        let middle = (TRACK_HANDLE - TRACK_HEIGHT) / 2.;
        let mut rail = div().relative().w_full().h(px(TRACK_HANDLE)).child(
            div()
                .absolute()
                .top(px(middle))
                .left_0()
                .right_0()
                .h(px(TRACK_HEIGHT))
                .rounded_full()
                .bg(rgb(BORDER_STRONG)),
        );
        let handle = |at: f32| {
            div()
                .absolute()
                .top_0()
                .left(relative(at))
                .ml(px(-TRACK_HANDLE / 2.))
                .size(px(TRACK_HANDLE))
                .rounded_full()
                .bg(rgb(PRIMARY))
                .border_2()
                .border_color(rgb(SURFACE_RAISED))
        };
        match climate.mode {
            ClimateMode::Off => {}
            ClimateMode::HeatCool => {
                let low = along(climate.target_low.unwrap_or(DEFAULT_LOW) as f64);
                let high = along(climate.target_high.unwrap_or(DEFAULT_HIGH) as f64);
                rail = rail
                    .child(
                        div()
                            .absolute()
                            .top(px(middle))
                            .left(relative(low))
                            .w(relative(high - low))
                            .h(px(TRACK_HEIGHT))
                            .bg(rgb(TEXT_TERTIARY)),
                    )
                    .child(handle(low))
                    .child(handle(high));
            }
            _ => {
                rail = rail.child(handle(along(
                    climate.target.unwrap_or(DEFAULT_TARGET) as f64
                )))
            }
        }
        // The reading's mark is left out when it would sit on a setpoint handle;
        // its label under the track still says where it is.
        let setpoints: Vec<i64> = match climate.mode {
            ClimateMode::Off => vec![],
            ClimateMode::HeatCool => vec![
                climate.target_low.unwrap_or(DEFAULT_LOW),
                climate.target_high.unwrap_or(DEFAULT_HIGH),
            ],
            _ => vec![climate.target.unwrap_or(DEFAULT_TARGET)],
        };
        let clear = |ambient: f64| {
            setpoints
                .iter()
                .all(|point| (ambient - *point as f64).abs() >= 1.)
        };
        // The reading is placed where its label says: at the rounded degree.
        let reading = climate.ambient.map(f64::round);
        if let Some(ambient) = reading.filter(|ambient| clear(*ambient)) {
            rail = rail.child(
                div()
                    .debug_selector(|| "home.climate.now".into())
                    .absolute()
                    .top(px((TRACK_HANDLE - TRACK_MARK) / 2.))
                    .left(relative(along(ambient)))
                    .ml(px(-TRACK_MARK / 2.))
                    .size(px(TRACK_MARK))
                    .rounded_full()
                    .bg(rgb(SURFACE_RAISED))
                    .border_2()
                    .border_color(rgb(TEXT)),
            );
        }
        column()
            .debug_selector(|| "home.climate.track".into())
            .w_full()
            .gap(px(SPACE_2))
            .child(rail)
            .child(
                // The scale's ends, and a label under the hollow mark for the reading.
                div()
                    .relative()
                    .w_full()
                    .h(type_size(CAPTION_SIZE))
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .child(hint(format!("{}°", low_end as i64))),
                    )
                    .child(
                        div()
                            .absolute()
                            .right_0()
                            .child(hint(format!("{}°", high_end as i64))),
                    )
                    .when_some(reading, |s, ambient| {
                        s.child(
                            div()
                                .absolute()
                                .left(relative(along(ambient)))
                                .ml(px(-TRACK_LABEL_HALF))
                                .w(px(TRACK_LABEL_HALF * 2.))
                                .flex()
                                .justify_center()
                                .child(hint(format!("Now {}", degrees(ambient)))),
                        )
                    }),
            )
    }

    /// A stepper with its label on the same line.
    #[allow(clippy::too_many_arguments)]
    fn setpoint(
        &self,
        id: &'static str,
        label: &'static str,
        value: i64,
        can_lower: bool,
        can_raise: bool,
        step: impl Fn(&mut HomeModel, i64, &mut Context<HomeModel>) + Clone + 'static,
        cx: &mut Context<Self>,
    ) -> Div {
        row().gap(px(SPACE_3)).child(hint(label)).child(stepper(
            id,
            label,
            format!("{value}°"),
            can_lower,
            can_raise,
            &self.hover,
            move |this: &mut Self, delta, _, cx| {
                let step = step.clone();
                this.home.update(cx, |home, cx| step(home, delta, cx))
            },
            cx,
        ))
    }

    fn climate(&self, climate: &HomeClimate, stacked: bool, cx: &mut Context<Self>) -> Div {
        let (status, tone) = climate_status(climate);
        let mode = MODES
            .iter()
            .position(|(mode, _)| *mode == climate.mode)
            .unwrap_or(0);
        let (min, max, gap) = (climate.min, climate.max, climate.gap);
        let controls: AnyElement = match climate.mode {
            ClimateMode::Off => hint("Choose a mode to set a temperature.").into_any_element(),
            ClimateMode::HeatCool => {
                let low = climate.target_low.unwrap_or(DEFAULT_LOW);
                let high = climate.target_high.unwrap_or(DEFAULT_HIGH);
                row()
                    .gap(px(SPACE_6))
                    .child(self.setpoint(
                        "home.climate.low",
                        "Heat to",
                        low,
                        low > min,
                        high - low > gap,
                        |home, delta, cx| home.step_range(false, delta, cx),
                        cx,
                    ))
                    .child(self.setpoint(
                        "home.climate.high",
                        "Cool to",
                        high,
                        high - low > gap,
                        high < max,
                        |home, delta, cx| home.step_range(true, delta, cx),
                        cx,
                    ))
                    .into_any_element()
            }
            _ => {
                let target = climate.target.unwrap_or(DEFAULT_TARGET);
                self.setpoint(
                    "home.climate.target",
                    "Target",
                    target,
                    target > min,
                    target < max,
                    |home, delta, cx| home.step_target(delta, cx),
                    cx,
                )
                .into_any_element()
            }
        };
        let card = if stacked {
            column()
        } else {
            row().items_start()
        };
        card.debug_selector(|| "home.climate".into())
            .w_full()
            .gap(px(if stacked { SPACE_5 } else { SPACE_8 }))
            .p(px(CARD_INSET))
            .pt(px(CARD_INSET - EYEBROW_OPTICAL_LIFT))
            .rounded(px(RADIUS_LG))
            .border_1()
            .border_color(rgb(BORDER))
            .bg(rgb(SURFACE_RAISED))
            .child(
                // A fixed column, so the controls never move when the mode does;
                // it only gives way when the window is too narrow for both.
                column()
                    .w(px(READING_WIDTH))
                    .min_w(px(TILE_MIN_WIDTH))
                    .flex_shrink(1.)
                    .gap(px(SPACE_3))
                    .child(
                        row()
                            .gap(px(SPACE_2))
                            .child(icon("thermometer", ICON_SIZE_SM))
                            .child(eyebrow("Inside")),
                    )
                    .child(hero(climate.ambient.map_or_else(|| "—".into(), degrees)))
                    .child(
                        row()
                            .gap(px(SPACE_2))
                            .text_size(type_size(LABEL_SIZE))
                            .text_color(rgb(TEXT_SECONDARY))
                            .child(status_dot(tone))
                            .child(div().truncate().child(status)),
                    ),
            )
            .child(
                column()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .justify_between()
                    .gap(px(SPACE_6))
                    .child(
                        row()
                            .w_full()
                            .flex_wrap()
                            .justify_between()
                            .gap(px(SPACE_4))
                            .child(segmented(
                                "home.climate.mode",
                                MODES.iter().map(|(_, label)| *label),
                                mode,
                                true,
                                &self.hover,
                                |this: &mut Self, index, _, cx| {
                                    this.home
                                        .update(cx, |home, cx| home.set_mode(MODES[index].0, cx))
                                },
                                cx,
                            ))
                            .child(controls),
                    )
                    .child(Self::track(climate)),
            )
    }

    fn rooms(&self, snapshot: &HomeSnapshot, columns: usize, cx: &mut Context<Self>) -> Div {
        let enabled = snapshot.reachable;
        let mut cards: Vec<AnyElement> = vec![];
        for (room, switches) in ROOMS {
            let (lit, total) = room_lights(snapshot, switches);
            let mut tiles = row().w_full().gap(px(SPACE_3));
            for (key, label) in switches.iter().copied() {
                let state = tile_state(snapshot, key);
                let on = state.on;
                let mut tile = SwitchTile::new(
                    SharedString::from(format!("home.switch.{key}")),
                    label,
                    glyph(key),
                )
                .on(on)
                .mixed(state.mixed)
                .pending(state.pending)
                .enabled(enabled);
                if let Some(detail) = state.detail {
                    tile = tile.detail(detail);
                }
                tiles = tiles.child(tile.build(
                    &self.hover,
                    move |this: &mut Self, _, cx| this.switch(key, !on, cx),
                    cx,
                ));
            }
            cards.push(
                column()
                    .flex_1()
                    .min_w_0()
                    .gap(px(SPACE_4))
                    .p(px(CARD_INSET))
                    // The title's cap height, not its line box, sits on the inset.
                    .pt(px(CARD_INSET - EYEBROW_OPTICAL_LIFT))
                    .rounded(px(RADIUS_LG))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .child(
                        row()
                            .justify_between()
                            .gap(px(SPACE_2))
                            .line_height(relative(1.))
                            .child(
                                div()
                                    .text_size(type_size(BODY_SIZE))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(room),
                            )
                            .child(hint(room_summary(lit, total))),
                    )
                    .child(tiles)
                    .into_any_element(),
            );
        }
        // A true column grid: short last rows keep empty cells, so edges align.
        let mut grid = column()
            .debug_selector(|| "home.rooms".into())
            .w_full()
            .gap(px(SPACE_4));
        let mut cards = cards.into_iter().peekable();
        while cards.peek().is_some() {
            let mut line = row().w_full().items_stretch().gap(px(SPACE_4));
            for _ in 0..columns {
                line = line.child(match cards.next() {
                    Some(card) => card,
                    // An empty cell takes a card's insets, so it is exactly as wide.
                    None => div()
                        .flex_1()
                        .min_w_0()
                        .p(px(CARD_INSET))
                        .border_1()
                        .border_color(rgba(SHELL << 8))
                        .into_any_element(),
                });
            }
            grid = grid.child(line);
        }
        grid
    }

    fn history(&self, actions: &[ActionView]) -> Div {
        let now = now();
        let mut rows = column();
        for (index, action) in actions.iter().enumerate() {
            if index > 0 {
                rows = rows.child(divider());
            }
            // Done is the norm; only waiting and failed changes are marked.
            let (mark, state) = match action.state {
                ActionState::Completed => (None, ago(action.created_at, now)),
                ActionState::Superseded => {
                    (None, format!("Replaced · {}", ago(action.created_at, now)))
                }
                ActionState::Failed => (
                    Some(Tone::Danger),
                    format!("Failed · {}", ago(action.created_at, now)),
                ),
                ActionState::Queued | ActionState::Running => {
                    (Some(Tone::Info), "Applying…".to_owned())
                }
            };
            rows = rows.child(
                row()
                    .min_h(px(LIST_ROW_HEIGHT))
                    .px(px(CARD_INSET))
                    .gap(px(SPACE_3))
                    .child(
                        column()
                            .flex_1()
                            .min_w_0()
                            .child(
                                row()
                                    .gap(px(SPACE_2))
                                    .when_some(mark, |s, tone| s.child(status_dot(tone)))
                                    .child(div().truncate().child(action.summary.clone())),
                            )
                            .when_some(action.error.clone(), |s, error| {
                                s.child(
                                    div()
                                        .truncate()
                                        .text_size(type_size(CAPTION_SIZE))
                                        .text_color(rgb(ERROR))
                                        .child(error),
                                )
                            }),
                    )
                    .child(hint(state)),
            );
        }
        column()
            .gap(px(SPACE_3))
            .child(eyebrow("Recent changes"))
            .child(
                column()
                    .debug_selector(|| "home.history".into())
                    .rounded(px(RADIUS_LG))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(SURFACE_RAISED))
                    .child(rows),
            )
    }
}

impl Render for SmartHomePage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.hover.animate(window);
        let (snapshot, error) = {
            let home = self.home.read(cx);
            (home.snapshot.clone(), home.error.clone())
        };
        let connected = snapshot.as_ref().is_some_and(|s| s.connection.is_some());
        let reachable = snapshot.as_ref().is_some_and(|s| s.reachable);
        let description = match (connected, reachable) {
            (true, false) => "Your control center can't be reached right now.",
            _ => "Lights and the thermostat, from your World Wide Webb control center.",
        };
        let mut page = Page::document(PageHeader::new("Smart Home").description(description));
        match snapshot {
            None => {
                page = page.child(match error {
                    Some(error) => banner(Tone::Danger, error).into_any_element(),
                    None => skeleton_rows("home.loading", 4).into_any_element(),
                })
            }
            Some(snapshot) if snapshot.connection.is_none() => {
                page = page.child(
                    EmptyState::new("home", "Connect your control center")
                        .description("Add its address in Settings to switch lights and set the thermostat here.")
                        .action(
                            Button::new("home.connect", "Open Settings")
                                .primary()
                                .build(
                                    &self.hover,
                                    |_: &mut Self, _, cx| cx.emit(OpenRoute(Route::Settings)),
                                    cx,
                                ),
                        )
                        .selector("home.empty")
                        .build(),
                );
            }
            Some(snapshot) => {
                if let Some(error) = snapshot.error.clone() {
                    page = page
                        .child(banner(Tone::Danger, error).debug_selector(|| "home.error".into()));
                }
                if let Some(climate) = &snapshot.climate {
                    // Narrow windows stack the reading above the controls.
                    let stacked = window.viewport_size().width < px(CLIMATE_STACK_BELOW);
                    page = page.child(self.climate(climate, stacked, cx));
                }
                page = page.child(
                    column()
                        .gap(px(SPACE_3))
                        .child(eyebrow("Lights"))
                        .child(self.rooms(&snapshot, room_columns(window), cx)),
                );
                if !snapshot.actions.is_empty() {
                    page = page.child(self.history(&snapshot.actions));
                }
            }
        }
        div()
            .id("home.page")
            .debug_selector(|| "home.page".into())
            .size_full()
            .child(page.build())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::core::prelude::v1::test;

    #[test]
    fn climate_reads_as_a_sentence() {
        let mut climate = crate::home::fixtures::connected().climate.unwrap();
        assert_eq!(
            climate_status(&climate),
            ("Cooling to 72°".into(), Tone::Info)
        );
        climate.mode = ClimateMode::HeatCool;
        climate.action = Some("Idle".into());
        climate.target_low = Some(68);
        climate.target_high = Some(74);
        assert_eq!(climate_status(&climate).0, "Holding 68–74°");
        climate.mode = ClimateMode::Off;
        assert_eq!(climate_status(&climate).0, "Thermostat off");
        assert_eq!(degrees(71.6), "72°");
        assert_eq!(room_summary(4, 4), "On");
        assert_eq!(room_summary(3, 4), "3 of 4 on");
        assert_eq!(room_summary(1, 1), "On");
        assert_eq!(room_summary(0, 2), "Off");
        assert_eq!(
            host("https://app.worldwidewebb.co/"),
            "app.worldwidewebb.co"
        );
    }
}
