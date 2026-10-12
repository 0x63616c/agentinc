//! Smart Home: the wall panel for the house. Lights, the fan, climate, the
//! Sonos rooms, the weather, alarms and the Wi-Fi, with the recent activity
//! beside them. Every control works on local state until the house is wired
//! in; the shape of the page is the contract.
use crate::{
    page::Page,
    routes::{Destination, Route},
    ui::*,
};
use chrono::{Local, Timelike};
use gpui::{prelude::*, *};

/// A lamp or light that is on or off.
#[derive(Clone, Debug)]
pub struct Light {
    pub name: &'static str,
    pub room: &'static str,
    pub on: bool,
}
/// A Sonos room with what it is playing.
#[derive(Clone, Debug)]
pub struct Speaker {
    pub room: &'static str,
    pub track: &'static str,
    pub artist: &'static str,
    pub playing: bool,
    pub volume: u8,
}
#[derive(Clone, Debug)]
pub struct Alarm {
    pub label: &'static str,
    pub time: &'static str,
    pub days: &'static str,
    pub on: bool,
}
#[derive(Clone, Debug)]
pub struct Activity {
    pub text: String,
    pub at: i64,
}
/// The weather right now and for one coming hour.
#[derive(Clone, Copy, Debug)]
pub struct Weather {
    pub icon: Icon,
    pub temperature: i32,
    pub summary: &'static str,
}

const FAN_SPEEDS: [&str; 4] = ["Off", "Low", "Mid", "High"];
const CLIMATE_MODES: [&str; 3] = ["Cool", "Heat", "Auto"];
const MAX_ACTIVITY: usize = 8;

/// One day's weather by hour, so the forecast is stable across frames.
const DAY: [(Icon, i32, &str); 24] = [
    (Icon::Moon, 11, "Clear"),
    (Icon::Moon, 10, "Clear"),
    (Icon::Moon, 10, "Clear"),
    (Icon::Cloud, 9, "Cloudy"),
    (Icon::Cloud, 9, "Cloudy"),
    (Icon::Cloud, 10, "Cloudy"),
    (Icon::CloudSun, 11, "Partly sunny"),
    (Icon::CloudSun, 12, "Partly sunny"),
    (Icon::Sun, 14, "Sunny"),
    (Icon::Sun, 16, "Sunny"),
    (Icon::Sun, 18, "Sunny"),
    (Icon::Sun, 19, "Sunny"),
    (Icon::CloudSun, 20, "Partly sunny"),
    (Icon::CloudSun, 21, "Partly sunny"),
    (Icon::Cloud, 20, "Cloudy"),
    (Icon::CloudRain, 18, "Showers"),
    (Icon::CloudRain, 17, "Showers"),
    (Icon::Cloud, 16, "Cloudy"),
    (Icon::CloudSun, 15, "Partly sunny"),
    (Icon::Cloud, 14, "Cloudy"),
    (Icon::Moon, 13, "Clear"),
    (Icon::Moon, 12, "Clear"),
    (Icon::Moon, 12, "Clear"),
    (Icon::Moon, 11, "Clear"),
];
/// The weather at this hour of the day (0 to 23).
pub fn weather_at(hour: u32) -> Weather {
    let (icon, temperature, summary) = DAY[(hour % 24) as usize];
    Weather {
        icon,
        temperature,
        summary,
    }
}
/// The next twelve hours after `hour`, each with its label.
pub fn next_hours(hour: u32) -> Vec<(String, Weather)> {
    (1..=12)
        .map(|offset| {
            let h = (hour + offset) % 24;
            (format!("{h:02}"), weather_at(h))
        })
        .collect()
}

pub struct SmartHomePage {
    lights: Vec<Light>,
    fan_speed: usize,
    climate_on: bool,
    climate_target: i32,
    climate_mode: usize,
    speakers: Vec<Speaker>,
    alarms: Vec<Alarm>,
    activity: Vec<Activity>,
}
impl EventEmitter<Destination> for SmartHomePage {}

