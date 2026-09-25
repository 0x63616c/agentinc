use crate::ui;
use crate::ui::{
    CONTROL_HEIGHT, FIELD_LABEL_GAP, PAGE_X, SETTINGS_INSET, SETTINGS_ROW_HEIGHT, type_size,
};
use crate::{
    input,
    model::{FontSize, PANE_WIDTHS, Route, Session},
    shell::{self, Shell},
    ui::Assets,
    ui::Overlay,
};
use anyhow::{Result, ensure};
use gpui::prelude::*;
use gpui::{
    AppContext, Bounds, IntoElement, Modifiers, MouseButton, Pixels, Render, VisualTestAppContext,
    Window, WindowHandle, div, point, px, size,
};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

struct LoadingPreview {
    kind: &'static str,
    start: Instant,
}

impl Render for LoadingPreview {
    fn render(&mut self, window: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        let frame = ui::LoadingFrame::new(self.start, window);
        div()
            .relative()
            .size_full()
            .bg(gpui::rgb(ui::SHELL))
            .flex()
            .items_center()
            .justify_center()
            .child(match self.kind {
                "page" => frame.page("Loading Tickets…").into_any_element(),
                "reconnect" => frame.inline("Reconnecting…").into_any_element(),
                _ => frame.inline("Evee is thinking…").into_any_element(),
            })
    }
}

fn capture_loading_frames(cx: &mut VisualTestAppContext, output: &std::path::Path) -> Result<()> {
    let preview = cx.open_offscreen_window(size(px(1360.), px(828.)), |_, cx| {
        cx.new(|_| LoadingPreview {
            kind: "page",
            start: Instant::now(),
        })
    })?;
    for kind in ["page", "inline", "reconnect"] {
        for (index, elapsed) in [0, 80, 160, 240].into_iter().enumerate() {
            preview.update(cx, |view, _, cx| {
                view.kind = kind;
                view.start = Instant::now() - Duration::from_millis(elapsed);
                cx.notify();
            })?;
            cx.run_until_parked();
            cx.update_window(preview.into(), |_, window, cx| window.draw(cx).clear(cx))?;
            cx.capture_screenshot(preview.into())?
                .save(output.join(format!("loading-{kind}-{index}.png")))?;
        }
    }
    Ok(())
}

// Check actual pixels in independent shell regions, rather than trusting scene/AX nodes.
// Coordinates are logical pixels; thresholds are deliberately below normal text contrast.
fn regions(width: u32, height: u32, dimmed: bool) -> Vec<(&'static str, [u32; 4], u8, usize)> {
    let text = if dimmed { 35 } else { 90 };
    let border = if dimmed { 7 } else { 20 };
    let mut regions = vec![
        ("header", [150, 10, width - 10, 40], text, 80),
        ("workspace", [15, 65, 165, 100], text, 60),
        (
            "profile avatar",
            [18, height - 38, 52, height - 8],
            text,
            30,
        ),
        ("profile name", [50, height - 34, 155, height - 9], text, 30),
        (
            "profile version",
            [160, height - 34, 234, height - 9],
            25,
            12,
        ),
        (
            "main border",
            [
                PANE_WIDTHS[0].2 as u32,
                110,
                PANE_WIDTHS[0].2 as u32 + 2,
                height - 30,
            ],
            border,
            150,
        ),
    ];
    for (index, (name, group_offset)) in [
        ("Tickets", 0),
        ("Assistant", 0),
        ("Agents", 12),
        ("Automations", 12),
    ]
    .into_iter()
    .enumerate()
    {
        let y = 115 + index as u32 * 34 + group_offset;
        regions.push((name, [20, y, 165, y + 28], text, 35));
    }
    regions
}

fn check_pixels(
    bytes: &[u8],
    pixel_width: u32,
    scale: u32,
    regions: &[(&str, [u32; 4], u8, usize)],
) -> Result<()> {
    for (name, [left, top, right, bottom], threshold, minimum) in regions {
        let mut count = 0;
        for y in top * scale..bottom * scale {
            for x in left * scale..right * scale {
                let offset = ((y * pixel_width + x) * 4) as usize;
                if bytes[offset..offset + 3].iter().copied().max().unwrap() > *threshold {
                    count += 1;
                }
            }
        }
        ensure!(
            count >= minimum * (scale * scale) as usize,
            "{name}: only {count} visible pixels"
        );
    }
    Ok(())
}

