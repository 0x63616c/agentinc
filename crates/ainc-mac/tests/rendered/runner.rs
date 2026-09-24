use crate::{
    input,
    model::{FontSize, PANE_WIDTHS, Route, Session},
    overlay::Overlay,
    shell::{self, Shell},
    style::Assets,
};
use anyhow::{Result, ensure};
use gpui::{AppContext, Modifiers, VisualTestAppContext, WindowHandle, point, px, size};
use std::{path::PathBuf, sync::Arc, time::Duration};

// Check actual pixels in independent shell regions, rather than trusting scene/AX nodes.
// Coordinates are logical pixels; thresholds are deliberately below normal text contrast.
fn regions(
    width: u32,
    height: u32,
    evee: bool,
    dimmed: bool,
) -> Vec<(&'static str, [u32; 4], u8, usize)> {
    let text = if dimmed { 35 } else { 90 };
    let border = if dimmed { 7 } else { 20 };
    let mut regions = vec![
        ("header", [150, 10, width - 10, 40], text, 80),
        ("workspace", [15, 65, 165, 100], text, 60),
        ("profile", [15, height - 44, 170, height - 12], text, 30),
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
    for (index, name) in [
        "Today",
        "Tickets",
        "Agents",
        "Automations",
        "Home",
        "Calendar",
        "Library",
        "My apps",
        "Assistant",
    ]
    .into_iter()
    .enumerate()
    {
        let y = 115 + index as u32 * 34;
        regions.push((name, [20, y, 165, y + 28], text, 35));
    }
    if evee {
        regions.extend([
            ("Evee header", [width - 250, 56, width - 20, 90], text, 50),
            (
                "Evee composer",
                [width - 244, height - 121, width - 30, height - 82],
                if dimmed { 20 } else { 60 },
                30,
            ),
            (
                "Evee border",
                [width - 266, 110, width - 264, height - 30],
                border,
                100,
            ),
        ]);
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
            evee,
            overlay.is_some_and(|o| o == Overlay::Search || o.is_dialog()),
        );
        check_pixels(image.as_raw(), image.width(), scale, &probes)
            .map_err(|e| anyhow::anyhow!("{name}: {e}"))?;
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
    suite.capture("initial", Route::Today, None, true)?;
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
            Route::Today,
            Route::Tickets,
            Route::Agents,
            Route::Home,
            Route::Calendar,
            Route::Library,
            Route::Apps,
            Route::Assistant,
        ]
        .into_iter()
        .enumerate()
        {
            suite.keys(&format!("cmd-{}", index + 1));
            suite.capture(&format!("route-{round}-{index}"), route, None, true)?;
        }
        suite.keys("cmd-k");
        suite.cx.simulate_input(window.into(), "settings");
        suite.capture(
            &format!("search-{round}"),
            Route::Assistant,
            Some(Overlay::Search),
            true,
        )?;
        suite.keys("enter");
        suite.capture(&format!("settings-{round}"), Route::Settings, None, true)?;
        suite.keys("cmd-9");
        suite.capture(
            &format!("automations-{round}"),
            Route::Automations,
            None,
            true,
        )?;
    }
    suite.keys("cmd-2");
    suite.settle()?;
    // Add Ticket is in the shared page header at the smaller window width.
    suite.click(811., 93.);
    suite.capture("add-dialog", Route::Tickets, Some(Overlay::AddTicket), true)?;
    suite.cx.simulate_input(window.into(), "Rendered café 👋");
    suite.capture("add-typed", Route::Tickets, Some(Overlay::AddTicket), true)?;
    suite.keys("escape");
    suite.capture("dialog-dismissed", Route::Tickets, None, true)?;
    suite.keys("cmd-k");
    suite.capture("search-open", Route::Tickets, Some(Overlay::Search), true)?;
    suite.keys("escape cmd-shift-e");
    suite.capture("evee-hidden", Route::Tickets, None, false)?;
    suite.keys("cmd-1");
    suite.capture("today-evee-hidden", Route::Today, None, false)?;
    suite.keys("cmd-shift-e");
    suite.capture("evee-restored", Route::Today, None, true)?;
    println!(
        "{} real Metal frames passed, including region-removal negative controls",
        suite.count
    );
    Ok(())
}
