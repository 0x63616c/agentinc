//! The Smart Home page: the thermostat and every light, arranged by room.
use crate::{
    calendar::{ago, now},
    home::{HomeModel, RANGE_GAP, SETPOINT_MAX, SETPOINT_MIN, glyph},
    model::{OpenRoute, Route},
    ui::*,
};
use ainc_client::types::{ActionView, ClimateMode, HomeClimate, HomeSnapshot, SwitchKey};
use gpui::{prelude::*, *};

/// Rooms in the order they read, with each switch's short name inside its room.
const ROOMS: [(&str, &[(SwitchKey, &str)]); 4] = [
    (
        "Everywhere",
        &[
            (SwitchKey::All, "All lights"),
            (SwitchKey::Lamps, "All lamps"),
        ],
    ),
    ("Living room", &[(SwitchKey::LivingRoomLamps, "Lamps")]),
    ("Bedroom", &[(SwitchKey::BedroomLamps, "Lamps")]),
    (
        "Kitchen",
        &[
            (SwitchKey::KitchenCeiling, "Ceiling"),
            (SwitchKey::UnderCabinet, "Under cabinet"),
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
    let target = match climate.mode {
        ClimateMode::Off => return ("Thermostat off".into(), Tone::Neutral),
        ClimateMode::HeatCool => format!(
            "{}–{}°",
            climate.target_low.unwrap_or(SETPOINT_MIN),
            climate.target_high.unwrap_or(SETPOINT_MAX)
        ),
        _ => format!("{}°", climate.target.unwrap_or(72)),
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
pub fn action_tone(action: &ActionView) -> Tone {
    match action.state.as_str() {
        "completed" => Tone::Success,
        "failed" => Tone::Danger,
        _ => Tone::Info,
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

    fn climate(&self, climate: &HomeClimate, cx: &mut Context<Self>) -> Div {
        let (status, tone) = climate_status(climate);
        let mode = MODES
            .iter()
            .position(|(mode, _)| *mode == climate.mode)
            .unwrap_or(0);
        let mut controls = column().items_end().gap(px(SPACE_4)).child(segmented(
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
        ));
        controls = match climate.mode {
            ClimateMode::Off => controls.child(caption("Choose a mode to set a temperature.")),
            ClimateMode::HeatCool => {
                let low = climate.target_low.unwrap_or(68);
                let high = climate.target_high.unwrap_or(74);
                controls.child(
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
                        )),
                )
            }
            _ => {
                let target = climate.target.unwrap_or(72);
                controls.child(labelled(
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
                ))
            }
        };
        row()
            .debug_selector(|| "home.climate".into())
            .w_full()
            .justify_between()
            .items_center()
            .gap(px(SPACE_6))
            .p(px(SPACE_6))
            .rounded(px(RADIUS_LG))
            .border_1()
            .border_color(rgb(BORDER))
            .bg(rgb(SURFACE_RAISED))
            .child(
                column()
                    .gap(px(SPACE_3))
                    .child(
                        row()
                            .gap(px(SPACE_2))
                            .child(icon("thermometer", ICON_SIZE_SM))
                            .child(eyebrow("Indoor")),
                    )
                    .child(hero(climate.ambient.map_or_else(|| "—".into(), degrees)))
                    .child(
                        row()
                            .gap(px(SPACE_2))
                            .text_size(type_size(LABEL_SIZE))
                            .text_color(rgb(TEXT_SECONDARY))
                            .child(status_dot(tone))
                            .child(status)
                            .when(climate.pending, |s| s.child(hint("· Updating"))),
                    ),
            )
            .child(controls)
    }

    fn rooms(&self, snapshot: &HomeSnapshot, cx: &mut Context<Self>) -> Div {
        let enabled = snapshot.reachable;
        let mut grid = row()
            .debug_selector(|| "home.rooms".into())
            .w_full()
            .items_start()
            .flex_wrap()
            .gap(px(SPACE_4));
        for (room, switches) in ROOMS {
            let mut tiles = column().w_full().gap(px(SPACE_3));
            for (key, label) in switches.iter().copied() {
                let Some(state) = snapshot.switches.iter().find(|s| s.key == key) else {
                    continue;
                };
                let on = state.on;
                tiles = tiles.child(div().w_full().flex().child(switch_tile(
                    SharedString::from(format!("home.switch.{key}")),
                    label,
                    glyph(key),
                    on,
                    state.pending,
                    enabled,
                    &self.hover,
                    move |this: &mut Self, _, cx| this.switch(key, !on, cx),
                    cx,
                )));
            }
            grid = grid.child(
                column()
                    .flex_1()
                    .min_w(px(TILE_MIN_WIDTH))
                    .gap(px(SPACE_3))
                    .child(eyebrow(room))
                    .child(tiles),
            );
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
            let state = match action.state.as_str() {
                "completed" => "Done",
                "failed" => "Failed",
                "running" => "Applying",
                _ => "Queued",
            };
            rows = rows.child(
                row()
                    .min_h(px(LIST_ROW_HEIGHT))
                    .px(px(SPACE_4))
                    .gap(px(SPACE_3))
                    .child(status_dot(action_tone(action)))
                    .child(
                        column()
                            .flex_1()
                            .min_w_0()
                            .child(div().truncate().child(action.summary.clone()))
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
                    .child(caption(format!(
                        "{state} · {}",
                        ago(action.created_at, now)
                    ))),
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
        let connection = snapshot.as_ref().and_then(|s| s.connection.clone());
        let reachable = snapshot.as_ref().is_some_and(|s| s.reachable);
        let description = match (&connection, reachable) {
            (Some(c), true) => format!("Live from {}", host(&c.base_url)),
            (Some(c), false) => format!("Can't reach {}", host(&c.base_url)),
            (None, _) => "Lights and climate through World Wide Webb's control center.".into(),
        };
        let mut header = PageHeader::new("Smart Home").description(description);
        if connection.is_some() {
            header = header.actions(
                row()
                    .gap(px(CONTROL_GAP))
                    .child(
                        Button::new("home.all-off", "All off")
                            .secondary()
                            .icon("power")
                            .enabled(reachable)
                            .build(
                                &self.hover,
                                |this: &mut Self, _, cx| this.switch(SwitchKey::All, false, cx),
                                cx,
                            ),
                    )
                    .child(
                        Button::new("home.all-on", "All on")
                            .primary()
                            .icon("bulb")
                            .enabled(reachable)
                            .build(
                                &self.hover,
                                |this: &mut Self, _, cx| this.switch(SwitchKey::All, true, cx),
                                cx,
                            ),
                    ),
            );
        }
        let mut page = Page::document(header);
        if let Some(error) = error.filter(|_| snapshot.is_none()) {
            page = page.child(banner(Tone::Danger, error));
        }
        match snapshot {
            None => page = page.child(skeleton_rows("home.loading", 4)),
            Some(snapshot) if snapshot.connection.is_none() => {
                page = page.child(
                    EmptyState::new("home", "Connect your control center")
                        .description(
                            "AgentInc switches lights and reads the thermostat through World Wide Webb. Add its address in Settings.",
                        )
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