// Metal layout rounds to device pixels; one logical pixel covers that rounding.
const GEOMETRY_TOLERANCE: f32 = 1.;

fn near(name: &str, actual: f32, expected: f32) -> Result<()> {
    ensure!(
        (actual - expected).abs() <= GEOMETRY_TOLERANCE,
        "{name}: {actual:.2}px, expected {expected:.2}px ±{GEOMETRY_TOLERANCE}px"
    );
    Ok(())
}

struct Suite {
    cx: VisualTestAppContext,
    window: WindowHandle<Shell>,
    output: PathBuf,
    count: usize,
}
impl Suite {
    fn settle(&mut self) -> Result<()> {
        self.cx.run_until_parked();
        self.cx.update_window(self.window.into(), |_, window, cx| {
            window.draw(cx).clear(cx)
        })?;
        // Product transitions use Instant; allow their actual 180 ms duration to settle.
        std::thread::sleep(Duration::from_millis(210));
        self.cx.run_until_parked();
        self.cx.update_window(self.window.into(), |_, window, cx| {
            window.draw(cx).clear(cx)
        })?;
        Ok(())
    }
    fn capture(
        &mut self,
        name: &str,
        route: Route,
        overlay: Option<Overlay>,
        evee: bool,
    ) -> Result<()> {
        self.settle()?;
        let state = self
            .window
            .read_with(&self.cx, |shell, _| shell.fixture_state())?;
        ensure!(
            state == (route, overlay, evee),
            "{name}: unexpected shell state {state:?}"
        );
        let (viewport, scale) = self.cx.update_window(self.window.into(), |_, window, _| {
            (window.viewport_size(), window.scale_factor())
        })?;
        let image = self.cx.capture_screenshot(self.window.into())?;
        image.save(self.output.join(format!("{name}.png")))?;
        let width = f32::from(viewport.width) as u32;
        let height = f32::from(viewport.height) as u32;
        let scale = scale as u32;
        ensure!(
            image.width() == width * scale && image.height() == height * scale,
            "capture dimensions differ from viewport"
        );
        let probes = regions(
            width,
            height,
            overlay.is_some_and(|o| o == Overlay::Search || o.is_dialog()),
        );
        check_pixels(image.as_raw(), image.width(), scale, &probes)
            .map_err(|e| anyhow::anyhow!("{name}: {e}"))?;
        self.check_page_geometry(route)?;
        if name == "route-1-0" {
            self.check_shell_geometry()?;
        }
        if name.starts_with("ticket-field-") {
            self.check_field_geometry("Ticket title")?;
        }
        if name.starts_with("automation-fields-") {
            self.check_field_geometry("Name")?;
            self.check_field_geometry("Ticket prompt")?;
            let first = self.bounds("Name.input")?;
            let second = self.bounds("Ticket prompt.input")?;
            near(
                "paired Automation input edge",
                f32::from(first.origin.x),
                f32::from(second.origin.x),
            )?;
        }
        if name == "route-0-0" || name == "route-1-0" {
            let button = self.bounds("tickets.create")?;
            near(
                "shared regular Button height",
                f32::from(button.size.height),
                f32::from(type_size(CONTROL_HEIGHT)),
            )?;
        }
        if name == "settings-0" || name == "settings-1" {
            self.check_settings_row_geometry("Font")?;
            self.check_settings_row_geometry("Font size")?;
        }
        if self.count == 0 {
            // Negative controls: each missing region must independently fail this gate.
            for probe in &probes {
                let mut blank = image.clone();
                let [left, top, right, bottom] = probe.1;
                for y in top * scale..bottom * scale {
                    for x in left * scale..right * scale {
                        blank.get_pixel_mut(x, y).0 = [0, 0, 0, 255];
                    }
                }
                ensure!(
                    check_pixels(
                        blank.as_raw(),
                        blank.width(),
                        scale,
                        std::slice::from_ref(probe)
                    )
                    .is_err(),
                    "negative control passed: {}",
                    probe.0
                );
            }
        }
        self.count += 1;
        println!(
            "PASS {name}: {}x{}, {} shell regions",
            image.width(),
            image.height(),
            probes.len()
        );
        Ok(())
    }

