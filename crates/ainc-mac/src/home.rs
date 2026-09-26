//! Smart Home state shared by the Dashboard and the Smart Home page. Changes
//! show immediately and are confirmed by the daemon's next reading.
use crate::storage::{self, api_error};
use ainc_client::types::{
    ClimateMode, HomeCommand, HomeConnection, HomeConnectionRequest, HomeSnapshot, SwitchKey,
};
use gpui::{prelude::*, *};

/// What a thermostat without remembered setpoints starts from.
pub const DEFAULT_TARGET: i64 = 72;
pub const DEFAULT_LOW: i64 = 68;
pub const DEFAULT_HIGH: i64 = 74;

/// A change the daemon refused or could not accept.
pub struct HomeFailed(pub String);

pub struct HomeModel {
    pub snapshot: Option<HomeSnapshot>,
    /// Why the daemon itself could not be read.
    pub error: Option<String>,
    visible: bool,
    refreshing: bool,
    /// Bumped by every local change so an older reading cannot undo it.
    generation: u64,
}
impl EventEmitter<HomeFailed> for HomeModel {}

/// The icon for each switch.
pub fn glyph(key: SwitchKey) -> &'static str {
    match key {
        SwitchKey::All => "bulb",
        SwitchKey::Lamps => "lamp",
        SwitchKey::BedroomLamps => "bed",
        SwitchKey::LivingRoomLamps => "sofa",
        SwitchKey::KitchenCeiling => "ceiling",
        SwitchKey::UnderCabinet => "strip",
    }
}

/// How a tile should read: on, partly on, and a line such as "2 of 4 on".
/// The daemon counts each group's lights, so a tile never contradicts them.
pub struct TileState {
    pub on: bool,
    pub mixed: bool,
    pub pending: bool,
    pub detail: Option<String>,
}
pub fn tile_state(snapshot: &HomeSnapshot, key: SwitchKey) -> TileState {
    let Some(switch) = snapshot.switches.iter().find(|s| s.key == key) else {
        return TileState {
            on: false,
            mixed: false,
            pending: false,
            detail: None,
        };
    };
    let mixed = switch.lit > 0 && switch.lit < switch.total;
    TileState {
        on: switch.on,
        mixed,
        pending: switch.pending,
        detail: mixed.then(|| format!("{} of {} on", switch.lit, switch.total)),
    }
}

/// Show a press at once: lights it changes travel, lights already in that
/// state stay counted, and every group recounts from the membership the
/// daemon reported. Counts include only confirmed lights, as the daemon's do.
pub fn apply_switch(snapshot: &mut HomeSnapshot, key: SwitchKey, on: bool) {
    let covered = |switch: &ainc_client::types::HomeSwitch| -> Vec<SwitchKey> {
        if switch.members.is_empty() {
            vec![switch.key]
        } else {
            switch.members.clone()
        }
    };
    let Some(pressed) = snapshot.switches.iter().find(|s| s.key == key) else {
        return;
    };
    let lights = covered(pressed);
    for switch in &mut snapshot.switches {
        if switch.members.is_empty() && lights.contains(&switch.key) && switch.on != on {
            switch.on = on;
            switch.pending = true;
        }
    }
    let leaves: Vec<(SwitchKey, bool, bool)> = snapshot
        .switches
        .iter()
        .filter(|s| s.members.is_empty())
        .map(|s| (s.key, s.on, s.pending))
        .collect();
    for switch in &mut snapshot.switches {
        let members = covered(switch);
        let state = |wanted: bool| {
            leaves
                .iter()
                .filter(|(light, on, pending)| {
                    members.contains(light) && *on && (wanted || !*pending)
                })
                .count() as i64
        };
        switch.lit = state(false);
        switch.on = state(true) == switch.total;
        if switch.key == key {
            switch.pending = true;
        }
    }
}