impl Default for SmartHomePage {
    fn default() -> Self {
        let now = time::now();
        Self {
            lights: vec![
                Light {
                    name: "Living room lamp",
                    room: "Living room",
                    on: true,
                },
                Light {
                    name: "Ceiling lights",
                    room: "Kitchen",
                    on: true,
                },
                Light {
                    name: "Desk lamp",
                    room: "Office",
                    on: false,
                },
                Light {
                    name: "Bedside lamp",
                    room: "Bedroom",
                    on: false,
                },
                Light {
                    name: "Hallway",
                    room: "Hallway",
                    on: true,
                },
            ],
            fan_speed: 1,
            climate_on: true,
            climate_target: 21,
            climate_mode: 2,
            speakers: vec![
                Speaker {
                    room: "Kitchen",
                    track: "Nightcall",
                    artist: "Kavinsky",
                    playing: true,
                    volume: 35,
                },
                Speaker {
                    room: "Living room",
                    track: "Weightless",
                    artist: "Marconi Union",
                    playing: false,
                    volume: 20,
                },
                Speaker {
                    room: "Office",
                    track: "Deep focus",
                    artist: "Playlist",
                    playing: false,
                    volume: 15,
                },
            ],
            alarms: vec![
                Alarm {
                    label: "Wake up",
                    time: "07:00",
                    days: "Weekdays",
                    on: true,
                },
                Alarm {
                    label: "Gym",
                    time: "06:30",
                    days: "Tue, Thu",
                    on: false,
                },
                Alarm {
                    label: "Sleep",
                    time: "23:00",
                    days: "Every day",
                    on: true,
                },
            ],
            activity: vec![
                Activity {
                    text: "Kitchen started playing Nightcall".into(),
                    at: now - 4 * 60,
                },
                Activity {
                    text: "Hallway turned on".into(),
                    at: now - 26 * 60,
                },
                Activity {
                    text: "Climate set to 21°".into(),
                    at: now - 2 * 3600,
                },
                Activity {
                    text: "Wake up alarm rang".into(),
                    at: now - 7 * 3600,
                },
            ],
        }
    }
}