    fn check_profile_row_geometry(&mut self) -> Result<()> {
        let card = self.bounds("sidebar-profile")?;
        let name = self.bounds("sidebar-profile-name")?;
        let version = self.bounds("sidebar-version")?;
        near("compact profile height", f32::from(card.size.height), 44.)?;
        ensure!(
            name.origin.x + name.size.width <= version.origin.x,
            "profile name overlaps version"
        );
        near(
            "profile name and version centres",
            f32::from(name.origin.y + name.size.height / 2.),
            f32::from(version.origin.y + version.size.height / 2.),
        )?;
        Ok(())
    }
    fn keys(&mut self, keys: &str) {
        self.cx.simulate_keystrokes(self.window.into(), keys);
    }
    fn click(&mut self, x: f32, y: f32) {
        self.cx.simulate_click(
            self.window.into(),
            point(px(x), px(y)),
            Modifiers::default(),
        );
    }

    fn click_selector(&mut self, selector: &str) -> Result<()> {
        let bounds = self.bounds(selector)?;
        self.click(
            f32::from(bounds.origin.x) + f32::from(bounds.size.width) / 2.,
            f32::from(bounds.origin.y) + f32::from(bounds.size.height) / 2.,
        );
        Ok(())
    }

    fn bounds(&mut self, selector: &str) -> Result<Bounds<Pixels>> {
        self.cx
            .update_window(self.window.into(), |_, window, _| {
                window.debug_bounds(selector)
            })?
            .ok_or_else(|| anyhow::anyhow!("missing {selector} bounds"))
    }

    fn check_field_geometry(&mut self, selector: &str) -> Result<()> {
        let label = self.bounds(&format!("{selector}.label"))?;
        let input = self.bounds(&format!("{selector}.input"))?;
        near(
            &format!("{selector} left edge"),
            f32::from(label.origin.x),
            f32::from(input.origin.x),
        )?;
        near(
            &format!("{selector} label gap"),
            f32::from(input.origin.y) - f32::from(label.origin.y + label.size.height),
            FIELD_LABEL_GAP,
        )
    }

    fn check_settings_row_geometry(&mut self, label: &str) -> Result<()> {
        let row = self.bounds(&format!("settings.row.{label}"))?;
        let text = self.bounds(&format!("settings.row.{label}.label"))?;
        let control = self.bounds(&format!("settings.row.{label}.control"))?;
        ensure!(
            f32::from(row.size.height) >= SETTINGS_ROW_HEIGHT - GEOMETRY_TOLERANCE,
            "{label}: SettingsRow is shorter than its shared minimum"
        );
        ensure!(
            f32::from(text.size.width) >= 150.,
            "{label}: SettingsRow label collapsed to a narrow column"
        );
        near(
            &format!("{label} SettingsRow label inset"),
            f32::from(text.origin.x - row.origin.x),
            SETTINGS_INSET,
        )?;
        near(
            &format!("{label} SettingsRow control inset"),
            f32::from(row.origin.x + row.size.width - control.origin.x - control.size.width),
            SETTINGS_INSET,
        )
    }

    fn check_shell_geometry(&mut self) -> Result<()> {
        let button = self.bounds("sidebar")?;
        let glyph = self.bounds("sidebar.glyph")?;
        near(
            "header icon horizontal center",
            f32::from(glyph.origin.x + glyph.size.width / 2.)
                - f32::from(button.origin.x + button.size.width / 2.),
            0.,
        )?;
        near(
            "header icon vertical center",
            f32::from(glyph.origin.y + glyph.size.height / 2.)
                - f32::from(button.origin.y + button.size.height / 2.),
            0.,
        )?;
        let main = self.bounds("main-pane")?;
        let status = self.bounds("status-bar")?;
        near(
            "status bar left edge",
            f32::from(status.origin.x),
            f32::from(main.origin.x),
        )?;
        near(
            "status bar width",
            f32::from(status.size.width),
            f32::from(main.size.width),
        )?;
        near("status bar height", f32::from(status.size.height), 10.)?;
        near(
            "status bar gap",
            f32::from(status.origin.y - main.origin.y - main.size.height),
            8.,
        )?;
        let content = self.bounds("main-content")?;
        near(
            "main content left inset",
            f32::from(content.origin.x - main.origin.x),
            PAGE_X,
        )?;
        near(
            "main content top inset",
            f32::from(content.origin.y - main.origin.y),
            PAGE_X,
        )?;
        ensure!(
            self.bounds("right-pane").is_err(),
            "removed Evee pane visible"
        );
        Ok(())
    }

