use crate::ui;
use crate::ui::{
    CONTROL_HEIGHT, FIELD_LABEL_GAP, PAGE_X, SETTINGS_INSET, SETTINGS_ROW_HEIGHT, SPACE_2, SPACE_3,
    STATUS_BAR_HEIGHT, TITLE_OPTICAL_LIFT, type_size,
};
use crate::{
    input,
    model::{FontSize, Overlay, PANE_WIDTHS, Route, Session},
    shell::{self, Shell},
    ui::Assets,
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

/// One pixel probe: a named logical rectangle, a brightness threshold and the
/// minimum number of pixels above it.
type Probe = (String, [u32; 4], u8, usize);

fn check_pixels(bytes: &[u8], pixel_width: u32, scale: u32, regions: &[Probe]) -> Result<()> {
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

/// The ⌘-number keystroke for a sidebar Route, in sidebar order.
fn go(route: Route) -> String {
    let index = crate::model::PAGES
        .iter()
        .filter(|page| page.in_sidebar)
        .position(|page| page.route == route)
        .expect("sidebar route");
    format!("cmd-{}", index + 1)
}

fn rect(bounds: Bounds<Pixels>) -> [u32; 4] {
    [
        f32::from(bounds.origin.x).max(0.) as u32,
        f32::from(bounds.origin.y).max(0.) as u32,
        f32::from(bounds.origin.x + bounds.size.width) as u32,
        f32::from(bounds.origin.y + bounds.size.height) as u32,
    ]
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

    // Check actual pixels in independent shell regions, rather than trusting scene/AX nodes.
    // Regions come from the current layout; thresholds are below normal text contrast.
    fn probes(&mut self, width: u32, dimmed: bool) -> Result<Vec<Probe>> {
        // The scrim leaves a fifth of each surface's brightness behind it.
        let text = if dimmed { 25 } else { 90 };
        let border = if dimmed { 5 } else { 20 };
        let mut probes: Vec<Probe> = vec![("header".into(), [150, 10, width - 10, 40], text, 80)];
        for (name, selector, minimum) in [
            ("workspace", "workspace-title", 60),
            ("profile name", "sidebar-profile-name", 30),
        ] {
            probes.push((name.into(), rect(self.bounds(selector)?), text, minimum));
        }
        // Tertiary text is dim by design; under a scrim it still has to be there.
        let tertiary = if dimmed { 12 } else { 60 };
        for (name, selector, minimum) in [
            ("profile handle", "sidebar-profile-handle", 20),
            ("status route", "status-bar.route", 30),
            ("status version", "sidebar-version", 12),
        ] {
            probes.push((name.into(), rect(self.bounds(selector)?), tertiary, minimum));
        }
        let profile = self.bounds("sidebar-profile")?;
        probes.push((
            "profile avatar".into(),
            rect(Bounds {
                origin: profile.origin + point(px(SPACE_2), px(0.)),
                size: size(px(24.), profile.size.height),
            }),
            text,
            30,
        ));
        let main = self.bounds("main-pane")?;
        probes.push((
            "main border".into(),
            [
                f32::from(main.origin.x) as u32,
                f32::from(main.origin.y) as u32 + 60,
                f32::from(main.origin.x) as u32 + 2,
                f32::from(main.origin.y + main.size.height) as u32 - 30,
            ],
            border,
            150,
        ));
        for index in 1..=6 {
            probes.push((
                format!("sidebar label {index}"),
                rect(self.bounds(&format!("sidebar-label-{index}"))?),
                text,
                35,
            ));
        }
        Ok(probes)
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
        let probes = self.probes(
            width,
            overlay.is_some_and(|o| o == Overlay::Search || o.is_dialog()),
        )?;
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
                CONTROL_HEIGHT,
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
        let handle = self.bounds("sidebar-profile-handle")?;
        near("compact profile height", f32::from(card.size.height), 44.)?;
        ensure!(
            name.origin.x + name.size.width <= card.origin.x + card.size.width,
            "profile name overflows its row"
        );
        near(
            "profile name and handle left edges",
            f32::from(name.origin.x),
            f32::from(handle.origin.x),
        )?;
        ensure!(
            name.origin.y + name.size.height <= handle.origin.y + px(GEOMETRY_TOLERANCE),
            "profile handle overlaps its name"
        );
        let status = self.bounds("status-bar")?;
        let version = self.bounds("sidebar-version")?;
        near(
            "version right inset in the status bar",
            f32::from(status.origin.x + status.size.width - version.origin.x - version.size.width),
            PAGE_X,
        )?;
        near(
            "version centred in the status bar",
            f32::from(version.origin.y + version.size.height / 2.),
            f32::from(status.origin.y + status.size.height / 2.),
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
            &format!("{label} SettingsRow top inset"),
            f32::from(text.origin.y - row.origin.y),
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
            f32::from(status.origin.x - main.origin.x),
            1.,
        )?;
        near(
            "status bar width",
            f32::from(status.size.width),
            f32::from(main.size.width) - 2.,
        )?;
        near(
            "status bar height",
            f32::from(status.size.height),
            STATUS_BAR_HEIGHT,
        )?;
        near(
            "status bar bottom edge",
            f32::from(main.origin.y + main.size.height - status.origin.y - status.size.height),
            1.,
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
        // The sidebar and the content card share one gap on every side.
        let sidebar = self.bounds("sidebar-content")?;
        let search = self.bounds("shell.search")?;
        near(
            "sidebar search left inset",
            f32::from(search.origin.x - sidebar.origin.x),
            SPACE_3,
        )?;
        near(
            "sidebar search right inset",
            f32::from(sidebar.origin.x + sidebar.size.width - search.origin.x - search.size.width),
            SPACE_3,
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
        let status = self.bounds("status-bar")?;
        let terminal_inset = if route == Route::Terminal {
            SPACE_2
        } else {
            0.
        };
        ensure!(
            frame.origin.y + frame.size.height <= status.origin.y,
            "{route:?} content overlaps status bar"
        );
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
            match self.bounds("page-leading") {
                Ok(leading) => near(
                    "page breadcrumb top inset",
                    f32::from(leading.origin.y - frame.origin.y),
                    PAGE_X - TITLE_OPTICAL_LIFT,
                )?,
                Err(_) => {
                    let title = self.bounds("page-title")?;
                    near(
                        "page title top inset",
                        f32::from(title.origin.y - frame.origin.y),
                        PAGE_X - TITLE_OPTICAL_LIFT,
                    )?;
                }
            }
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

    /// Every component specimen renders inside the page's content width.
    fn check_components_geometry(&mut self, specimens: &[&str]) -> Result<()> {
        let content = self.bounds("main-content")?;
        for specimen in specimens {
            let bounds = self.bounds(&format!("components.{specimen}"))?;
            ensure!(
                bounds.origin.x >= content.origin.x - px(GEOMETRY_TOLERANCE)
                    && bounds.origin.x + bounds.size.width
                        <= content.origin.x + content.size.width + px(GEOMETRY_TOLERANCE),
                "gallery specimen {specimen} leaves the page content"
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
    suite.capture("initial", Route::Dashboard, None, false)?;
    suite.check_profile_row_geometry()?;
    suite.window.update(&mut suite.cx, |shell, _, cx| {
        shell.fixture_profile_name(
            "A very long profile name that must truncate before the version",
            cx,
        );
    })?;
    suite.capture("profile-long-name", Route::Dashboard, None, false)?;
    suite.check_profile_row_geometry()?;
    suite.window.update(&mut suite.cx, |shell, _, cx| {
        shell.fixture_profile_name("QA Profile", cx);
    })?;
    let hover_target = suite.bounds("sidebar-label-2")?.center();
    suite.cx.simulate_mouse_move(
        suite.window.into(),
        hover_target,
        None::<MouseButton>,
        Modifiers::default(),
    );
    suite.capture("hover-tickets", Route::Dashboard, None, false)?;
    let workspace_card = suite.bounds("workspace-title")?.center();
    suite.cx.simulate_mouse_move(
        suite.window.into(),
        workspace_card,
        None::<MouseButton>,
        Modifiers::default(),
    );
    suite.capture("hover-workspace", Route::Dashboard, None, false)?;
    suite.cx.simulate_mouse_move(
        suite.window.into(),
        point(px(500.), px(500.)),
        None::<MouseButton>,
        Modifiers::default(),
    );
    // The user menu, its Support submenu, notifications and toasts.
    suite.click_selector("sidebar-profile")?;
    suite.capture(
        "user-menu",
        Route::Dashboard,
        Some(Overlay::UserMenu { support: false }),
        false,
    )?;
    let menu = suite.bounds("user-menu")?;
    let profile = suite.bounds("sidebar-profile")?;
    ensure!(
        menu.origin.y + menu.size.height <= profile.origin.y,
        "user menu must open above the user row"
    );
    near(
        "user menu aligns with the user row",
        f32::from(menu.origin.x),
        f32::from(profile.origin.x),
    )?;
    suite.click_selector("user-menu.support")?;
    suite.capture(
        "user-menu-support",
        Route::Dashboard,
        Some(Overlay::UserMenu { support: true }),
        false,
    )?;
    suite.bounds("user-menu.support.menu")?;
    suite.keys("escape");
    suite.capture("user-menu-closed", Route::Dashboard, None, false)?;
    suite.window.update(&mut suite.cx, |shell, _, cx| {
        shell.fixture_notifications(cx);
    })?;
    suite.click_selector("notifications")?;
    suite.capture(
        "notifications",
        Route::Dashboard,
        Some(Overlay::Notifications),
        false,
    )?;
    suite.bounds("notifications.panel")?;
    suite.keys("escape");
    suite.window.update(&mut suite.cx, |shell, _, cx| {
        shell.fixture_toast(cx);
    })?;
    suite.capture("toasts", Route::Dashboard, None, false)?;
    let toasts = suite.bounds("toasts")?;
    let status = suite.bounds("status-bar")?;
    ensure!(
        toasts.origin.y + toasts.size.height <= status.origin.y,
        "toasts must stack above the status bar"
    );
    suite.click_selector("toast.close.1")?;
    suite.click_selector("toast.close.2")?;
    suite.capture("toasts-dismissed", Route::Dashboard, None, false)?;
    ensure!(
        suite.bounds("toasts").is_err(),
        "dismissed toasts must leave the shell"
    );
    // The Dashboard, Smart Home and Calendar with a connected home and a
    // realistic fortnight of events.
    suite.window.update(&mut suite.cx, |shell, _, cx| {
        shell.fixture_life(true, cx);
    })?;
    suite.capture("dashboard", Route::Dashboard, None, false)?;
    for selector in [
        "dashboard.band",
        "dashboard.lights",
        "dashboard.upcoming",
        "dashboard.work-list",
    ] {
        suite.bounds(selector)?;
    }
    suite.click_selector("dashboard.switch.under_cabinet")?;
    suite.capture("dashboard-switched", Route::Dashboard, None, false)?;
    suite.keys(&go(Route::SmartHome));
    suite.capture("smart-home", Route::SmartHome, None, false)?;
    for selector in [
        "home.climate",
        "home.rooms",
        "home.history",
        "home.climate.target",
    ] {
        suite.bounds(selector)?;
    }
    suite.click_selector("home.climate.mode.3")?;
    suite.capture("smart-home-auto", Route::SmartHome, None, false)?;
    suite.bounds("home.climate.low")?;
    suite.bounds("home.climate.high")?;
    suite.keys(&go(Route::Calendar));
    suite.capture("calendar-month", Route::Calendar, None, false)?;
    suite.bounds("calendar.month")?;
    suite.bounds("calendar.day-panel")?;
    suite.click_selector("calendar.view.1")?;
    suite.capture("calendar-week", Route::Calendar, None, false)?;
    suite.bounds("calendar.week")?;
    suite.click_selector("calendar.view.0")?;
    suite.capture("calendar-agenda", Route::Calendar, None, false)?;
    suite.bounds("calendar.agenda")?;
    suite.click_selector("calendar.new")?;
    suite.capture(
        "calendar-new-event",
        Route::Calendar,
        Some(Overlay::CalendarEvent),
        false,
    )?;
    suite.cx.simulate_input(window.into(), "Review the roadmap");
    suite.capture(
        "calendar-new-typed",
        Route::Calendar,
        Some(Overlay::CalendarEvent),
        false,
    )?;
    suite.keys("escape");
    suite.click_selector("calendar.row.own")?;
    suite.capture(
        "calendar-edit-event",
        Route::Calendar,
        Some(Overlay::CalendarEvent),
        false,
    )?;
    suite.bounds("calendar.delete")?;
    suite.keys("escape");
    suite.click_selector("calendar.row.dentist")?;
    suite.capture(
        "calendar-mirror-event",
        Route::Calendar,
        Some(Overlay::CalendarEvent),
        false,
    )?;
    suite.bounds("calendar.done")?;
    suite.keys("escape");
    suite.window.update(&mut suite.cx, |shell, _, cx| {
        shell.fixture_life(false, cx);
    })?;
    suite.keys(&go(Route::SmartHome));
    suite.capture("smart-home-disconnected", Route::SmartHome, None, false)?;
    suite.bounds("home.empty")?;
    suite.keys("cmd-,");
    suite.capture("settings-home", Route::Settings, None, false)?;
    suite.bounds("settings.section.Smart Home")?;
    suite.bounds("settings.section.Calendar")?;
    suite.window.update(&mut suite.cx, |shell, _, cx| {
        shell.fixture_life(true, cx);
    })?;
    let now = chrono::Utc::now().timestamp_millis();
    let execution = |workflow_type: &str,
                     workflow_id: &str,
                     run_id: &str,
                     status: &str,
                     minutes_ago: i64,
                     closed_after: Option<i64>| {
        let started_at = now - minutes_ago * 60_000;
        ainc_client::types::ExecutionView {
            workflow_id: workflow_id.into(),
            run_id: run_id.into(),
            workflow_type: workflow_type.into(),
            status: status.into(),
            started_at,
            closed_at: closed_after.map(|seconds| started_at + seconds * 1000),
            url: Some(format!(
                "http://127.0.0.1:8080/namespaces/agentinc/workflows/{workflow_id}/{run_id}/history"
            )),
        }
    };
    let executions = vec![
        execution(
            "agentinc.run",
            "ticket/4821:reconcile-weekly-budget-and-receipts",
            "8a37e3d2-6a42-4918-a5d2-98fc38ea2274",
            "Running",
            3,
            None,
        ),
        execution(
            "turnkeel.occurrence",
            "automation/weekday-morning-review-for-the-family-and-household",
            "175f70bd-ffb3-4c3a-9aad-90c8c979ecb1",
            "Completed",
            18,
            Some(42),
        ),
        execution(
            "agentinc.session",
            "conversation/9332",
            "c8915b43-b5b4-4acf-8d7c-1bd9d7a5ca74",
            "Failed",
            64,
            Some(14),
        ),
        execution(
            "agentinc.run",
            "ticket/4790",
            "264d4aaa-20ae-4050-9ff5-5822f4125c6e",
            "Canceled",
            170,
            Some(65),
        ),
        execution(
            "turnkeel.occurrence",
            "automation/house-check",
            "74e812df-2eac-4eea-920d-876633bef27a",
            "TimedOut",
            1_460,
            Some(3_600),
        ),
    ];
    suite.window.update(&mut suite.cx, |shell, _, cx| {
        shell.fixture_temporal(
            ainc_client::types::ExecutionPage {
                executions: executions.clone(),
                next_page: Some("next".into()),
                ui_available: true,
            },
            cx,
        );
    })?;
    suite.keys(&go(Route::Temporal));
    suite.capture("temporal-populated", Route::Temporal, None, false)?;
    suite.bounds("temporal.row.8a37e3d2-6a42-4918-a5d2-98fc38ea2274")?;
    let table = suite.bounds("temporal.table")?;
    let status = suite.bounds("status-bar")?;
    ensure!(
        table.origin.y + table.size.height < status.origin.y,
        "Temporal table overlaps bottom status bar"
    );
    suite.window.update(&mut suite.cx, |shell, _, cx| {
        shell.fixture_temporal_loading(cx);
    })?;
    suite.capture("temporal-loading", Route::Temporal, None, false)?;
    suite.bounds("temporal.loading")?;
    suite.window.update(&mut suite.cx, |shell, _, cx| {
        shell.fixture_temporal(
            ainc_client::types::ExecutionPage {
                executions: Vec::new(),
                next_page: None,
                ui_available: true,
            },
            cx,
        );
    })?;
    suite.capture("temporal-empty", Route::Temporal, None, false)?;
    suite.bounds("temporal.empty")?;
    suite.window.update(&mut suite.cx, |shell, _, cx| {
        shell.fixture_temporal_error(cx);
    })?;
    suite.capture("temporal-error", Route::Temporal, None, false)?;
    suite.bounds("temporal.error")?;
    suite.window.update(&mut suite.cx, |shell, _, cx| {
        shell.fixture_temporal(
            ainc_client::types::ExecutionPage {
                executions: executions.clone(),
                next_page: Some("next".into()),
                ui_available: true,
            },
            cx,
        );
    })?;
    for round in 0..3 {
        if round == 2 {
            window = suite
                .cx
                .open_offscreen_window(size(px(800.), px(600.)), |window, cx| {
                    cx.new(|cx| {
                        Shell::fixture(temporary.path().join("min-session.json"), window, cx)
                    })
                })?;
            suite.window = window;
            ensure!(
                suite
                    .cx
                    .update_window(window.into(), |_, window, _| window.viewport_size())?
                    == size(px(800.), px(600.)),
                "minimum window dimensions"
            );
        }
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
            suite.window.update(&mut suite.cx, |shell, _, cx| {
                shell.fixture_temporal(
                    ainc_client::types::ExecutionPage {
                        executions: vec![execution(
                            "agentinc.run",
                            "ticket/4821:reconcile-weekly-budget-and-receipts",
                            "8a37e3d2-6a42-4918-a5d2-98fc38ea2274",
                            "Running",
                            3,
                            None,
                        )],
                        next_page: None,
                        ui_available: false,
                    },
                    cx,
                );
            })?;
            suite.keys(&go(Route::Temporal));
            suite.capture("temporal-small-no-ui", Route::Temporal, None, false)?;
            suite.bounds("temporal.no-ui")?;
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
            suite.keys(&go(route));
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
        if round == 0 {
            suite.window.update(&mut suite.cx, |shell, _, cx| {
                shell.fixture_recent_commands(&["page.tickets", "action.check-updates"], cx);
            })?;
            suite.click_selector("shell.search")?;
        } else {
            suite.keys("cmd-k");
        }
        if round == 0 {
            suite.capture(
                "search-empty",
                Route::Automations,
                Some(Overlay::Search),
                false,
            )?;
            suite.bounds("palette.result.recent.page.tickets")?;
            suite.bounds("palette.result.pages.page.tickets")?;
            suite.keys("down");
            suite.keys("down");
            suite.capture(
                "search-keyboard",
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
        suite.keys(&go(Route::Automations));
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
        for (shortcut, route) in [(5, Route::Terminal), (6, Route::Temporal)] {
            suite.keys(&go(route));
            suite.capture(&format!("route-{round}-{shortcut}"), route, None, false)?;
        }
        for route in [Route::Dashboard, Route::Calendar, Route::SmartHome] {
            suite.keys(&go(route));
            suite.capture(&format!("life-{round}-{route:?}"), route, None, false)?;
        }
    }
    suite.keys(&go(Route::Tickets));
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
    let palette = suite.bounds("search.dialog")?;
    let status = suite.bounds("status-bar")?;
    ensure!(
        palette.origin.y + palette.size.height <= status.origin.y,
        "the palette must clear the status bar at the minimum window size"
    );
    suite.cx.simulate_input(window.into(), "zzzz");
    suite.capture(
        "search-no-matches",
        Route::Tickets,
        Some(Overlay::Search),
        false,
    )?;
    suite.bounds("palette.empty")?;
    suite.keys("escape");
    suite.keys("cmd-,");
    suite.capture("settings-shortcut", Route::Settings, None, false)?;
    // Keyboard navigation reveals focus rings; a pointer press hides them again.
    suite.keys("tab");
    suite.capture("focus-ring", Route::Settings, None, false)?;
    suite.click_selector("titlebar-center-space")?;
    // A Ticket with an agent, a status, an assignee and a Comment.
    suite.window.update(&mut suite.cx, |shell, _, cx| {
        shell.fixture_ticket_detail(cx);
    })?;
    suite.capture("ticket-detail", Route::Tickets, None, false)?;
    suite.bounds("tickets.status.in_progress")?;
    suite.click_selector("tickets.back")?;
    suite.capture("tickets-list", Route::Tickets, None, false)?;
    suite.bounds("ticket.1")?;
    suite.keys(&go(Route::Agents));
    suite.capture("agents-list", Route::Agents, None, false)?;
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
    let closed = suite.bounds("settings.row.Model")?;
    suite.click_selector("codex-model-select")?;
    suite.capture("model-dropdown-open", Route::Settings, None, false)?;
    suite.bounds("codex-model-select.menu")?;
    let open = suite.bounds("settings.row.Model")?;
    near(
        "open select never resizes its row",
        f32::from(open.size.height),
        f32::from(closed.size.height),
    )?;
    suite.keys("escape");
    suite.capture("model-dropdown-closed", Route::Settings, None, false)?;
    ensure!(
        suite.bounds("codex-model-select.menu").is_err(),
        "escape must close the model select"
    );
    // Choosing an option selects that model, closes the menu and keeps the
    // row's height.
    suite.click_selector("codex-model-select")?;
    suite.click_selector("codex-model-select.option.1")?;
    suite.capture("model-dropdown-selected", Route::Settings, None, false)?;
    ensure!(
        suite.bounds("codex-model-select.menu").is_err(),
        "choosing an option must close the model select"
    );
    near(
        "select keeps its row height after a choice",
        f32::from(suite.bounds("settings.row.Model")?.size.height),
        f32::from(closed.size.height),
    )?;
    let chosen = suite
        .window
        .read_with(&suite.cx, |shell, cx| shell.fixture_selected_model(cx))?;
    ensure!(
        chosen.as_deref() == Some("model-one"),
        "choosing an option must select that model, got {chosen:?}"
    );
    // The component gallery, one capture per section, reached through the palette.
    suite.keys("cmd-k");
    suite.cx.simulate_input(window.into(), "components");
    suite.keys("enter");
    suite.capture("components-buttons", Route::Components, None, false)?;
    suite.check_components_geometry(&["variants", "sizes-and-icons", "states"])?;
    let primary = suite.bounds("components.regular")?;
    let large = suite.bounds("components.large")?;
    near(
        "regular button height",
        f32::from(primary.size.height),
        CONTROL_HEIGHT,
    )?;
    ensure!(
        large.size.height > primary.size.height,
        "large buttons must be taller than regular ones"
    );
    suite.window.update(&mut suite.cx, |shell, _, cx| {
        shell.fixture_components(1, false, cx);
    })?;
    suite.capture("components-inputs", Route::Components, None, false)?;
    suite.check_components_geometry(&[
        "fields",
        "text-area",
        "select",
        "toggles-and-checkboxes",
    ])?;
    suite.window.update(&mut suite.cx, |shell, _, cx| {
        shell.fixture_components(1, true, cx);
    })?;
    suite.capture("components-select-open", Route::Components, None, false)?;
    suite.bounds("components.select.menu")?;
    suite.keys("escape");
    suite.settle()?;
    ensure!(
        suite.bounds("components.select.menu").is_err(),
        "escape must close the components select"
    );
    suite.window.update(&mut suite.cx, |shell, _, cx| {
        shell.fixture_components(2, false, cx);
    })?;
    suite.capture("components-data", Route::Components, None, false)?;
    suite.check_components_geometry(&[
        "badges-and-status",
        "list-rows",
        "table",
        "empty-and-loading",
    ])?;
    suite.window.update(&mut suite.cx, |shell, _, cx| {
        shell.fixture_components(3, false, cx);
    })?;
    suite.capture("components-overlays", Route::Components, None, false)?;
    suite.check_components_geometry(&["dialog", "sheet", "menus-and-popovers", "toasts"])?;
    suite.keys(&go(Route::Terminal));
    suite.capture("terminal", Route::Terminal, None, false)?;
    suite.window.update(&mut suite.cx, |shell, _, cx| {
        shell.fixture_terminal_unavailable(cx);
    })?;
    suite.capture("terminal-unavailable", Route::Terminal, None, false)?;
    suite.bounds("terminal.unavailable")?;
    println!(
        "{} real Metal frames passed, including region-removal negative controls",
        suite.count
    );
    Ok(())
}