impl SmartHomePage {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self::default()
    }
    /// How many lights are on.
    pub fn lights_on(&self) -> usize {
        self.lights.iter().filter(|l| l.on).count()
    }
    /// The room playing music, if one is.
    pub fn playing(&self) -> Option<&Speaker> {
        self.speakers.iter().find(|s| s.playing)
    }
    pub fn climate(&self) -> Option<i32> {
        self.climate_on.then_some(self.climate_target)
    }
    /// The next alarm that is on.
    pub fn next_alarm(&self) -> Option<&Alarm> {
        self.alarms.iter().find(|a| a.on)
    }
    fn log(&mut self, text: String) {
        self.activity.insert(
            0,
            Activity {
                text,
                at: time::now(),
            },
        );
        self.activity.truncate(MAX_ACTIVITY);
    }
    fn toggle_light(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(light) = self.lights.get_mut(index) else {
            return;
        };
        light.on = !light.on;
        let text = format!(
            "{} turned {}",
            light.name,
            if light.on { "on" } else { "off" }
        );
        self.log(text);
        cx.notify();
    }
    fn set_fan(&mut self, speed: usize, cx: &mut Context<Self>) {
        self.fan_speed = speed;
        self.log(format!("Fan set to {}", FAN_SPEEDS[speed].to_lowercase()));
        cx.notify();
    }
    fn set_climate(&mut self, delta: i32, cx: &mut Context<Self>) {
        self.climate_target = (self.climate_target + delta).clamp(16, 30);
        self.log(format!("Climate set to {}°", self.climate_target));
        cx.notify();
    }
    fn toggle_speaker(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(speaker) = self.speakers.get_mut(index) else {
            return;
        };
        speaker.playing = !speaker.playing;
        let text = if speaker.playing {
            format!("{} started playing {}", speaker.room, speaker.track)
        } else {
            format!("{} paused", speaker.room)
        };
        self.log(text);
        cx.notify();
    }
    fn set_volume(&mut self, index: usize, delta: i8, cx: &mut Context<Self>) {
        if let Some(speaker) = self.speakers.get_mut(index) {
            speaker.volume = (speaker.volume as i16 + delta as i16).clamp(0, 100) as u8;
            cx.notify();
        }
    }

    fn section(title: &'static str, name: Icon, trailing: Option<AnyElement>) -> Div {
        card().child(
            row()
                .items_center()
                .gap(px(SPACE_2))
                .child(icon(name, ICON_SIZE))
                .child(heading(title))
                .child(div().flex_1())
                .children(trailing),
        )
    }
    fn device_tile(&self, index: usize, light: &Light, ui: &mut Ui<Self>) -> Div {
        let on = light.on;
        row()
            .items_center()
            .gap(px(SPACE_3))
            .min_h(px(LIST_ROW_HEIGHT))
            .child(
                div()
                    .size(px(CONTROL_HEIGHT))
                    .rounded(px(RADIUS_MD))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(rgb(if on {
                        theme().status_amber_surface
                    } else {
                        theme().surface_control
                    }))
                    .child(icon(Icon::Bulb, ICON_SIZE).text_color(rgb(if on {
                        theme().status_amber
                    } else {
                        theme().text_tertiary
                    }))),
            )
            .child(
                column()
                    .flex_1()
                    .min_w_0()
                    .gap(px(SPACE_HALF))
                    .child(div().text_size(type_size(BODY_SIZE)).child(light.name))
                    .child(caption(light.room)),
            )
            .child(
                Toggle::new(
                    SharedString::from(format!("home.light.{index}")),
                    light.name,
                )
                .on(on)
                .build(ui, move |this, _, cx| this.toggle_light(index, cx)),
            )
    }
    fn controls(&self, ui: &mut Ui<Self>) -> Div {
        let on = self.lights_on();
        Self::section(
            "Controls",
            Icon::Bulb,
            Some(
                badge(
                    copy::pluralize(on, "light on", "lights on"),
                    if on > 0 { Tone::Warning } else { Tone::Neutral },
                )
                .into_any_element(),
            ),
        )
        .child(
            column().children(
                self.lights
                    .iter()
                    .enumerate()
                    .map(|(index, light)| self.device_tile(index, light, ui)),
            ),
        )
        .child(divider())
        .child(
            row()
                .items_center()
                .gap(px(SPACE_3))
                .min_h(px(LIST_ROW_HEIGHT))
                .child(
                    div()
                        .size(px(CONTROL_HEIGHT))
                        .rounded(px(RADIUS_MD))
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(rgb(if self.fan_speed > 0 {
                            theme().status_blue_surface
                        } else {
                            theme().surface_control
                        }))
                        .child(
                            icon(Icon::Fan, ICON_SIZE).text_color(rgb(if self.fan_speed > 0 {
                                theme().status_blue
                            } else {
                                theme().text_tertiary
                            })),
                        ),
                )
                .child(
                    column()
                        .flex_1()
                        .min_w_0()
                        .gap(px(SPACE_HALF))
                        .child(div().text_size(type_size(BODY_SIZE)).child("Ceiling fan"))
                        .child(caption("Living room")),
                )
                .child(
                    Segmented::new("home.fan", FAN_SPEEDS)
                        .selected(self.fan_speed)
                        .build(ui, |this, index, _, cx| this.set_fan(index, cx)),
                ),
        )
    }
    fn climate_card(&self, ui: &mut Ui<Self>) -> Div {
        let on = self.climate_on;
        Self::section(
            "Climate",
            Icon::Thermometer,
            Some(
                Toggle::new("home.climate", "Air conditioning")
                    .on(on)
                    .build(ui, |this, _, cx| {
                        this.climate_on = !this.climate_on;
                        let text = if this.climate_on {
                            "Climate turned on"
                        } else {
                            "Climate turned off"
                        };
                        this.log(text.into());
                        cx.notify();
                    })
                    .into_any_element(),
            ),
        )
        .child(
            row()
                .items_center()
                .gap(px(SPACE_4))
                .child(
                    Button::new("home.climate.down", "Cooler")
                        .icon(Icon::ChevronDown)
                        .icon_only()
                        .secondary()
                        .enabled(on)
                        .build(ui, |this, _, cx| this.set_climate(-1, cx)),
                )
                .child(
                    column()
                        .flex_1()
                        .items_center()
                        .gap(px(SPACE_HALF))
                        .child(
                            div()
                                .text_size(px(DISPLAY_SIZE * 2.))
                                .font_weight(FontWeight::LIGHT)
                                .text_color(rgb(if on {
                                    theme().text
                                } else {
                                    theme().text_tertiary
                                }))
                                .child(format!("{}°", self.climate_target)),
                        )
                        .child(caption(if on {
                            format!(
                                "Inside 22°, {}",
                                CLIMATE_MODES[self.climate_mode].to_lowercase()
                            )
                        } else {
                            "Inside 22°, off".to_owned()
                        })),
                )
                .child(
                    Button::new("home.climate.up", "Warmer")
                        .icon(Icon::ChevronUp)
                        .icon_only()
                        .secondary()
                        .enabled(on)
                        .build(ui, |this, _, cx| this.set_climate(1, cx)),
                ),
        )
        .child(
            Segmented::new("home.climate.mode", CLIMATE_MODES)
                .selected(self.climate_mode)
                .enabled(on)
                .build(ui, |this, index, _, cx| {
                    this.climate_mode = index;
                    this.log(format!(
                        "Climate mode set to {}",
                        CLIMATE_MODES[index].to_lowercase()
                    ));
                    cx.notify();
                }),
        )
    }
    fn speaker_row(&self, index: usize, count: usize, speaker: &Speaker, ui: &mut Ui<Self>) -> Div {
        let playing = speaker.playing;
        row()
            .items_center()
            .gap(px(SPACE_3))
            .min_h(px(LIST_ROW_HEIGHT))
            .py(px(SPACE_1))
            .when(index + 1 < count, |s| {
                s.border_b_1().border_color(rgb(theme().border_subtle))
            })
            .child(
                Button::new(
                    SharedString::from(format!("home.speaker.{index}.play")),
                    if playing { "Pause" } else { "Play" },
                )
                .icon(if playing { Icon::Pause } else { Icon::Play })
                .icon_only()
                .kind(if playing {
                    ButtonKind::Primary
                } else {
                    ButtonKind::Secondary
                })
                .build(ui, move |this, _, cx| this.toggle_speaker(index, cx)),
            )
            .child(
                column()
                    .flex_1()
                    .min_w_0()
                    .gap(px(SPACE_HALF))
                    .child(
                        row()
                            .gap(px(SPACE_2))
                            .items_center()
                            .child(div().text_size(type_size(BODY_SIZE)).child(speaker.room))
                            .when(playing, |s| s.child(status_dot(Tone::Success))),
                    )
                    .child(caption(format!("{} · {}", speaker.track, speaker.artist))),
            )
            .child(
                row()
                    .items_center()
                    .gap(px(SPACE_1))
                    .child(
                        Button::new(
                            SharedString::from(format!("home.speaker.{index}.quieter")),
                            "Quieter",
                        )
                        .icon(Icon::ChevronDown)
                        .icon_only()
                        .ghost()
                        .small()
                        .build(ui, move |this, _, cx| this.set_volume(index, -5, cx)),
                    )
                    .child(
                        div()
                            .w(px(SPACE_8))
                            .text_size(type_size(LABEL_SIZE))
                            .text_color(rgb(theme().text_secondary))
                            .text_center()
                            .child(format!("{}%", speaker.volume)),
                    )
                    .child(
                        Button::new(
                            SharedString::from(format!("home.speaker.{index}.louder")),
                            "Louder",
                        )
                        .icon(Icon::ChevronUp)
                        .icon_only()
                        .ghost()
                        .small()
                        .build(ui, move |this, _, cx| this.set_volume(index, 5, cx)),
                    ),
            )
    }
    fn sound(&self, ui: &mut Ui<Self>) -> Div {
        let count = self.speakers.len();
        Self::section(
            "Sound System",
            Icon::Speaker,
            self.playing()
                .map(|s| badge(format!("Playing in {}", s.room), Tone::Success).into_any_element()),
        )
        .child(
            column().children(
                self.speakers
                    .iter()
                    .enumerate()
                    .map(|(index, speaker)| self.speaker_row(index, count, speaker, ui)),
            ),
        )
    }
    fn weather(&self, hour: u32) -> Div {
        let now = weather_at(hour);
        let high = DAY.iter().map(|d| d.1).max().unwrap_or_default();
        let low = DAY.iter().map(|d| d.1).min().unwrap_or_default();
        Self::section("Weather", Icon::CloudSun, None)
            .child(
                row()
                    .items_center()
                    .gap(px(SPACE_4))
                    .child(icon(now.icon, ICON_SIZE_LG * 2.).text_color(rgb(theme().text)))
                    .child(
                        column()
                            .gap(px(SPACE_HALF))
                            .child(
                                div()
                                    .text_size(px(DISPLAY_SIZE * 1.5))
                                    .font_weight(FontWeight::LIGHT)
                                    .child(format!("{}°", now.temperature)),
                            )
                            .child(caption(format!(
                                "{} · High {high}° · Low {low}°",
                                now.summary
                            ))),
                    ),
            )
            .child(divider())
            .child(eyebrow("Next 12 hours"))
            .child(
                row()
                    .justify_between()
                    .children(next_hours(hour).into_iter().map(|(label, weather)| {
                        column()
                            .flex_1()
                            .items_center()
                            .gap(px(SPACE_1))
                            .child(hint(label))
                            .child(icon(weather.icon, ICON_SIZE))
                            .child(
                                div()
                                    .text_size(type_size(LABEL_SIZE))
                                    .child(format!("{}°", weather.temperature)),
                            )
                    })),
            )
    }
    fn clock(&self, ui: &mut Ui<Self>) -> Div {
        let now = time::now();
        let count = self.alarms.len();
        Self::section("Clock", Icon::Clock, None)
            .child(
                column()
                    .gap(px(SPACE_HALF))
                    .child(
                        div()
                            .text_size(px(DISPLAY_SIZE * 2.))
                            .font_weight(FontWeight::LIGHT)
                            .child(time::clock(now)),
                    )
                    .child(caption(time::long_date(now))),
            )
            .child(divider())
            .child(eyebrow("Alarms"))
            .child(
                column().children(self.alarms.iter().enumerate().map(|(index, alarm)| {
                    row()
                        .items_center()
                        .gap(px(SPACE_3))
                        .min_h(px(LIST_ROW_HEIGHT))
                        .when(index + 1 < count, |s| {
                            s.border_b_1().border_color(rgb(theme().border_subtle))
                        })
                        .child(icon(Icon::Bell, ICON_SIZE_SM))
                        .child(
                            column()
                                .flex_1()
                                .min_w_0()
                                .gap(px(SPACE_HALF))
                                .child(
                                    row()
                                        .gap(px(SPACE_2))
                                        .items_baseline()
                                        .child(
                                            div()
                                                .text_size(type_size(HEADING_SIZE))
                                                .child(alarm.time),
                                        )
                                        .child(
                                            div()
                                                .text_size(type_size(BODY_SIZE))
                                                .text_color(rgb(theme().text_secondary))
                                                .child(alarm.label),
                                        ),
                                )
                                .child(caption(alarm.days)),
                        )
                        .child(
                            Toggle::new(
                                SharedString::from(format!("home.alarm.{index}")),
                                alarm.label,
                            )
                            .on(alarm.on)
                            .build(ui, move |this, _, cx| {
                                if let Some(alarm) = this.alarms.get_mut(index) {
                                    alarm.on = !alarm.on;
                                    cx.notify();
                                }
                            }),
                        )
                })),
            )
    }
    fn activity_card(&self) -> Div {
        let now = time::now();
        let count = self.activity.len();
        Self::section("Activity", Icon::History, None).child(column().children(
            self.activity.iter().enumerate().map(|(index, entry)| {
                row()
                    .items_center()
                    .gap(px(SPACE_3))
                    .min_h(px(CONTROL_HEIGHT))
                    .when(index + 1 < count, |s| {
                        s.border_b_1().border_color(rgb(theme().border_subtle))
                    })
                    .child(status_dot(if index == 0 {
                        Tone::Info
                    } else {
                        Tone::Neutral
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(type_size(BODY_SIZE))
                            .child(entry.text.clone()),
                    )
                    .child(hint(time::relative(entry.at, now)))
            }),
        ))
    }
    fn wifi(&self) -> Div {
        Self::section(
            "Wi-Fi",
            Icon::Wifi,
            Some(badge("Online", Tone::Success).into_any_element()),
        )
        .child(property_row("Network", div().child("World Wide Webb")))
        .child(property_row("Guest", div().child("WWW Guest")))
        .child(property_row("Password", div().child("ask-the-house")))
    }
}

impl Page for SmartHomePage {
    const ROUTE: Route = Route::SmartHome;
}

impl Render for SmartHomePage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let hour = Local::now().hour();
        let weather = weather_at(hour);
        let mut parts = vec![format!(
            "{} · {}°",
            copy::pluralize(self.lights_on(), "light on", "lights on"),
            weather.temperature
        )];
        if let Some(speaker) = self.playing() {
            parts.push(format!("Playing in the {}", speaker.room.to_lowercase()));
        }
        let ui = &mut Ui::new(window, cx);
        PageFrame::document(
            PageHeader::new(self.title())
                .description(parts.join(" · "))
                .actions(badge("Everyone home", Tone::Info)),
        )
        .child(
            row()
                .items_start()
                .gap(px(SPACE_4))
                .child(
                    column()
                        .flex_1()
                        .min_w_0()
                        .gap(px(SPACE_4))
                        .child(self.controls(ui))
                        .child(self.climate_card(ui))
                        .child(self.sound(ui)),
                )
                .child(
                    column()
                        .flex_1()
                        .min_w_0()
                        .gap(px(SPACE_4))
                        .child(self.weather(hour))
                        .child(self.clock(ui))
                        .child(self.activity_card())
                        .child(self.wifi()),
                ),
        )
        .build()
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_ACTIVITY, SmartHomePage, next_hours, weather_at};

    #[test]
    fn the_forecast_covers_twelve_hours_and_wraps_midnight() {
        let hours = next_hours(20);
        assert_eq!(hours.len(), 12);
        assert_eq!(hours[0].0, "21");
        assert_eq!(hours[11].0, "08");
        assert_eq!(weather_at(24).temperature, weather_at(0).temperature);
    }

    #[test]
    fn toggling_a_light_logs_it_first() {
        let mut home = SmartHomePage::default();
        let before = home.lights_on();
        home.lights[2].on = !home.lights[2].on;
        home.log("Desk lamp turned on".into());
        assert_eq!(home.lights_on(), before + 1);
        assert_eq!(home.activity[0].text, "Desk lamp turned on");
        for _ in 0..MAX_ACTIVITY {
            home.log("x".into());
        }
        assert_eq!(home.activity.len(), MAX_ACTIVITY);
    }
}