impl HomeModel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        #[cfg(not(test))]
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(2))
                    .await;
                let alive = this.update(cx, |this, cx| {
                    if this.visible {
                        this.refresh(cx);
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
        let _ = cx;
        Self {
            snapshot: None,
            error: None,
            visible: false,
            refreshing: false,
            generation: 0,
        }
    }
    /// Pages that show the home keep it live while they are on screen.
    pub fn set_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        if visible && !self.visible {
            self.visible = true;
            self.refresh(cx);
        }
        self.visible = visible;
    }
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        // Tests drive these models through fixtures, never the daemon.
        if cfg!(test) {
            return;
        }
        if self.refreshing {
            return;
        }
        self.refreshing = true;
        let generation = self.generation;
        let work = cx.background_executor().spawn(async {
            storage::background(async {
                let client = storage::client().await?;
                let snapshot = client.home_state().send().await.map_err(api_error)?;
                anyhow::Ok(snapshot.into_inner())
            })
        });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                this.refreshing = false;
                match result {
                    Ok(snapshot) if this.generation == generation => {
                        this.snapshot = Some(snapshot);
                        this.error = None;
                    }
                    Ok(_) => {}
                    Err(error) => this.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn switch(&mut self, key: SwitchKey, on: bool, cx: &mut Context<Self>) {
        if let Some(snapshot) = &mut self.snapshot {
            apply_switch(snapshot, key, on);
        }
        self.command(HomeCommand::Switch { key, on }, cx);
    }
    pub fn set_mode(&mut self, mode: ClimateMode, cx: &mut Context<Self>) {
        let Some(climate) = self.snapshot.as_mut().and_then(|s| s.climate.as_mut()) else {
            return;
        };
        climate.mode = mode;
        climate.pending = true;
        if mode == ClimateMode::HeatCool {
            climate.target_low.get_or_insert(DEFAULT_LOW);
            climate.target_high.get_or_insert(DEFAULT_HIGH);
        }
        self.command(HomeCommand::SetClimateMode { mode }, cx);
    }
    /// Move the single target by `delta` degrees within the band.
    pub fn step_target(&mut self, delta: i64, cx: &mut Context<Self>) {
        let Some(climate) = self.snapshot.as_mut().and_then(|s| s.climate.as_mut()) else {
            return;
        };
        let target =
            (climate.target.unwrap_or(DEFAULT_TARGET) + delta).clamp(climate.min, climate.max);
        climate.target = Some(target);
        climate.pending = true;
        self.command(HomeCommand::SetClimateTarget { target }, cx);
    }
    /// Move one end of the Auto range, keeping the ends the thermostat's gap apart.
    pub fn step_range(&mut self, high: bool, delta: i64, cx: &mut Context<Self>) {
        let Some(climate) = self.snapshot.as_mut().and_then(|s| s.climate.as_mut()) else {
            return;
        };
        let mut low = climate.target_low.unwrap_or(DEFAULT_LOW);
        let mut top = climate.target_high.unwrap_or(DEFAULT_HIGH);
        if high {
            top = (top + delta).clamp(low + climate.gap, climate.max);
        } else {
            low = (low + delta).clamp(climate.min, top - climate.gap);
        }
        climate.target_low = Some(low);
        climate.target_high = Some(top);
        climate.pending = true;
        self.command(HomeCommand::SetClimateRange { low, high: top }, cx);
    }

    fn command(&mut self, command: HomeCommand, cx: &mut Context<Self>) {
        self.generation += 1;
        cx.notify();
        #[cfg(test)]
        {
            let _ = command;
        }
        #[cfg(not(test))]
        {
            use ainc_client::types::HomeRequest;
            let work = cx.background_executor().spawn(async move {
                storage::background(async move {
                    let client = storage::client().await?;
                    client
                        .home_command()
                        .body(HomeRequest {
                            operation_id: uuid::Uuid::new_v4().to_string(),
                            command,
                        })
                        .send()
                        .await
                        .map_err(api_error)?;
                    anyhow::Ok(())
                })
            });
            cx.spawn(async move |this, cx| {
                let result = work.await;
                let _ = this.update(cx, |this, cx| {
                    if let Err(error) = result {
                        cx.emit(HomeFailed(error.to_string()));
                    }
                    // Read the daemon's view: pending actions stay shown as wanted.
                    this.generation += 1;
                    this.refreshing = false;
                    this.refresh(cx);
                });
            })
            .detach();
        }
    }

    /// Save the control center address and optional Access token.
    pub fn connect(
        &mut self,
        request: HomeConnectionRequest,
        cx: &mut Context<Self>,
    ) -> Task<anyhow::Result<HomeConnection>> {
        let work = cx.background_executor().spawn(async move {
            storage::background(async move {
                let client = storage::client().await?;
                let connection = client
                    .home_connect()
                    .body(request)
                    .send()
                    .await
                    .map_err(api_error)?;
                anyhow::Ok(connection.into_inner())
            })
        });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| this.refresh(cx));
            result
        })
    }
    pub fn disconnect(&mut self, cx: &mut Context<Self>) -> Task<anyhow::Result<()>> {
        let work = cx.background_executor().spawn(async {
            storage::background(async {
                let client = storage::client().await?;
                client.home_disconnect().send().await.map_err(api_error)?;
                anyhow::Ok(())
            })
        });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| this.refresh(cx));
            result
        })
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn fixture(&mut self, snapshot: Option<HomeSnapshot>, cx: &mut Context<Self>) {
        self.snapshot = snapshot;
        self.error = None;
        cx.notify();
    }
}

#[cfg(test)]
pub(crate) mod fixtures {
    use super::*;
    use ainc_client::types::{ActionView, HomeClimate, HomeSwitch};