    fn check_page_geometry(&mut self, route: Route) -> Result<()> {
        let main = self.bounds("main-pane")?;
        let frame = self.bounds("page-frame")?;
        let terminal_inset = if route == Route::Terminal { 8. } else { 0. };
        near(
            "page frame left edge",
            f32::from(frame.origin.x - main.origin.x),
            terminal_inset,
        )?;
        near(
            "page frame right edge",
            f32::from(main.origin.x + main.size.width - frame.origin.x - frame.size.width),
            terminal_inset,
        )?;
        if let Ok(content) = self.bounds("main-content") {
            near(
                "page content left inset",
                f32::from(content.origin.x - frame.origin.x),
                PAGE_X,
            )?;
            near(
                "page content right inset",
                f32::from(
                    frame.origin.x + frame.size.width - content.origin.x - content.size.width,
                ),
                PAGE_X,
            )?;
            let surface = match route {
                Route::Settings => Some("settings.row.Font"),
                Route::Automations => Some("automations.page"),
                _ => None,
            };
            if let Some(selector) = surface {
                let surface = self.bounds(selector)?;
                near(
                    "page surface right edge",
                    f32::from(
                        content.origin.x + content.size.width
                            - surface.origin.x
                            - surface.size.width,
                    ),
                    0.,
                )?;
            }
        } else {
            ensure!(
                matches!(route, Route::Assistant | Route::Terminal),
                "{route:?} is missing its shared document content"
            );
        }
        Ok(())
    }
}

