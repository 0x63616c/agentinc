use crate::ui;
use crate::ui::{
    CONTROL_HEIGHT, FIELD_LABEL_GAP, PAGE_X, SETTINGS_INSET, SETTINGS_ROW_HEIGHT, SPACE_2, SPACE_3,
    STATUS_BAR_HEIGHT, STATUS_BAR_X, TITLE_OPTICAL_LIFT, type_size,
};
use crate::{
    input,
    overlay::Overlay,
    routes::Route,
    shell::{self, Shell},
    ui::Assets,
    ui_state::{FontSize, SIDEBAR_DEFAULT, UiState},
};
use anyhow::{Result, ensure};
use gpui::prelude::*;
use gpui::{
    AppContext, Bounds, IntoElement, Modifiers, MouseButton, Pixels, Point, Render,
    VisualTestAppContext, Window, WindowHandle, div, point, px, size,
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

    fn check_assignee_photo(&mut self, selector: &str, present: bool) -> Result<()> {
        let bounds = rect(self.bounds(selector)?);
        let scale = self.cx.update_window(self.window.into(), |_, window, _| {
            window.scale_factor() as u32
        })?;
        let image = self.cx.capture_screenshot(self.window.into())?;
        let color = ui::LABEL_COLORS[0];
        let expected = [(color >> 16) as u8, (color >> 8) as u8, color as u8];
        let mut matching = 0;
        for y in bounds[1] * scale..bounds[3] * scale {
            for x in bounds[0] * scale..bounds[2] * scale {
                if image.get_pixel(x, y).0[..3] == expected {
                    matching += 1;
                }
            }
        }
        ensure!(
            (matching >= 30) == present,
            "{selector}: local profile photo presence should be {present}"
        );
        Ok(())
    }

    fn check_label_menu_anchor(&mut self) -> Result<()> {
        let trigger = self.bounds("tickets.labels.open")?;
        let menu = self.bounds("tickets.labels.menu")?;
        near(
            "label menu left edge follows its button",
            f32::from(menu.origin.x),
            f32::from(trigger.origin.x),
        )?;
        if menu.origin.y >= trigger.origin.y + trigger.size.height {
            near(
                "label menu gap below its button",
                f32::from(menu.origin.y - trigger.origin.y - trigger.size.height),
                ui::SPACE_1,
            )
        } else {
            // The shared anchored menu flips above when the window is too short.
            near(
                "flipped label menu stays attached to its button",
                f32::from(trigger.origin.y - menu.origin.y - menu.size.height),
                0.,
            )
        }
    }

    fn check_button_icon_order(&mut self, selector: &str, trailing: bool) -> Result<()> {
        let label = self.bounds(&format!("{selector}.label"))?;
        let icon = self.bounds(&format!("{selector}.icon"))?;
        if trailing {
            ensure!(
                icon.origin.x >= label.origin.x + label.size.width,
                "{selector}: + must follow the label"
            );
        } else {
            ensure!(
                icon.origin.x + icon.size.width <= label.origin.x,
                "{selector}: other icons must lead the label"
            );
        }
        near(
            &format!("{selector} icon is vertically centered"),
            f32::from(icon.center().y),
            f32::from(label.center().y),
        )
    }

    // Check actual pixels in independent shell regions, rather than trusting scene/AX nodes.
    // Regions come from the current layout; thresholds are below normal text contrast.
    fn probes(&mut self, width: u32, dimmed: bool) -> Result<Vec<Probe>> {
        // The scrim leaves a fifth of each surface's brightness behind it.
        let text = if dimmed { 25 } else { 90 };
        let border = if dimmed { 5 } else { 20 };
        let mut probes: Vec<Probe> = vec![("header".into(), [150, 10, width - 10, 40], text, 80)];
        probes.push((
            "profile name".into(),
            rect(self.bounds("sidebar-profile-name")?),
            text,
            30,
        ));
        // Tertiary text is dim by design; under a scrim it still has to be there.
        let tertiary = if dimmed { 12 } else { 60 };
        probes.push((
            "status version".into(),
            rect(self.bounds("sidebar-version")?),
            tertiary,
            12,
        ));
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
            overlay.is_some_and(|o| o == Overlay::GoTo || o.is_dialog()),
        )?;
        check_pixels(image.as_raw(), image.width(), scale, &probes)
            .map_err(|e| anyhow::anyhow!("{name}: {e}"))?;
        self.check_page_geometry(route)?;
        if name == "route-1-0" {
            self.check_shell_geometry()?;
        }
        if name.starts_with("ticket-field-") {
            self.check_field_geometry("Title")?;
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
            self.check_button_icon_order("tickets.create", true)?;
        }
        if name == "route-0-2" {
            self.check_button_icon_order("agents.create", true)?;
            self.check_button_icon_order("agents.create.empty", true)?;
        }
        if name == "automations-0" {
            self.check_button_icon_order("automations.create", true)?;
            self.check_button_icon_order("automations.create.empty", true)?;
        }
        if name == "assistant-conversation-list" {
            self.check_button_icon_order("new-conversation", true)?;
        }
        if name == "ticket-create-dialog" {
            self.check_button_icon_order("tickets.labels.open", true)?;
        }
        if name == "components-buttons" {
            self.check_button_icon_order("components.small", true)?;
            self.check_button_icon_order("components.regular", true)?;
            self.check_button_icon_order("components.trailing", false)?;
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
        near("compact profile height", f32::from(card.size.height), 44.)?;
        ensure!(
            name.origin.x + name.size.width <= card.origin.x + card.size.width,
            "profile name overflows its row"
        );
        let status = self.bounds("status-bar")?;
        let version = self.bounds("sidebar-version")?;
        near(
            "version right inset in the status bar",
            f32::from(status.origin.x + status.size.width - version.origin.x - version.size.width),
            STATUS_BAR_X,
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

fn press(suite: &mut Suite, position: Point<Pixels>) {
    suite.cx.simulate_mouse_down(
        suite.window.into(),
        position,
        MouseButton::Left,
        Modifiers::default(),
    );
}
fn drag_to(suite: &mut Suite, from: Point<Pixels>, to: Point<Pixels>) {
    // Several moves, as a pointer does: past the drag threshold, then across.
    for step in 1..=8 {
        let t = step as f32 / 8.;
        let position = point(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t);
        suite.cx.simulate_mouse_move(
            suite.window.into(),
            position,
            MouseButton::Left,
            Modifiers::default(),
        );
    }
}
fn release(suite: &mut Suite, position: Point<Pixels>) {
    suite.cx.simulate_mouse_up(
        suite.window.into(),
        position,
        MouseButton::Left,
        Modifiers::default(),
    );
}

/// The board, a real pointer drag between and within lanes, filters, the
/// list, a rich detail, its menus and dialogs, and ⌘K Tickets.
fn tickets_suite(suite: &mut Suite, window: WindowHandle<Shell>) -> Result<()> {
    use ainc_client::types::TicketStatus;
    let page = suite
        .window
        .read_with(&suite.cx, |shell, _| shell.fixture_tickets_page())?;
    suite.keys("cmd-1");
    let budget = page.update(&mut suite.cx, |page, cx| page.fixture_board(cx));
    // A synthetic local photo keeps this independent of the macOS account.
    let color = ui::LABEL_COLORS[0];
    let photo = image::RgbaImage::from_pixel(
        16,
        16,
        image::Rgba([(color >> 16) as u8, (color >> 8) as u8, color as u8, 255]),
    );
    let mut bytes = std::io::Cursor::new(Vec::new());
    photo.write_to(&mut bytes, image::ImageFormat::Png)?;
    page.update(&mut suite.cx, |page, cx| {
        page.set_owner(
            "Calum",
            Some(Arc::new(gpui::Image::from_bytes(
                gpui::ImageFormat::Png,
                bytes.into_inner(),
            ))),
            cx,
        );
    });
    let id_of = |suite: &mut Suite, title: &str| {
        page.read_with(&suite.cx, |page, _| page.fixture_ticket_id(title))
            .ok_or_else(|| anyhow::anyhow!("missing fixture Ticket {title}"))
    };
    suite.capture("tickets-board", Route::Tickets, None, false)?;
    let add = suite.bounds("tickets.lane.backlog.create")?;
    let plus = suite.bounds("tickets.lane.backlog.create.icon")?;
    near(
        "icon-only + stays centered",
        f32::from(plus.center().x),
        f32::from(add.center().x),
    )?;
    near(
        "icon-only + stays square",
        f32::from(add.size.width),
        f32::from(add.size.height),
    )?;
    // Lanes: one per status, headers on one line, the first on the content rail,
    // cards inset evenly inside their lane, everything above the status bar.
    let content = suite.bounds("main-content")?;
    let status_bar = suite.bounds("status-bar")?;
    let backlog = suite.bounds("tickets.lane.backlog")?;
    near(
        "first lane on the content rail",
        f32::from(backlog.origin.x),
        f32::from(content.origin.x),
    )?;
    ensure!(
        backlog.origin.y + backlog.size.height
            <= status_bar.origin.y - px(PAGE_X) + px(GEOMETRY_TOLERANCE),
        "board lanes must keep the page inset above the status bar"
    );
    let header_top = suite.bounds("tickets.lane.backlog.header")?.origin.y;
    for key in ["to_do", "in_progress", "blocked", "done", "cancelled"] {
        let header = suite.bounds(&format!("tickets.lane.{key}.header"))?;
        near(
            &format!("{key} lane header line"),
            f32::from(header.origin.y),
            f32::from(header_top),
        )?;
    }
    let dentist = id_of(suite, "Book a dentist cleaning")?;
    let card = suite.bounds(&format!("ticket.{budget}"))?;
    let lane = suite.bounds("tickets.lane.in_progress")?;
    near(
        "card inset inside its lane",
        f32::from(card.origin.x - lane.origin.x),
        ui::BOARD_LANE_INSET + 1.,
    )?;
    near(
        "card right inset inside its lane",
        f32::from(lane.origin.x + lane.size.width - card.origin.x - card.size.width),
        ui::BOARD_LANE_INSET + 1.,
    )?;
    // Drag "Book a dentist cleaning" from To do to below the first In progress card.
    let from = suite.bounds(&format!("ticket.{dentist}"))?.center();
    let first = page.read_with(&suite.cx, |page, _| {
        page.fixture_column(TicketStatus::InProgress)
    })[0];
    let first_bounds = suite.bounds(&format!("ticket.{first}"))?;
    let to = point(
        first_bounds.center().x,
        first_bounds.origin.y + first_bounds.size.height - px(4.),
    );
    press(suite, from);
    drag_to(suite, from, to);
    suite.capture("tickets-drag", Route::Tickets, None, false)?;
    let target = page.read_with(&suite.cx, |page, _| page.fixture_drop_target());
    ensure!(
        target == Some((TicketStatus::InProgress, Some(first))),
        "drop target follows the pointer, got {target:?}"
    );
    release(suite, to);
    suite.capture("tickets-dropped", Route::Tickets, None, false)?;
    let (status, column) = page.read_with(&suite.cx, |page, _| {
        (
            page.fixture_status(dentist),
            page.fixture_column(TicketStatus::InProgress),
        )
    });
    ensure!(
        status == Some(TicketStatus::InProgress) && column.get(1) == Some(&dentist),
        "dropped card must land below its neighbour, got {status:?} {column:?}"
    );
    // Reorder inside a lane: the last To do card goes to the top.
    let todo = page.read_with(&suite.cx, |page, _| page.fixture_column(TicketStatus::ToDo));
    let (top, last) = (todo[0], *todo.last().unwrap());
    let from = suite.bounds(&format!("ticket.{last}"))?.center();
    let top_bounds = suite.bounds(&format!("ticket.{top}"))?;
    let to = point(top_bounds.center().x, top_bounds.origin.y + px(4.));
    press(suite, from);
    drag_to(suite, from, to);
    release(suite, to);
    suite.settle()?;
    let reordered = page.read_with(&suite.cx, |page, _| page.fixture_column(TicketStatus::ToDo));
    ensure!(
        reordered.first() == Some(&last) && reordered.len() == todo.len(),
        "reordering inside a lane moves the card to the top, got {reordered:?}"
    );
    // Filters, then the list over the same Tickets.
    page.update(&mut suite.cx, |page, cx| {
        page.fixture_menu(Some("tickets.filter.priority"), cx)
    });
    suite.capture("tickets-filter-menu", Route::Tickets, None, false)?;
    suite.bounds("tickets.filter.priority.menu")?;
    page.update(&mut suite.cx, |page, cx| {
        page.fixture_menu(None, cx);
        page.fixture_filter_label("Money", cx);
    });
    suite.capture("tickets-board-filtered", Route::Tickets, None, false)?;
    suite.bounds("tickets.filter.clear")?;
    suite.click_selector("tickets.filter.clear")?;
    page.update(&mut suite.cx, |page, cx| page.fixture_view(true, cx));
    suite.capture("tickets-list-view", Route::Tickets, None, false)?;
    suite.bounds("tickets.list")?;
    suite.bounds("tickets.group.in_progress")?;
    // One Ticket with description, history, Comments, relationships and a run.
    suite.click_selector(&format!("ticket.{budget}"))?;
    suite.capture("ticket-detail-rich", Route::Tickets, None, false)?;
    let properties = suite.bounds("tickets.properties")?;
    let content = suite.bounds("main-content")?;
    near(
        "properties on the content's right edge",
        f32::from(
            content.origin.x + content.size.width - properties.origin.x - properties.size.width,
        ),
        0.,
    )?;
    suite.bounds("tickets.timeline")?;
    suite.bounds("tickets.description.text")?;
    page.update(&mut suite.cx, |page, cx| {
        page.fixture_menu(Some("tickets.detail.assignee"), cx)
    });
    suite.capture("ticket-assignee-photo-menu", Route::Tickets, None, false)?;
    suite.check_assignee_photo("tickets.detail.assignee.option.0", true)?;
    page.update(&mut suite.cx, |page, cx| {
        page.fixture_menu(Some("tickets.detail.status"), cx)
    });
    suite.capture("ticket-status-menu", Route::Tickets, None, false)?;
    suite.bounds("tickets.detail.status.menu")?;
    page.update(&mut suite.cx, |page, cx| {
        page.fixture_menu(Some("tickets.labels"), cx)
    });
    suite.capture("ticket-labels-menu", Route::Tickets, None, false)?;
    suite.bounds("tickets.labels.menu")?;
    page.update(&mut suite.cx, |page, cx| page.fixture_menu(None, cx));
    // Relate through the dialog itself: search, choose, confirm.
    suite.click_selector("tickets.link")?;
    suite.capture(
        "ticket-link-dialog-empty",
        Route::Tickets,
        Some(Overlay::Dialog(Route::Tickets)),
        false,
    )?;
    suite.cx.simulate_input(window.into(), "dentist");
    suite.click_selector(&format!("tickets.link.target.{dentist}"))?;
    suite.capture(
        "ticket-link-dialog",
        Route::Tickets,
        Some(Overlay::Dialog(Route::Tickets)),
        false,
    )?;
    suite.bounds("tickets.link.candidates")?;
    suite.click_selector("tickets.submit")?;
    suite.settle()?;
    let related = page.read_with(&suite.cx, |page, _| page.fixture_relations(budget));
    ensure!(
        related.contains(&(crate::tickets::model::Relation::BlockedBy, dentist)),
        "the chosen Ticket becomes a blocker, got {related:?}"
    );
    suite.click_selector("tickets.back")?;
    page.update(&mut suite.cx, |page, cx| page.select(dentist, cx));
    suite.capture("ticket-owner-photo", Route::Tickets, None, false)?;
    suite.check_assignee_photo("tickets.detail.assignee", true)?;
    suite.click_selector("tickets.back")?;
    page.update(&mut suite.cx, |page, cx| page.fixture_view(false, cx));
    suite.settle()?;
    // Quick create from a lane starts in that lane's status.
    suite.click_selector("tickets.lane.blocked.create")?;
    suite.capture(
        "ticket-create-dialog",
        Route::Tickets,
        Some(Overlay::Dialog(Route::Tickets)),
        false,
    )?;
    suite.check_assignee_photo("tickets.draft.assignee", true)?;
    suite.click_selector("tickets.draft.assignee")?;
    suite.settle()?;
    suite.check_assignee_photo("tickets.draft.assignee.option.0", true)?;
    page.update(&mut suite.cx, |page, cx| page.fixture_menu(None, cx));
    suite.click_selector("tickets.labels.open")?;
    suite.capture(
        "ticket-create-label-menu",
        Route::Tickets,
        Some(Overlay::Dialog(Route::Tickets)),
        false,
    )?;
    suite.check_label_menu_anchor()?;
    suite.cx.simulate_input(window.into(), "test");
    suite.settle()?;
    suite.check_label_menu_anchor()?;
    suite.check_button_icon_order("tickets.labels.create", true)?;
    suite.click_selector("tickets.labels.create")?;
    suite.settle()?;
    // Adding a tag changes the trigger to a compact +; its menu follows it.
    suite.check_label_menu_anchor()?;
    suite.bounds("tickets.labels.remove.test")?;
    page.update(&mut suite.cx, |page, cx| page.fixture_menu(None, cx));
    suite.click_selector("tickets.draft.assignee")?;
    page.update(&mut suite.cx, |page, cx| {
        page.set_owner("Calum", None, cx);
    });
    suite.capture(
        "ticket-create-assignee-fallback",
        Route::Tickets,
        Some(Overlay::Dialog(Route::Tickets)),
        false,
    )?;
    suite.bounds("tickets.draft.assignee.option.0")?;
    suite.check_assignee_photo("tickets.draft.assignee", false)?;
    suite.check_assignee_photo("tickets.draft.assignee.option.0", false)?;
    page.update(&mut suite.cx, |page, cx| page.fixture_menu(None, cx));
    suite.click_selector("Title.input")?;
    page.update(&mut suite.cx, |page, cx| {
        page.fixture_menu(Some("tickets.draft.status"), cx)
    });
    suite.capture(
        "ticket-create-status-menu",
        Route::Tickets,
        Some(Overlay::Dialog(Route::Tickets)),
        false,
    )?;
    suite.bounds("tickets.draft.status.option.3")?;
    page.update(&mut suite.cx, |page, cx| page.fixture_menu(None, cx));
    suite
        .cx
        .simulate_input(window.into(), "Order a new passport photo");
    suite.click_selector("tickets.submit")?;
    suite.settle()?;
    let created = id_of(suite, "Order a new passport photo")?;
    ensure!(
        page.read_with(&suite.cx, |page, _| page.fixture_status(created))
            == Some(TicketStatus::Blocked),
        "a lane's quick create starts in that lane"
    );
    // ⌘K finds a Ticket by title and opens it; it also creates one.
    suite.keys("cmd-k");
    suite.cx.simulate_input(window.into(), "dentist");
    suite.capture(
        "palette-ticket-search",
        Route::Tickets,
        Some(Overlay::GoTo),
        false,
    )?;
    suite.bounds(&format!("palette.result.tickets.goto-ticket.{dentist}"))?;
    suite.keys("enter");
    suite.capture("ticket-from-palette", Route::Tickets, None, false)?;
    suite.bounds("tickets.properties")?;
    suite.keys("cmd-k");
    suite.cx.simulate_input(window.into(), "new ticket");
    suite.keys("enter");
    suite.capture(
        "palette-new-ticket",
        Route::Tickets,
        Some(Overlay::Dialog(Route::Tickets)),
        false,
    )?;
    suite.keys("escape");
    suite.click_selector("tickets.back")?;
    suite.settle()?;
    Ok(())
}

pub fn run() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let session_path = temporary.path().join("session.json");
    let small_session_path = temporary.path().join("small-session.json");
    if std::env::var_os("AINC_RENDER_LARGER").is_some() {
        let mut ui_state = UiState::default();
        ui_state.font_size = FontSize::Larger;
        ui_state.save(&session_path)?;
        ui_state.save(&small_session_path)?;
    }
    let output = std::env::var_os("AINC_RENDER_OUTPUT")
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
    let hover_target = suite.bounds("sidebar-label-1")?.center();
    suite.cx.simulate_mouse_move(
        suite.window.into(),
        hover_target,
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
    // The user menu, its Support submenu, the signed-out Assistant and toasts.
    suite.click_selector("sidebar-profile")?;
    suite.capture(
        "user-menu",
        Route::Assistant,
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
    let divider = suite.bounds("user-menu.divider")?;
    let updates = suite.bounds("user-menu.updates")?;
    let settings = suite.bounds("user-menu.settings")?;
    let support = suite.bounds("user-menu.support")?;
    for (name, upper, lower) in [
        ("divider to updates", divider, updates),
        ("updates to settings", updates, settings),
        ("settings to support", settings, support),
    ] {
        near(
            name,
            f32::from(lower.origin.y - upper.bottom()),
            ui::MENU_INSET,
        )?;
    }
    let chevron = suite.bounds("user-menu.support.chevron")?;
    near(
        "Support chevron has its full icon width",
        f32::from(chevron.size.width),
        ui::ICON_SIZE,
    )?;
    ensure!(
        chevron.right() <= support.right() && chevron.bottom() <= support.bottom(),
        "Support chevron must fit inside its menu row"
    );
    suite.click_selector("user-menu.support")?;
    suite.capture(
        "user-menu-support",
        Route::Assistant,
        Some(Overlay::UserMenu { support: true }),
        false,
    )?;
    suite.bounds("user-menu.support.menu")?;
    let help = suite.bounds("user-menu.help")?;
    let feedback = suite.bounds("user-menu.feedback")?;
    let about = suite.bounds("user-menu.about")?;
    for (name, upper, lower) in [
        ("help to feedback", help, feedback),
        ("feedback to about", feedback, about),
    ] {
        near(
            name,
            f32::from(lower.origin.y - upper.bottom()),
            ui::MENU_INSET,
        )?;
    }
    suite.keys("escape");
    suite.capture("user-menu-closed", Route::Assistant, None, false)?;
    // The Conversation view before ChatGPT is connected: one empty state, one action.
    suite.window.update(&mut suite.cx, |shell, _, cx| {
        shell.fixture_signed_out(cx);
    })?;
    suite.capture("assistant-signed-out", Route::Assistant, None, false)?;
    suite.bounds("assistant.signed-out")?;
    suite.click_selector("back-to-conversations")?;
    suite.window.update(&mut suite.cx, |shell, _, cx| {
        shell.fixture_toast(cx);
    })?;
    suite.capture("toasts", Route::Assistant, None, false)?;
    let toasts = suite.bounds("toasts")?;
    let status = suite.bounds("status-bar")?;
    ensure!(
        toasts.origin.y + toasts.size.height <= status.origin.y,
        "toasts must stack above the status bar"
    );
    suite.click_selector("toast.close.1")?;
    suite.click_selector("toast.close.2")?;
    suite.capture("toasts-dismissed", Route::Assistant, None, false)?;
    ensure!(
        suite.bounds("toasts").is_err(),
        "dismissed toasts must leave the shell"
    );
    let now = chrono::Utc::now().timestamp_millis();
    use ainc_client::types::{WorkKind, WorkStatus};
    let execution = |kind: WorkKind,
                     id: &str,
                     status: WorkStatus,
                     minutes_ago: i64,
                     closed_after: Option<i64>| {
        let started_at = now - minutes_ago * 60_000;
        ainc_client::types::WorkView {
            id: id.into(),
            kind,
            status,
            started_at,
            closed_at: closed_after.map(|seconds| started_at + seconds * 1000),
        }
    };
    let executions = vec![
        execution(
            WorkKind::Run,
            "ticket/4821:reconcile-weekly-budget-and-receipts",
            WorkStatus::Running,
            3,
            None,
        ),
        execution(
            WorkKind::Occurrence,
            "automation/weekday-morning-review-for-the-family-and-household",
            WorkStatus::Completed,
            18,
            Some(42),
        ),
        execution(
            WorkKind::Session,
            "conversation/9332",
            WorkStatus::Failed,
            64,
            Some(14),
        ),
        execution(
            WorkKind::Run,
            "ticket/4790",
            WorkStatus::Cancelled,
            170,
            Some(65),
        ),
        execution(
            WorkKind::Occurrence,
            "automation/house-check",
            WorkStatus::Failed,
            1_460,
            Some(3_600),
        ),
    ];
    suite.window.update(&mut suite.cx, |shell, _, cx| {
        shell.fixture_temporal(
            ainc_client::types::WorkPage {
                work: executions.clone(),
                next_page: Some("next".into()),
            },
            cx,
        );
    })?;
    suite.keys("cmd-6");
    suite.capture("temporal-populated", Route::Temporal, None, false)?;
    suite.bounds("temporal.row.ticket/4821:reconcile-weekly-budget-and-receipts")?;
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
            ainc_client::types::WorkPage {
                work: Vec::new(),
                next_page: None,
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
            ainc_client::types::WorkPage {
                work: executions.clone(),
                next_page: Some("next".into()),
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
                    ainc_client::types::WorkPage {
                        work: vec![execution(
                            WorkKind::Run,
                            "ticket/4821:reconcile-weekly-budget-and-receipts",
                            WorkStatus::Running,
                            3,
                            None,
                        )],
                        next_page: None,
                    },
                    cx,
                );
            })?;
            suite.keys("cmd-6");
            suite.capture("temporal-small", Route::Temporal, None, false)?;
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
                    Some(Overlay::Dialog(Route::Tickets)),
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
                Some(Overlay::GoTo),
                false,
            )?;
            suite.bounds("palette.result.recent.page.tickets")?;
            suite.bounds("palette.result.pages.page.tickets")?;
            suite.keys("down");
            suite.keys("down");
            suite.capture(
                "search-keyboard",
                Route::Automations,
                Some(Overlay::GoTo),
                false,
            )?;
        }
        suite.cx.simulate_input(window.into(), "settings");
        suite.capture(
            &format!("search-{round}"),
            Route::Automations,
            Some(Overlay::GoTo),
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
                Some(Overlay::Dialog(Route::Automations)),
                false,
            )?;
            suite.keys("escape");
        }
        for (shortcut, route) in [(5, Route::Terminal), (6, Route::Temporal)] {
            suite.keys(&format!("cmd-{shortcut}"));
            suite.capture(&format!("route-{round}-{shortcut}"), route, None, false)?;
        }
    }
    suite.keys("cmd-1");
    suite.settle()?;
    suite.click_selector("tickets.create")?;
    suite.capture(
        "add-dialog",
        Route::Tickets,
        Some(Overlay::Dialog(Route::Tickets)),
        false,
    )?;
    suite.cx.simulate_input(window.into(), "Rendered café 👋");
    suite.capture(
        "add-typed",
        Route::Tickets,
        Some(Overlay::Dialog(Route::Tickets)),
        false,
    )?;
    suite.keys("escape");
    suite.capture("dialog-dismissed", Route::Tickets, None, false)?;
    suite.keys("cmd-k");
    suite.capture("search-open", Route::Tickets, Some(Overlay::GoTo), false)?;
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
        Some(Overlay::GoTo),
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
    suite.bounds("tickets.detail.status")?;
    suite.click_selector("tickets.back")?;
    suite.capture("tickets-list", Route::Tickets, None, false)?;
    suite.bounds("ticket.1")?;
    let previous = suite.window;
    let board = suite
        .cx
        .open_offscreen_window(size(px(1360.), px(828.)), |window, cx| {
            cx.new(|cx| Shell::fixture(temporary.path().join("tickets-session.json"), window, cx))
        })?;
    suite.window = board;
    tickets_suite(&mut suite, board)?;
    suite.window = previous;
    suite.keys("cmd-3");
    suite.capture("agents-list", Route::Agents, None, false)?;
    suite.window.update(&mut suite.cx, |shell, _, cx| {
        shell.fixture_conversation(false, cx)
    })?;
    suite.capture("assistant-new-conversation", Route::Assistant, None, false)?;
    suite.window.update(&mut suite.cx, |shell, _, cx| {
        shell.fixture_conversation(true, cx)
    })?;
    suite.capture("assistant-conversation", Route::Assistant, None, false)?;
    suite.click_selector("back-to-conversations")?;
    suite.capture("assistant-conversation-list", Route::Assistant, None, false)?;
    suite.window.update(&mut suite.cx, |shell, _, cx| {
        shell.fixture_navigate(Route::Connections, cx);
        shell.fixture_models(cx);
    })?;
    suite.capture("settings-model-closed", Route::Connections, None, false)?;
    let closed = suite.bounds("settings.row.Model")?;
    suite.click_selector("codex-model-select")?;
    suite.capture("model-dropdown-open", Route::Connections, None, false)?;
    suite.bounds("codex-model-select.menu")?;
    let open = suite.bounds("settings.row.Model")?;
    near(
        "open select never resizes its row",
        f32::from(open.size.height),
        f32::from(closed.size.height),
    )?;
    suite.keys("escape");
    suite.capture("model-dropdown-closed", Route::Connections, None, false)?;
    ensure!(
        suite.bounds("codex-model-select.menu").is_err(),
        "escape must close the model select"
    );
    // Choosing an option selects that model, closes the menu and keeps the
    // row's height.
    suite.click_selector("codex-model-select")?;
    suite.click_selector("codex-model-select.option.1")?;
    suite.capture("model-dropdown-selected", Route::Connections, None, false)?;
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
        "properties",
        "list-rows",
        "table",
        "empty-and-loading",
    ])?;
    suite.window.update(&mut suite.cx, |shell, _, cx| {
        shell.fixture_components(3, false, cx);
    })?;
    suite.capture("components-overlays", Route::Components, None, false)?;
    suite.check_components_geometry(&["dialog", "sheet", "menus-and-popovers", "toasts"])?;
    suite.keys("cmd-5");
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