    /// A connected home: bedroom lamps and the kitchen ceiling on, cooling to 72°.
    pub fn connected() -> HomeSnapshot {
        let switch =
            |key, label: &str, room: &str, on: bool, members: Vec<SwitchKey>, lit| HomeSwitch {
                key,
                label: label.into(),
                room: room.into(),
                on,
                pending: false,
                total: members.len().max(1) as i64,
                members,
                lit,
            };
        use SwitchKey::*;
        HomeSnapshot {
            connection: Some(HomeConnection {
                base_url: "https://app.worldwidewebb.co".into(),
                access_token: true,
            }),
            reachable: true,
            error: None,
            switches: vec![
                switch(
                    All,
                    "All lights",
                    "Everywhere",
                    false,
                    vec![BedroomLamps, LivingRoomLamps, KitchenCeiling, UnderCabinet],
                    2,
                ),
                switch(
                    Lamps,
                    "All lamps",
                    "Everywhere",
                    false,
                    vec![BedroomLamps, LivingRoomLamps],
                    1,
                ),
                switch(BedroomLamps, "Bedroom lamps", "Bedroom", true, vec![], 1),
                switch(
                    LivingRoomLamps,
                    "Living room lamps",
                    "Living room",
                    false,
                    vec![],
                    0,
                ),
                switch(
                    KitchenCeiling,
                    "Kitchen ceiling",
                    "Kitchen",
                    true,
                    vec![],
                    1,
                ),
                switch(UnderCabinet, "Under cabinet", "Kitchen", false, vec![], 0),
            ],
            climate: Some(HomeClimate {
                mode: ClimateMode::Cool,
                ambient: Some(71.6),
                action: Some("Cooling".into()),
                target: Some(72),
                target_low: None,
                target_high: None,
                pending: false,
                min: 67,
                max: 77,
                gap: 2,
            }),
            actions: vec![
                ActionView {
                    id: "a2".into(),
                    summary: "Bedroom lamps on".into(),
                    state: ainc_client::types::ActionState::Completed,
                    error: None,
                    created_at: chrono::Utc::now().timestamp() - 240,
                    finished_at: Some(chrono::Utc::now().timestamp() - 239),
                },
                ActionView {
                    id: "a1".into(),
                    summary: "Climate to 72°".into(),
                    state: ainc_client::types::ActionState::Completed,
                    error: None,
                    created_at: chrono::Utc::now().timestamp() - 3_900,
                    finished_at: Some(chrono::Utc::now().timestamp() - 3_899),
                },
            ],
        }
    }
    /// Not connected yet. Used by the rendered shell suite.
    #[allow(dead_code)]
    pub fn disconnected() -> HomeSnapshot {
        HomeSnapshot {
            connection: None,
            reachable: false,
            climate: None,
            actions: vec![],
            ..connected()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // The GPUI glob exports its own `test` attribute; these are plain tests.
    use ::core::prelude::v1::test;

    fn on(snapshot: &HomeSnapshot) -> Vec<(SwitchKey, bool)> {
        snapshot.switches.iter().map(|s| (s.key, s.on)).collect()
    }

    #[test]
    fn switches_and_their_groups_agree_immediately() {
        let mut home = fixtures::connected();
        apply_switch(&mut home, SwitchKey::BedroomLamps, false);
        assert_eq!(
            on(&home),
            [
                (SwitchKey::All, false),
                (SwitchKey::Lamps, false),
                (SwitchKey::BedroomLamps, false),
                (SwitchKey::LivingRoomLamps, false),
                (SwitchKey::KitchenCeiling, true),
                (SwitchKey::UnderCabinet, false),
            ]
        );
        apply_switch(&mut home, SwitchKey::All, true);
        assert!(
            home.switches.iter().all(|s| s.on),
            "every light is wanted on"
        );
        let travelling = |key| home.switches.iter().find(|s| s.key == key).unwrap().pending;
        assert!(travelling(SwitchKey::All) && travelling(SwitchKey::UnderCabinet));
        assert!(
            !travelling(SwitchKey::KitchenCeiling),
            "a light already on is not travelling"
        );
        let all = home
            .switches
            .iter()
            .find(|s| s.key == SwitchKey::All)
            .unwrap();
        assert_eq!(
            all.lit, 1,
            "only confirmed lights count while the rest travel"
        );
        apply_switch(&mut home, SwitchKey::UnderCabinet, false);
        assert_eq!(
            on(&home)[0],
            (SwitchKey::All, false),
            "All needs everything on"
        );
        assert_eq!(on(&home)[1], (SwitchKey::Lamps, true));
        apply_switch(&mut home, SwitchKey::Lamps, false);
        assert_eq!(
            on(&home)[1..4],
            [
                (SwitchKey::Lamps, false),
                (SwitchKey::BedroomLamps, false),
                (SwitchKey::LivingRoomLamps, false)
            ]
        );
    }

    #[test]
    fn group_tiles_count_their_members() {
        let home = fixtures::connected();
        let all = tile_state(&home, SwitchKey::All);
        assert!(!all.on && all.mixed);
        assert_eq!(all.detail.as_deref(), Some("2 of 4 on"));
        let lamps = tile_state(&home, SwitchKey::Lamps);
        assert!(!lamps.on && lamps.mixed, "one of two lamp groups is on");
        assert_eq!(lamps.detail.as_deref(), Some("1 of 2 on"));
        let bedroom = tile_state(&home, SwitchKey::BedroomLamps);
        assert!(bedroom.on && !bedroom.mixed && bedroom.detail.is_none());
    }
}
