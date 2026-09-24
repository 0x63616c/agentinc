mod assistant;
mod automations;
mod components;
mod evee;
mod input;
mod model;
mod native_update;
mod overlay;
mod palette;
mod profile;
mod shell;
mod storage;
mod style;
mod tickets;
mod updates;
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

fn main_window_options(
    bounds: Bounds<Pixels>,
    title: SharedString,
    visible: bool,
) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some(title),
            appears_transparent: true,
            traffic_light_position: Some(point(px(18.), px(18.))),
        }),
        app_owns_titlebar_drag: true,
        window_min_size: Some(size(px(800.), px(600.))),
        app_id: Some("co.worldwidewebb.agentinc".into()),
        focus: visible,
        show: visible,
        ..Default::default()
    }
}

fn main() {
    ainc_release::process::reset_inherited_signals().expect("reset inherited process signals");
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    #[cfg(feature = "automation")]
    if args.len() == 3 && args[0] == "--update-ui-smoke" {
        let manifest: ainc_release::Manifest =
            serde_json::from_slice(&std::fs::read(&args[1]).expect("smoke manifest"))
                .expect("valid smoke manifest");
        native_update::smoke(&manifest, std::path::Path::new(&args[2]))
            .expect("native update smoke");
        return;
    }
    #[cfg(feature = "automation")]
    let (pilot_directory, pilot_visible) = {
        if args.is_empty() {
            (None, false)
        } else if (args.len() == 2 || (args.len() == 3 && args[2] == "--gpui-pilot-visible"))
            && args[0] == "--gpui-pilot-session"
        {
            for name in [
                "AGENTINC_SESSION_PATH",
                "AINC_DISCOVERY_FILE",
                "AINC_LEGACY_DIR",
                "AGENTINC_CODEX_HOME",
                "AGENTINC_WINDOW_TITLE",
            ] {
                let value = std::env::var_os(name)
                    .filter(|v| !v.is_empty())
                    .unwrap_or_else(|| {
                        eprintln!("Pilot launch requires isolation variable {name}");
                        std::process::exit(2);
                    });
                if name != "AGENTINC_WINDOW_TITLE" && !std::path::Path::new(&value).is_absolute() {
                    eprintln!("Pilot isolation paths must be absolute: {name}");
                    std::process::exit(2);
                }
            }
            (Some(std::path::PathBuf::from(&args[1])), args.len() == 3)
        } else {
            eprintln!(
                "Usage: agentinc-os [--gpui-pilot-session ABSOLUTE_NEW_DIRECTORY [--gpui-pilot-visible]]"
            );
            std::process::exit(2);
        }
    };
    #[cfg(not(feature = "automation"))]
    if !args.is_empty() {
        eprintln!("This build does not accept automation flags");
        std::process::exit(2);
    }

    let _ = log::set_logger(&DiagnosticLog);
    log::set_max_level(log::LevelFilter::Warn);
    gpui_platform::application()
        .with_assets(style::Assets)
        .run(move |cx| {
            updates::init(cx);
            input::bind_keys(cx);
            shell::bind_keys(cx);
            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.set_menus(vec![
                Menu {
                    disabled: false,
                    name: "AgentInc".into(),
                    items: vec![
                        MenuItem::action("Check for Updates…", updates::CheckForUpdates),
                        MenuItem::action("Changelog", updates::ShowChangelog),
                        MenuItem::separator(),
                        MenuItem::os_submenu("Services", SystemMenuType::Services),
                        MenuItem::separator(),
                        MenuItem::action("Quit AgentInc", Quit),
                    ],
                },
                Menu {
                    disabled: false,
                    name: "File".into(),
                    items: vec![MenuItem::action("Search…", Search)],
                },
                Menu {
                    disabled: false,
                    name: "View".into(),
                    items: vec![
                        MenuItem::action("Toggle Sidebar", ToggleSidebar),
                        MenuItem::action("Toggle Evee", ToggleEvee),
                        MenuItem::action("Back", GoBack),
                        MenuItem::action("Forward", GoForward),
                    ],
                },
            ]);
            #[cfg(feature = "automation")]
            // Pilot captures the same pages at both supported review widths.
            let pilot_narrow =
                pilot_directory.is_some() && std::env::var_os("AGENTINC_PILOT_NARROW").is_some();
            #[cfg(not(feature = "automation"))]
            let pilot_narrow = false;
            let bounds = Bounds::centered(
                None,
                if pilot_narrow {
                    size(px(1160.), px(728.))
                } else {
                    size(px(1360.), px(828.))
                },
                cx,
            );
            #[cfg(feature = "automation")]
            let visible = pilot_directory.is_none() || pilot_visible;
            #[cfg(not(feature = "automation"))]
            let visible = true;
            let result = cx.open_window(
                main_window_options(
                    bounds,
                    std::env::var("AGENTINC_WINDOW_TITLE")
                        .unwrap_or_else(|_| "AgentInc".into())
                        .into(),
                    visible,
                ),
                |window, cx| cx.new(|cx| Shell::new(window, cx)),
            );
            let window = match result {
                Ok(window) => window,
                Err(error) => {
                    eprintln!("Could not open AgentInc: {error}");
                    cx.quit();
                    return;
                }
            };
            cx.set_global(updates::UpdateHost(window));
            #[cfg(feature = "automation")]
            if let Some(directory) = pilot_directory {
                let title = std::env::var("AGENTINC_WINDOW_TITLE").expect("validated pilot title");
                match gpui_pilot::host::Host::start(&directory, title, window.into(), cx) {
                    Ok(host) => cx.set_global(host),
                    Err(error) => {
                        eprintln!("Could not start pilot: {error:#}");
                        cx.quit();
                        return;
                    }
                }
            }
            #[cfg(not(feature = "automation"))]
            let _ = window;
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            if visible {
                cx.activate(true);
            }
        });
}
