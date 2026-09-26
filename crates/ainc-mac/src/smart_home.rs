//! The Smart Home page: the thermostat and every light, arranged by room.
use crate::{
    calendar::{ago, now},
    home::{
        DEFAULT_HIGH, DEFAULT_LOW, DEFAULT_TARGET, HomeModel, RANGE_GAP, SETPOINT_MAX,
        SETPOINT_MIN, glyph, tile_state,
    },
    model::{OpenRoute, Route},
    ui::*,
};
use ainc_client::types::{ActionView, ClimateMode, HomeClimate, HomeSnapshot, SwitchKey};
use gpui::{prelude::*, *};

/// Rooms as a two-by-two grid: the two-light rooms above, the one-light
/// rooms below, each switch named for its place in the room.
type Room = (&'static str, &'static [(SwitchKey, &'static str)]);
const ROOMS: [[Room; 2]; 2] = [
    [
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
    ],
    [
        ("Living room", &[(SwitchKey::LivingRoomLamps, "Lamps")]),
        ("Bedroom", &[(SwitchKey::BedroomLamps, "Lamps")]),
    ],
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
    let target = match climate.mode {
        ClimateMode::Off => return ("Thermostat off".into(), Tone::Neutral),
        ClimateMode::HeatCool => format!(
            "{}–{}°",
            climate.target_low.unwrap_or(DEFAULT_LOW),
            climate.target_high.unwrap_or(DEFAULT_HIGH)
        ),
        _ => format!("{}°", climate.target.unwrap_or(DEFAULT_TARGET)),
    };
    match climate.action.as_deref() {
        Some("Cooling") => (format!("Cooling to {target}"), Tone::Info),
        Some("Heating") => (format!("Heating to {target}"), Tone::Warning),
        _ => (format!("Holding {target}"), Tone::Neutral),
    }
}
/// The host a connection points at, for "Live from …".
pub fn host(base_url: &str) -> String {
    base_url
        .split("://")
        .nth(1)
        .unwrap_or(base_url)
        .trim_end_matches('/')
        .to_owned()
}

/// The thermostat's scale: a little wider than the setpoint band.
const TRACK_MIN: f64 = (SETPOINT_MIN - 3) as f64;
const TRACK_MAX: f64 = (SETPOINT_MAX + 3) as f64;
fn along(value: f64) -> f32 {
    ((value - TRACK_MIN) / (TRACK_MAX - TRACK_MIN)).clamp(0., 1.) as f32
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

    /// The thermostat's scale with the indoor reading and what it is holding to.
    fn track(climate: &HomeClimate) -> Div {
        let mut rail = div().relative().w_full().h(px(TRACK_HANDLE)).child(
            div()
                .absolute()
                .top(px((TRACK_HANDLE - TRACK_HEIGHT) / 2.))
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
                            .top(px((TRACK_HANDLE - TRACK_HEIGHT) / 2.))
                            .left(relative(low))
                            .w(relative(high - low))
                            .h(px(TRACK_HEIGHT))
                            .bg(rgb(TEXT_SECONDARY)),
                    )
                    .child(handle(low))
                    .child(handle(high));
            }
            _ => {
                rail = rail.child(handle(along(
                    climate.target.unwrap_or(DEFAULT_TARGET) as f64
                )));
            }
        }
        if let Some(ambient) = climate.ambient {
            rail = rail.child(
                div()
                    .debug_selector(|| "home.climate.now".into())
                    .absolute()
                    .top(px((TRACK_HANDLE - TRACK_MARK) / 2.))
                    .left(relative(along(ambient)))
                    .ml(px(-TRACK_MARK / 2.))
                    .size(px(TRACK_MARK))
                    .rounded_full()
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
                row()
                    .justify_between()
                    .child(hint(format!("{}°", TRACK_MIN as i64)))
                    .child(hint("○ inside  ● set"))
                    .child(hint(format!("{}°", TRACK_MAX as i64))),
            )
    }

    fn climate(&self, climate: &HomeClimate, cx: &mut Context<Self>) -> Div {
        let (status, tone) = climate_status(climate);
        let mode = MODES
            .iter()
            .position(|(mode, _)| *mode == climate.mode)
            .unwrap_or(0);
        let controls: AnyElement = match climate.mode {
            ClimateMode::Off => caption("Choose a mode to set a temperature.").into_any_element(),
            ClimateMode::HeatCool => {
                let low = climate.target_low.unwrap_or(DEFAULT_LOW);
                let high = climate.target_high.unwrap_or(DEFAULT_HIGH);
                row()
                    .gap(px(SPACE_6))
                    .child(labelled(
                        "Heat to",
                        stepper(
                            "home.climate.low",
                            "heating setpoint",
                            format!("{low}°"),
                            low > SETPOINT_MIN,
                            high - low > RANGE_GAP,
                            &self.hover,
                            |this: &mut Self, delta, _, cx| {
                                this.home
                                    .update(cx, |home, cx| home.step_range(false, delta, cx))
                            },
                            cx,
                        ),
                    ))
                    .child(labelled(
                        "Cool to",
                        stepper(
                            "home.climate.high",
                            "cooling setpoint",
                            format!("{high}°"),
                            high - low > RANGE_GAP,
                            high < SETPOINT_MAX,
                            &self.hover,
                            |this: &mut Self, delta, _, cx| {
                                this.home
                                    .update(cx, |home, cx| home.step_range(true, delta, cx))
                            },
                            cx,
                        ),
                    ))
                    .into_any_element()
            }
            _ => {
                let target = climate.target.unwrap_or(DEFAULT_TARGET);
                labelled(
                    "Target",
                    stepper(
                        "home.climate.target",
                        "target temperature",
                        format!("{target}°"),
                        target > SETPOINT_MIN,
                        target < SETPOINT_MAX,
                        &self.hover,
                        |this: &mut Self, delta, _, cx| {
                            this.home.update(cx, |home, cx| home.step_target(delta, cx))
                        },
                        cx,
                    ),
                )
                .into_any_element()
            }
        };
        row()
            .debug_selector(|| "home.climate".into())
            .w_full()
            .items_start()
            .gap(px(SPACE_8))
            .p(px(CARD_INSET))
            .pt(px(CARD_INSET - EYEBROW_OPTICAL_LIFT))
            .rounded(px(RADIUS_LG))
            .border_1()
            .border_color(rgb(BORDER))
            .bg(rgb(SURFACE_RAISED))
            .child(
                column()
                    .flex_shrink_0()
                    .gap(px(SPACE_3))
                    .child(
                        row()
                            .h(px(CONTROL_HEIGHT_SM))
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
                            .child(if climate.pending {
                                "Updating…".to_owned()
                            } else {
                                status
                            }),
                    ),
            )
            .child(
                column()
                    .flex_1()
                    .min_w_0()
                    .gap(px(SPACE_5))
                    .child(
                        row()
                            .w_full()
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

    fn rooms(&self, snapshot: &HomeSnapshot, cx: &mut Context<Self>) -> Div {
        let enabled = snapshot.reachable;
        let mut grid = column()
            .debug_selector(|| "home.rooms".into())
            .w_full()
            .gap(px(SPACE_4));
        for line in ROOMS {
            let mut cards = row().w_full().items_stretch().gap(px(SPACE_4));
            for (room, switches) in line {
                // Everywhere counts the home's lights; a room counts its own.
                let summary = if switches.iter().any(|(key, _)| *key == SwitchKey::All) {
                    let all = tile_state(snapshot, SwitchKey::All);
                    all.detail.unwrap_or_else(|| {
                        if all.on {
                            "All on".into()
                        } else {
                            "Off".into()
                        }
                    })
                } else {
                    match switches
                        .iter()
                        .filter(|(key, _)| tile_state(snapshot, *key).on)
                        .count()
                    {
                        0 => "Off".into(),
                        lit => format!("{lit} on"),
                    }
                };
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
                cards = cards.child(
                    column()
                        .flex_1()
                        .min_w_0()
                        .gap(px(SPACE_4))
                        .p(px(CARD_INSET))
                        .rounded(px(RADIUS_LG))
                        .border_1()
                        .border_color(rgb(BORDER))
                        .child(
                            row()
                                .justify_between()
                                .child(
                                    div()
                                        .text_size(type_size(BODY_SIZE))
                                        .font_weight(FontWeight::MEDIUM)
                                        .child(room),
                                )
                                .child(hint(summary)),
                        )
                        .child(tiles),
                );
            }
            grid = grid.child(cards);
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
            let (mark, state) = match action.state.as_str() {
                "completed" => (None, ago(action.created_at, now)),
                "failed" => (
                    Some(Tone::Danger),
                    format!("Failed · {}", ago(action.created_at, now)),
                ),
                _ => (Some(Tone::Info), "Applying…".to_owned()),
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

/// A caption above a control.
pub fn labelled(label: &'static str, control: impl IntoElement) -> Div {
    column()
        .items_center()
        .gap(px(SPACE_2))
        .child(eyebrow(label))
        .child(control)
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
            (true, true) => "Lights and the thermostat, live from your control center.",
            (true, false) => "Your control center can't be reached right now.",
            (false, _) => "Lights and the thermostat, through your control center.",
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
                    page = page.child(self.climate(climate, cx));
                }
                page = page.child(
                    column()
                        .gap(px(SPACE_3))
                        .child(eyebrow("Lights"))
                        .child(self.rooms(&snapshot, cx)),
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
        assert_eq!(
            host("https://app.worldwidewebb.co/"),
            "app.worldwidewebb.co"
        );
    }
}
