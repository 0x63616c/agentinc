mod assistant;
mod evee;
mod input;
mod model;
mod overlay;
mod profile;
mod shell;
mod storage;
mod style;
mod tasks;
use gpui::*;
use shell::*;
struct DiagnosticLog;
impl log::Log for DiagnosticLog {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }
    fn log(&self, r: &log::Record) {
        eprintln!("{}: {}", r.level(), r.args());
    }
    fn flush(&self) {}
}
fn main() {
    let _ = log::set_logger(&DiagnosticLog);
    log::set_max_level(log::LevelFilter::Warn);
    Application::new().with_assets(style::Assets).run(|cx| {
        input::bind_keys(cx);
        shell::bind_keys(cx);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.set_menus(vec![
            Menu {
                name: "Agentinc OS".into(),
                items: vec![
                    MenuItem::os_submenu("Services", SystemMenuType::Services),
                    MenuItem::separator(),
                    MenuItem::action("Quit Agentinc OS", Quit),
                ],
            },
            Menu {
                name: "File".into(),
                items: vec![MenuItem::action("Search…", Search)],
            },
            Menu {
                name: "View".into(),
                items: vec![
                    MenuItem::action("Toggle Sidebar", ToggleSidebar),
                    MenuItem::action("Toggle Evee", ToggleEvee),
                    MenuItem::action("Back", GoBack),
                    MenuItem::action("Forward", GoForward),
                ],
            },
        ]);
        let bounds = Bounds::centered(None, size(px(1360.), px(828.)), cx);
        let result = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some(
                        std::env::var("AGENTINC_WINDOW_TITLE")
                            .unwrap_or_else(|_| "Agentinc OS".into())
                            .into(),
                    ),
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(18.), px(18.))),
                }),
                window_min_size: Some(size(px(800.), px(600.))),
                app_id: Some("com.agentinc.os".into()),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| Shell::new(window, cx)),
        );
        if let Err(error) = result {
            eprintln!("Could not open Agentinc OS: {error}");
            cx.quit();
            return;
        }
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.activate(true);
    });
}