pub fn run() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let session_path = temporary.path().join("session.json");
    let small_session_path = temporary.path().join("small-session.json");
    if std::env::var_os("AGENTINC_RENDER_LARGER").is_some() {
        let mut session = Session::default();
        session.font_size = FontSize::Larger;
        session.save(&session_path)?;
        session.save(&small_session_path)?;
    }
    let output = std::env::var_os("AGENTINC_RENDER_OUTPUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/rendered-shell"));
    std::fs::create_dir_all(&output)?;
    let mut cx = VisualTestAppContext::with_asset_source(
        gpui_platform::current_platform(false),
        Arc::new(Assets),
    );
    capture_loading_frames(&mut cx, &output)?;
    cx.update(|cx| {
        input::bind_keys(cx);
        shell::bind_keys(cx);
    });
    let mut window = cx.open_offscreen_window(size(px(1360.), px(828.)), |window, cx| {
        cx.new(|cx| Shell::fixture(session_path, window, cx))
    })?;
    let mut suite = Suite {
        cx,
        window,
        output,
        count: 0,
    };
    for (index, elapsed) in [0, 80, 160, 240].into_iter().enumerate() {
        suite.window.update(&mut suite.cx, |shell, _, cx| {
            shell.fixture_launch_elapsed(Duration::from_millis(elapsed), cx)
        })?;
        suite.cx.run_until_parked();
        suite
            .cx
            .update_window(suite.window.into(), |_, window, cx| {
                window.draw(cx).clear(cx)
            })?;
        suite
            .cx
            .capture_screenshot(suite.window.into())?
            .save(suite.output.join(format!("loading-launch-{index}.png")))?;
    }
    suite.window.update(&mut suite.cx, |shell, _, cx| {
        shell.fixture_launch_elapsed(Duration::from_secs(1), cx)
    })?;
    suite.capture("initial", Route::Assistant, None, false)?;
    suite.check_profile_row_geometry()?;
    suite.window.update(&mut suite.cx, |shell, _, cx| {
        shell.fixture_profile_name(
            "A very long profile name that must truncate before the version",
            cx,
        );
    })?;
    suite.capture("profile-long-name", Route::Assistant, None, false)?;
    suite.check_profile_row_geometry()?;
    suite.window.update(&mut suite.cx, |shell, _, cx| {
        shell.fixture_profile_name("QA Profile", cx);
    })?;
    suite.cx.simulate_mouse_move(
        suite.window.into(),
        point(px(125.), px(167.)),
        None::<MouseButton>,
        Modifiers::default(),
    );
    suite.capture("hover-tickets", Route::Assistant, None, false)?;
    suite.cx.simulate_mouse_move(
        suite.window.into(),
        point(px(500.), px(500.)),
        None::<MouseButton>,
        Modifiers::default(),
    );
    for round in 0..3 {
        if round == 1 {
            // AppKit resize is asynchronous without its native event loop. Use a second
            // real offscreen window at the smaller size; native resize is verified separately.
            window = suite
                .cx
                .open_offscreen_window(size(px(1160.), px(728.)), |window, cx| {
                    cx.new(|cx| Shell::fixture(small_session_path.clone(), window, cx))
                })?;
            suite.window = window;
            let actual = suite
                .cx
                .update_window(window.into(), |_, window, _| window.viewport_size())?;
            ensure!(
                actual == size(px(1160.), px(728.)),
                "small window dimensions"
            );
        }
        for (index, route) in [
            Route::Tickets,
            Route::Assistant,
            Route::Agents,
            Route::Automations,
        ]
        .into_iter()
        .enumerate()
        {
            suite.keys(&format!("cmd-{}", index + 1));
            suite.capture(&format!("route-{round}-{index}"), route, None, false)?;
            if round < 2 && route == Route::Tickets {
                suite.click_selector("tickets.create")?;
                suite.capture(
                    &format!("ticket-field-{round}"),
                    route,
                    Some(Overlay::AddTicket),
                    false,
                )?;
                suite.keys("escape");
            }
        }
        suite.keys("cmd-k");
        if round == 0 {
            suite.capture(
                "search-empty",
                Route::Automations,
                Some(Overlay::Search),
                false,
            )?;
        }
        suite.cx.simulate_input(window.into(), "settings");
        suite.capture(
            &format!("search-{round}"),
            Route::Automations,
            Some(Overlay::Search),
            false,
        )?;
        suite.keys("enter");
        suite.capture(&format!("settings-{round}"), Route::Settings, None, false)?;
        suite.keys("cmd-4");
        suite.capture(
            &format!("automations-{round}"),
            Route::Automations,
            None,
            false,
        )?;
        if round < 2 {
            suite.click_selector("automations.create")?;
            suite.capture(
                &format!("automation-fields-{round}"),
                Route::Automations,
                None,
                false,
            )?;
        }
    }
    suite.keys("cmd-1");
    suite.settle()?;
    suite.click_selector("tickets.create")?;
    suite.capture(
        "add-dialog",
        Route::Tickets,
        Some(Overlay::AddTicket),
        false,
    )?;
    suite.cx.simulate_input(window.into(), "Rendered café 👋");
    suite.capture("add-typed", Route::Tickets, Some(Overlay::AddTicket), false)?;
    suite.keys("escape");
    suite.capture("dialog-dismissed", Route::Tickets, None, false)?;
    suite.keys("cmd-k");
    suite.capture("search-open", Route::Tickets, Some(Overlay::Search), false)?;
    suite.keys("cmd-,");
    suite.capture("settings-shortcut", Route::Settings, None, false)?;
    suite
        .window
        .update(&mut suite.cx, |shell, _, cx| shell.fixture_chat(false, cx))?;
    suite.capture("assistant-new-conversation", Route::Assistant, None, false)?;
    suite
        .window
        .update(&mut suite.cx, |shell, _, cx| shell.fixture_chat(true, cx))?;
    suite.capture("assistant-conversation", Route::Assistant, None, false)?;
    suite.click_selector("back-to-conversations")?;
    suite.capture("assistant-conversation-list", Route::Assistant, None, false)?;
    suite.keys("cmd-,");
    suite
        .window
        .update(&mut suite.cx, |shell, _, cx| shell.fixture_models(cx))?;
    suite.capture("settings-model-closed", Route::Settings, None, false)?;
    suite.click_selector("codex-model-select")?;
    suite.capture("model-dropdown-open", Route::Settings, None, false)?;
    suite.keys("escape");
    suite.keys("cmd-5");
    suite.capture("terminal", Route::Terminal, None, false)?;
    println!(
        "{} real Metal frames passed, including region-removal negative controls",
        suite.count
    );
    Ok(())
}
