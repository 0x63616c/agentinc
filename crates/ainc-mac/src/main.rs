//! The AgentInc app entry point: menus, the main window and the opt-in pilot host.
mod about;
mod action;
mod agents;
mod assistant;
mod automations;
mod components;
mod connections;
mod daemon;
mod evee;
mod input;
mod native_update;
mod overlay;
mod page;
mod profile;
mod routes;
mod settings;
mod shell;
mod sync;
mod temporal;
mod terminal;
mod tickets;
mod ui;
mod ui_state;
mod updates;
use gpui::*;
use shell::*;
/// Logs go to stderr and, on macOS, the unified log under the bundle id. `AINC_LOG` filters
/// (default `info`); gpui's `log` records are bridged in.
fn init_tracing() {
    use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};
    let _ = tracing_log::LogTracer::init();
    let filter = EnvFilter::try_from_env("AINC_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr));
    #[cfg(target_os = "macos")]
    let subscriber = subscriber.with(tracing_oslog::OsLogger::new(
        ainc_release::identity::BUNDLE_ID,
        "app",
    ));
    subscriber.init();
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
        app_id: Some(ainc_release::identity::BUNDLE_ID.into()),
        focus: visible,
        show: visible,
        ..Default::default()
    }
}

#[allow(clippy::disallowed_macros)] // usage text
fn main() {
    ainc_release::process::reset_inherited_signals().expect("reset inherited process signals");
    // TLS: reqwest links rustls without a provider; ring is the one Temporal already uses.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    #[cfg(all(feature = "automation", target_os = "macos"))]
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
                "AINC_SESSION_PATH",
                "AINC_DISCOVERY_FILE",
                "AINC_LEGACY_DIR",
                "AINC_CODEX_HOME",
                "AINC_WINDOW_TITLE",
            ] {
                let value = std::env::var_os(name)
                    .filter(|v| !v.is_empty())
                    .unwrap_or_else(|| {
                        eprintln!("Pilot launch requires isolation variable {name}");
                        std::process::exit(2);
                    });
                if name != "AINC_WINDOW_TITLE" && !std::path::Path::new(&value).is_absolute() {
                    eprintln!("Pilot isolation paths must be absolute: {name}");
                    std::process::exit(2);
                }
            }
            (Some(std::path::PathBuf::from(&args[1])), args.len() == 3)
        } else {
            eprintln!(
                "Usage: AgentInc [--gpui-pilot-session ABSOLUTE_NEW_DIRECTORY [--gpui-pilot-visible]]"
            );
            std::process::exit(2);
        }
    };
    #[cfg(not(feature = "automation"))]
    if !args.is_empty() {
        eprintln!("This build does not accept automation flags");
        std::process::exit(2);
    }

    init_tracing();
    gpui_platform::application()
        .with_assets(ui::Assets)
        .run(move |cx| {
            cx.on_action(|_: &about::About, _| about::show());
            updates::init(cx);
            input::bind_keys(cx);
            shell::bind_keys(cx);
            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.set_menus(vec![
                Menu {
                    disabled: false,
                    name: "AgentInc".into(),
                    items: vec![
                        MenuItem::action("About AgentInc", about::About),
                        MenuItem::separator(),
                        MenuItem::action("Check for Updates…", updates::CheckForUpdates),
                        MenuItem::action("Release Notes", updates::ShowChangelog),
                        MenuItem::separator(),
                        MenuItem::os_submenu("Services", SystemMenuType::Services),
                        MenuItem::separator(),
                        MenuItem::action("Quit AgentInc", Quit),
                    ],
                },
                Menu {
                    disabled: false,
                    name: "File".into(),
                    items: vec![MenuItem::action("Go to…", GoTo)],
                },
                Menu {
                    disabled: false,
                    name: "View".into(),
                    items: vec![
                        MenuItem::action("Toggle Sidebar", ToggleSidebar),
                        MenuItem::action("Back", GoBack),
                        MenuItem::action("Forward", GoForward),
                    ],
                },
            ]);
            #[cfg(feature = "automation")]
            // Pilot captures the same pages at both supported review widths.
            let pilot_narrow =
                pilot_directory.is_some() && std::env::var_os("AINC_PILOT_NARROW").is_some();
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
                    std::env::var("AINC_WINDOW_TITLE")
                        .unwrap_or_else(|_| "AgentInc".into())
                        .into(),
                    visible,
                ),
                |window, cx| cx.new(|cx| Shell::new(window, cx)),
            );
            let window = match result {
                Ok(window) => window,
                Err(error) => {
                    tracing::error!(%error, "could not open the main window");
                    cx.quit();
                    return;
                }
            };
            #[cfg(ainc_upgrade_test)]
            updates::start_upgrade_test(cx);
            #[cfg(feature = "automation")]
            if let Some(directory) = pilot_directory {
                let title = std::env::var("AINC_WINDOW_TITLE").expect("validated pilot title");
                match gpui_pilot::host::Host::start(&directory, title, window.into(), cx) {
                    Ok(host) => cx.set_global(host),
                    Err(error) => {
                        tracing::error!(error = format!("{error:#}"), "could not start pilot");
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
