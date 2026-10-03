//! Shell interaction tests: navigation, the sidebar, Command hints, the palette.
use super::{Shell, bind_keys};
use crate::{
    input,
    overlay::Overlay,
    routes::Route,
    ui_state::{SIDEBAR_DEFAULT, SIDEBAR_MIN, UiState},
};
use gpui::{
    Focusable, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, Pixels, Point, TestAppContext,
    VisualTestContext, point, px,
};

fn double_click(cx: &mut VisualTestContext, position: Point<Pixels>) {
    for click_count in [1, 2] {
        cx.simulate_event(MouseDownEvent {
            position,
            button: MouseButton::Left,
            modifiers: Modifiers::default(),
            click_count,
            first_mouse: false,
        });
        cx.simulate_event(MouseUpEvent {
            position,
            button: MouseButton::Left,
            modifiers: Modifiers::default(),
            click_count,
        });
    }
}

fn assert_sidebar_hints(cx: &mut VisualTestContext, visible: bool) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert_eq!(cx.debug_bounds("sidebar-badge-1").is_some(), visible);
}

fn zoom_requests() -> usize {
    super::header::TITLEBAR_ZOOMS.with(|zooms| zooms.get())
}

#[gpui::test]
fn tab_shortcuts_history_mouse_close_and_restore(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tabs.json");
    cx.update(bind_keys);
    let (shell, cx) = cx.add_window_view(|window, cx| Shell::fixture(path.clone(), window, cx));
    cx.simulate_keystrokes("cmd-1 cmd-t cmd-3 cmd-shift-[");
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.ui_state.tabs().len(), 2);
        assert_eq!(shell.ui_state.current(), Route::Tickets);
    });
    cx.simulate_keystrokes("cmd-[");
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.ui_state.current(), Route::Assistant)
    });
    cx.simulate_keystrokes("cmd-shift-]");
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.ui_state.current(), Route::Agents)
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    // Closing an inactive tab does not activate it, nor bubble into its parent.
    let close = cx.debug_bounds("tabs.0.close").unwrap().center();
    cx.simulate_click(close, Modifiers::default());
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.ui_state.tabs().len(), 1);
        assert_eq!(shell.ui_state.current(), Route::Agents);
    });
    assert_eq!(UiState::load(&path).current(), Route::Agents);
    cx.simulate_keystrokes("cmd-shift-w");
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.ui_state.tabs().len(), 1);
        assert_eq!(shell.ui_state.current(), Route::Assistant);
    });
    cx.simulate_keystrokes("cmd-k cmd-shift-u");
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.overlays.borrow().active(), None)
    });
}

#[gpui::test]
fn overflowing_tabs_scroll_reveal_and_stay_between_controls(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    cx.update(bind_keys);
    let (shell, cx) =
        cx.add_window_view(|window, cx| Shell::fixture(dir.path().join("tabs.json"), window, cx));
    cx.simulate_resize(gpui::size(px(760.), px(600.)));
    for _ in 0..24 {
        cx.simulate_keystrokes("cmd-t");
    }
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let viewport = cx.debug_bounds("tabs.viewport").unwrap();
    let active = cx.debug_bounds("tabs.24").unwrap();
    let new = cx.debug_bounds("tabs.new").unwrap();
    assert!(active.left() >= viewport.left() && active.right() <= viewport.right());
    assert!(viewport.right() <= new.left());
    assert!(new.right() <= px(760.));
    let offset = shell.read_with(cx, |shell, _| shell.tab_scroll.offset().x);
    assert!(offset < px(0.));
    cx.simulate_event(gpui::ScrollWheelEvent {
        position: viewport.center(),
        delta: gpui::ScrollDelta::Lines(point(0., 3.)),
        ..Default::default()
    });
    shell.read_with(cx, |shell, _| assert!(shell.tab_scroll.offset().x > offset));
    cx.simulate_keystrokes("cmd-shift-]");
    cx.update(|window, cx| window.draw(cx).clear(cx));
    shell.read_with(cx, |shell, _| assert_eq!(shell.ui_state.active_tab(), 0));
    let first = cx.debug_bounds("tabs.0").unwrap();
    assert!(first.left() >= viewport.left() && first.right() <= viewport.right());
    cx.simulate_resize(gpui::size(px(960.), px(700.)));
    cx.simulate_keystrokes("cmd-shift-[");
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let viewport = cx.debug_bounds("tabs.viewport").unwrap();
    let last = cx.debug_bounds("tabs.24").unwrap();
    assert!(last.left() >= viewport.left() && last.right() <= viewport.right());
}

#[gpui::test]
fn restored_active_tab_is_visible_on_the_first_frame(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tabs.json");
    let mut state = UiState::default();
    for _ in 0..24 {
        state.new_tab();
    }
    state.save(&path).unwrap();
    let (_, cx) = cx.add_window_view(|window, cx| Shell::fixture(path, window, cx));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let viewport = cx.debug_bounds("tabs.viewport").unwrap();
    let tab = cx.debug_bounds("tabs.24").unwrap();
    assert!(tab.left() >= viewport.left() && tab.right() <= viewport.right());
}

#[gpui::test]
fn automation_agent_select_keeps_stable_ids_and_dismisses_before_dialog(cx: &mut TestAppContext) {
    use crate::{automations::AutomationsPage, page::Page};
    let dir = tempfile::tempdir().unwrap();
    cx.update(bind_keys);
    let (shell, cx) =
        cx.add_window_view(|window, cx| Shell::fixture(dir.path().join("tabs.json"), window, cx));
    shell.update(cx, |shell, cx| {
        shell
            .daemon
            .send(ainc_client::types::TicketCommand::RegisterAgent {
                name: "Test agent".into(),
                instructions: "Test".into(),
                model: "test".into(),
            })
            .unwrap();
        shell
            .page_entity::<AutomationsPage>()
            .update(cx, |page, _| page.reload());
        shell.fixture_navigate(Route::Automations, cx);
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let create = cx
        .debug_bounds("automations.create.empty")
        .unwrap()
        .center();
    cx.simulate_click(create, Modifiers::default());
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let trigger = cx.debug_bounds("automations.agent").unwrap().center();
    cx.simulate_click(trigger, Modifiers::default());
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(cx.debug_bounds("automations.agent.agent-1").is_some());
    cx.simulate_keystrokes("escape");
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(cx.debug_bounds("automations.agent.agent-1").is_none());
    assert!(cx.debug_bounds("automations.agent").is_some());
    shell.read_with(cx, |shell, _| {
        assert_eq!(
            shell.overlays.borrow().active(),
            Some(Overlay::Dialog(Route::Automations))
        )
    });
    cx.simulate_click(trigger, Modifiers::default());
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let choice = cx
        .debug_bounds("automations.agent.agent-1")
        .unwrap()
        .center();
    cx.simulate_click(choice, Modifiers::default());
    shell.read_with(cx, |shell, cx| {
        let page = shell.page_entity::<AutomationsPage>();
        assert_eq!(
            page.read(cx)
                .drafts(cx)
                .unwrap()
                .get::<String>("agent")
                .as_deref(),
            Some("agent-1")
        );
    });
}

#[gpui::test]
fn compact_sidebar_preserves_icon_positions_and_profile_access(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (shell, cx) =
        cx.add_window_view(|window, cx| Shell::fixture(dir.path().join("tabs.json"), window, cx));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let search = cx.debug_bounds("shell.search").unwrap();
    let nav = cx.debug_bounds("sidebar-nav-1").unwrap();
    let profile = cx.debug_bounds("sidebar-profile-avatar").unwrap();
    shell.update(cx, |shell, cx| {
        shell.ui_state.sidebar.open = false;
        shell.sidebar_visible = crate::ui_state::SIDEBAR_COLLAPSED;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert_eq!(cx.debug_bounds("shell.search").unwrap().top(), search.top());
    assert_eq!(cx.debug_bounds("sidebar-nav-1").unwrap().origin, nav.origin);
    assert_eq!(cx.debug_bounds("sidebar-profile-avatar").unwrap(), profile);
    assert!(cx.debug_bounds("sidebar-profile-name").is_none());
    let main = cx.debug_bounds("main-pane").unwrap();
    assert_eq!(main.left(), px(crate::ui_state::SIDEBAR_COLLAPSED));
    let ticket = cx.debug_bounds("sidebar-nav-1").unwrap().center();
    cx.simulate_click(ticket, Modifiers::default());
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.ui_state.current(), Route::Tickets)
    });
    cx.simulate_click(profile.center(), Modifiers::default());
    shell.read_with(cx, |shell, _| {
        assert_eq!(
            shell.overlays.borrow().active(),
            Some(Overlay::UserMenu { support: false })
        )
    });
}

#[gpui::test]
fn support_menu_stays_beside_profile_menu_at_all_font_sizes(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (shell, cx) =
        cx.add_window_view(|window, cx| Shell::fixture(dir.path().join("tabs.json"), window, cx));
    cx.simulate_resize(gpui::size(px(760.), px(600.)));
    for (font_size, _) in crate::ui_state::FontSize::ALL {
        cx.update(|window, cx| {
            shell.update(cx, |shell, cx| {
                let appearance = crate::ui_state::Appearance {
                    font_size,
                    ..Default::default()
                };
                shell.apply_appearance(appearance, cx);
                shell.overlays.borrow_mut().open(
                    Overlay::UserMenu { support: true },
                    window,
                    cx,
                    None,
                );
            });
            window.draw(cx).clear(cx);
        });
        let parent = cx.debug_bounds("user-menu").unwrap();
        let support = cx.debug_bounds("user-menu.support.menu").unwrap();
        assert!(support.left() >= parent.right() + px(crate::ui::SPACE_2));
        assert!(support.right() <= px(760.));
        assert!(support.bottom() <= px(600.));
        assert_eq!(support.bottom(), parent.bottom());
    }
    crate::ui::set_type_scale(1.);
}

#[gpui::test]
fn custom_header_owns_titlebar_gestures(_cx: &mut TestAppContext) {
    let bounds = gpui::Bounds::new(point(px(0.), px(0.)), gpui::size(px(1360.), px(828.)));
    assert!(crate::main_window_options(bounds, "QA".into(), true).app_owns_titlebar_drag);
}

#[gpui::test]
fn header_controls_do_not_zoom_but_empty_space_does(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (shell, cx) = cx
        .add_window_view(|window, cx| Shell::fixture(dir.path().join("session.json"), window, cx));
    shell.update(cx, |shell, cx| {
        shell.ui_state.navigate(Route::Tickets);
        shell.ui_state.navigate(Route::Agents);
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let back = cx.debug_bounds("back").unwrap().center();
    double_click(cx, back);
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.ui_state.current(), Route::Assistant);
        assert_eq!(zoom_requests(), 0);
    });
    let forward = cx.debug_bounds("forward").unwrap().center();
    double_click(cx, forward);
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.ui_state.current(), Route::Agents);
        assert_eq!(zoom_requests(), 0);
    });
    for id in ["sidebar", "shell.search"] {
        let position = cx.debug_bounds(id).unwrap().center();
        double_click(cx, position);
        assert_eq!(zoom_requests(), 0);
    }
    let empty = cx.debug_bounds("titlebar-center-space").unwrap().center();
    double_click(cx, empty);
    assert_eq!(zoom_requests(), 1);
}

#[gpui::test]
fn sidebar_drag_persists_and_toggle_restores_its_width(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("session.json");
    cx.update(bind_keys);
    let (shell, cx) = cx.add_window_view(|window, cx| Shell::fixture(path.clone(), window, cx));
    let start = point(px(SIDEBAR_DEFAULT), px(200.));
    let end = point(px(260.), px(200.));
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(end, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::default());
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.ui_state.sidebar.width, 260.)
    });
    assert_eq!(UiState::load(&path).sidebar.width, 260.);
    cx.simulate_keystrokes("cmd-b");
    shell.read_with(cx, |shell, _| assert!(!shell.ui_state.sidebar.open));
    let focus = shell.read_with(cx, |shell, _| shell.focus.clone());
    cx.update(|window, cx| window.focus(&focus, cx));
    cx.simulate_keystrokes("cmd-b");
    shell.read_with(cx, |shell, _| {
        assert!(shell.ui_state.sidebar.open);
        assert_eq!(shell.ui_state.sidebar.width, 260.);
    });
}

#[gpui::test]
fn sidebar_badges_align_and_text_stays_inside_at_minimum_and_default_widths(
    cx: &mut TestAppContext,
) {
    let dir = tempfile::tempdir().unwrap();
    let (shell, cx) = cx
        .add_window_view(|window, cx| Shell::fixture(dir.path().join("session.json"), window, cx));
    let right = |bounds: gpui::Bounds<gpui::Pixels>| f32::from(bounds.origin.x + bounds.size.width);
    for width in [SIDEBAR_MIN, SIDEBAR_DEFAULT] {
        cx.update(|window, cx| {
            shell.update(cx, |shell, cx| {
                shell.ui_state.sidebar.width = width;
                shell.sidebar_visible = width;
                shell.command_held = true;
                cx.notify();
            });
            window.draw(cx).clear(cx);
        });
        let sidebar = cx.debug_bounds("sidebar-content").unwrap();
        let search = cx.debug_bounds("shell.search").unwrap();
        assert!(
            f32::from(search.origin.x) >= f32::from(sidebar.origin.x),
            "search left edge at width {width}"
        );
        assert!(
            right(search) <= right(sidebar),
            "search right edge at width {width}"
        );
        let mut badge_right: Option<f32> = None;
        for index in 1..=5 {
            let badge = cx
                .debug_bounds(
                    [
                        "sidebar-badge-1",
                        "sidebar-badge-2",
                        "sidebar-badge-3",
                        "sidebar-badge-4",
                        "sidebar-badge-5",
                    ][index - 1],
                )
                .unwrap();
            let label = cx
                .debug_bounds(
                    [
                        "sidebar-label-1",
                        "sidebar-label-2",
                        "sidebar-label-3",
                        "sidebar-label-4",
                        "sidebar-label-5",
                    ][index - 1],
                )
                .unwrap();
            assert!(
                right(label) + 8. <= f32::from(badge.origin.x),
                "label {index} at width {width}"
            );
            assert!(
                right(badge) <= right(sidebar) - 10.,
                "badge {index} at width {width}"
            );
            if let Some(expected) = badge_right {
                assert!(
                    (right(badge) - expected).abs() <= 1.,
                    "badge {index} at width {width}"
                );
            } else {
                badge_right = Some(right(badge));
            }
        }
    }
}

#[gpui::test]
fn sidebar_hints_follow_command_across_navigation_and_input_focus(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    cx.update(|cx| {
        bind_keys(cx);
        input::bind_keys(cx);
    });
    let (shell, cx) = cx
        .add_window_view(|window, cx| Shell::fixture(dir.path().join("session.json"), window, cx));
    let command = Modifiers {
        platform: true,
        ..Modifiers::default()
    };
    for number in [2, 5, 1] {
        cx.simulate_modifiers_change(command);
        cx.simulate_keystrokes(&format!("cmd-{number}"));
        shell.read_with(cx, |shell, _| {
            assert_eq!(
                shell.ui_state.current(),
                Route::from_shortcut(number).unwrap()
            );
            assert!(
                shell.command_held,
                "navigation must not hide a held Command"
            );
        });
        assert_sidebar_hints(cx, true);
        cx.simulate_modifiers_change(Modifiers::default());
        shell.read_with(cx, |shell, _| assert!(!shell.command_held));
        assert_sidebar_hints(cx, false);
    }

    cx.simulate_modifiers_change(command);
    cx.simulate_keystrokes("cmd-k");
    let focus = shell.read_with(cx, |shell, cx| shell.input.focus_handle(cx));
    cx.update(|window, _| assert!(focus.is_focused(window)));
    cx.simulate_modifiers_change(Modifiers::default());
    shell.read_with(cx, |shell, _| assert!(!shell.command_held));
    cx.simulate_input("settings");
    shell.read_with(cx, |shell, cx| {
        assert_eq!(shell.input.read(cx).content, "settings")
    });
    cx.simulate_keystrokes("enter");
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.ui_state.current(), Route::Settings)
    });
}

#[gpui::test]
fn sidebar_hints_recover_from_a_missed_release_on_navigation_click(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (shell, cx) = cx
        .add_window_view(|window, cx| Shell::fixture(dir.path().join("session.json"), window, cx));
    cx.simulate_modifiers_change(Modifiers {
        platform: true,
        ..Modifiers::default()
    });
    assert_sidebar_hints(cx, true);
    // No release event: AppKit may have sent it to a different responder.
    let agents = cx.debug_bounds("sidebar-nav-3").unwrap().center();
    cx.simulate_click(agents, Modifiers::default());
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.ui_state.current(), Route::Agents);
        assert!(!shell.command_held);
    });
    assert_sidebar_hints(cx, false);
}

#[gpui::test]
fn sidebar_hints_clear_when_window_deactivates_without_command_release(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (shell, cx) = cx
        .add_window_view(|window, cx| Shell::fixture(dir.path().join("session.json"), window, cx));
    // Test windows start inactive; deactivate_window otherwise does nothing.
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    cx.update(|window, _| assert!(window.is_window_active()));
    cx.simulate_modifiers_change(Modifiers {
        platform: true,
        ..Modifiers::default()
    });
    assert_sidebar_hints(cx, true);
    cx.deactivate_window();
    cx.update(|window, _| assert!(!window.is_window_active()));
    shell.read_with(cx, |shell, _| assert!(!shell.command_held));
    assert_sidebar_hints(cx, false);
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    shell.read_with(cx, |shell, _| assert!(!shell.command_held));
}

#[gpui::test]
fn sidebar_hints_do_not_restore_a_stale_command_cache_on_reactivation(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let command = Modifiers {
        platform: true,
        ..Modifiers::default()
    };
    let (shell, cx) = cx.add_window_view(|window, cx| {
        // Register before Shell's observer to model a platform reporting
        // stale Command state at activation, without a fresh input event.
        cx.observe_window_activation(window, move |_: &mut Shell, window, _| {
            if window.is_window_active() {
                window.set_modifiers(command);
            }
        })
        .detach();
        Shell::fixture(dir.path().join("session.json"), window, cx)
    });
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    cx.simulate_modifiers_change(command);
    assert_sidebar_hints(cx, true);
    cx.deactivate_window();
    assert_sidebar_hints(cx, false);
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    cx.update(|window, _| {
        assert!(window.is_window_active());
        assert!(
            window.modifiers().platform,
            "regression requires a stale cache"
        );
    });
    shell.read_with(cx, |shell, _| assert!(!shell.command_held));
    assert_sidebar_hints(cx, false);

    // Fresh modifier observations still show hints and clear them normally.
    cx.simulate_modifiers_change(command);
    assert_sidebar_hints(cx, true);
    cx.simulate_modifiers_change(Modifiers::default());
    assert_sidebar_hints(cx, false);
}

#[gpui::test]
fn sidebar_hints_preserve_fresh_command_input_before_queued_activation(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (shell, cx) = cx
        .add_window_view(|window, cx| Shell::fixture(dir.path().join("session.json"), window, cx));
    let command = Modifiers {
        platform: true,
        ..Modifiers::default()
    };
    for native_input in [false, true] {
        cx.update(|window, _| {
            assert!(!window.is_window_active());
            window.activate_window();
        });
        if native_input {
            // Ghostty's C callback updates Shell synchronously, without
            // delivering a GPUI event or draining the foreground executor.
            shell.update(cx, |shell, cx| shell.set_command_held(true, cx));
        } else {
            // simulate_modifiers_change drains the executor; dispatch
            // directly so the activation observer is still pending.
            cx.update(|window, cx| {
                window.dispatch_event(
                    gpui::PlatformInput::ModifiersChanged(gpui::ModifiersChangedEvent {
                        modifiers: command,
                        capslock: gpui::Capslock { on: false },
                    }),
                    cx,
                );
            });
        }
        cx.update(|window, _| assert!(!window.is_window_active()));
        shell.read_with(cx, |shell, _| assert!(shell.command_held));
        cx.run_until_parked();
        cx.update(|window, _| assert!(window.is_window_active()));
        shell.read_with(cx, |shell, _| assert!(shell.command_held));
        assert_sidebar_hints(cx, true);

        // A subsequently delivered deactivation still clears the state;
        // the next activation alone must not resurrect it.
        cx.deactivate_window();
        assert_sidebar_hints(cx, false);
    }
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    assert_sidebar_hints(cx, false);
}

#[gpui::test]
fn routes_and_history_use_shell_actions(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    cx.update(bind_keys);
    let (shell, cx) = cx
        .add_window_view(|window, cx| Shell::fixture(dir.path().join("session.json"), window, cx));
    for number in 1..=5 {
        cx.simulate_keystrokes(&format!("cmd-{number}"));
        shell.read_with(cx, |shell, _| {
            assert_eq!(
                shell.fixture_state().0,
                Route::from_shortcut(number).unwrap()
            )
        });
    }
    cx.simulate_keystrokes("cmd-[");
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.ui_state.current(), Route::Automations)
    });
    cx.simulate_keystrokes("cmd-]");
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.ui_state.current(), Route::Terminal)
    });
    cx.simulate_keystrokes("cmd-,");
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.ui_state.current(), Route::Settings)
    });
    cx.simulate_keystrokes("cmd-1");
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(cx.debug_bounds("sidebar-settings").is_none());
    let profile = cx.debug_bounds("sidebar-profile").unwrap().center();
    cx.simulate_click(profile, Modifiers::default());
    shell.read_with(cx, |shell, _| {
        assert_eq!(
            shell.overlays.borrow().active(),
            Some(Overlay::UserMenu { support: false })
        );
    });
    let settings = cx.debug_bounds("user-menu.settings").unwrap().center();
    cx.simulate_click(settings, Modifiers::default());
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.ui_state.current(), Route::Settings);
        assert_eq!(shell.overlays.borrow().active(), None);
    });
}

#[gpui::test]
fn search_filters_selects_and_restores_focus(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    cx.update(|cx| {
        bind_keys(cx);
        input::bind_keys(cx);
    });
    let (shell, cx) = cx
        .add_window_view(|window, cx| Shell::fixture(dir.path().join("session.json"), window, cx));
    cx.simulate_keystrokes("cmd-k");
    cx.simulate_input("settings");
    shell.read_with(cx, |shell, cx| {
        assert_eq!(shell.overlays.borrow().active(), Some(Overlay::GoTo));
        let results = shell.palette_results(&shell.input.read(cx).content);
        assert_eq!(results.len(), 1);
        assert_eq!(results.choices[0].0.as_ref(), "page.settings");
    });
    cx.simulate_keystrokes("enter");
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.ui_state.current(), Route::Settings);
        assert_eq!(shell.overlays.borrow().active(), None);
    });
    cx.simulate_keystrokes("cmd-k");
    cx.simulate_input("no-such-space");
    cx.simulate_keystrokes("enter");
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.ui_state.current(), Route::Settings)
    });
    cx.simulate_keystrokes("escape");
    let focus = shell.read_with(cx, |shell, _| shell.focus.clone());
    cx.update(|window, _| assert!(focus.is_focused(window)));
}

#[gpui::test]
fn shortcuts_survive_clicking_a_control_that_disappears(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    cx.update(bind_keys);
    let (shell, cx) = cx
        .add_window_view(|window, cx| Shell::fixture(dir.path().join("session.json"), window, cx));
    shell.update(cx, |shell, cx| {
        shell.toasts.push("Saved", None, crate::ui::Tone::Info);
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let close = cx.debug_bounds("toast.close.1").unwrap().center();
    cx.simulate_click(close, Modifiers::default());
    shell.read_with(cx, |shell, _| assert!(shell.toasts.is_empty()));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.simulate_keystrokes("cmd-3");
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.ui_state.current(), Route::Agents)
    });
}

#[gpui::test]
fn palette_groups_rank_prefixes_and_remember_recents(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (shell, cx) = cx
        .add_window_view(|window, cx| Shell::fixture(dir.path().join("session.json"), window, cx));
    shell.update(cx, |shell, _| {
        let all = shell.palette_results("");
        assert_eq!(all.groups[0].title.as_ref(), "Pages");
        assert!(all.groups.iter().any(|g| g.title.as_ref() == "Actions"));
        let ranked = shell.palette_results("te");
        let pages = &ranked.groups[0];
        assert_eq!(pages.title.as_ref(), "Pages");
        // Prefix matches (Terminal, Temporal) outrank the later subsequence in Tickets.
        assert!(
            pages.entries[0].label.starts_with("Te"),
            "{}",
            pages.entries[0].label
        );
        assert!(
            pages.entries[1].label.starts_with("Te"),
            "{}",
            pages.entries[1].label
        );
        assert!(pages.entries.iter().any(|e| e.label.as_ref() == "Tickets"));
        assert!(!pages.entries[0].positions.is_empty());
        shell.ui_state.remember_command("page.agents");
        let recent = shell.palette_results("");
        assert_eq!(recent.groups[0].title.as_ref(), "Recent");
        assert_eq!(recent.choices[0].0.as_ref(), "page.agents");
        assert_eq!(shell.palette_results("zzzz").len(), 0);
    });
}

#[gpui::test]
fn search_shortcut_works_with_pane_or_no_focus(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    cx.update(|cx| {
        bind_keys(cx);
        input::bind_keys(cx);
    });
    let (shell, cx) = cx
        .add_window_view(|window, cx| Shell::fixture(dir.path().join("session.json"), window, cx));
    let pane_focus = shell.read_with(cx, |shell, _| shell.sidebar_focus.clone());
    cx.update(|window, cx| window.focus(&pane_focus, cx));
    cx.simulate_keystrokes("cmd-k");
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.overlays.borrow().active(), Some(Overlay::GoTo));
    });
    cx.simulate_keystrokes("escape");
    cx.update(|window, cx| window.blur(cx));
    cx.simulate_keystrokes("cmd-k");
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.overlays.borrow().active(), Some(Overlay::GoTo));
    });
}

#[gpui::test]
fn search_focus_wraps_and_escape_dismisses(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    cx.update(|cx| {
        bind_keys(cx);
        input::bind_keys(cx);
    });
    let (shell, cx) = cx
        .add_window_view(|window, cx| Shell::fixture(dir.path().join("session.json"), window, cx));
    cx.simulate_keystrokes("cmd-k shift-tab");
    let last = shell.read_with(cx, |shell, cx| {
        let count = shell.palette_results(&shell.input.read(cx).content).len();
        shell.picker_result_focus[count - 1].clone()
    });
    cx.update(|window, _| assert!(last.is_focused(window)));
    cx.simulate_keystrokes("tab");
    let input = shell.read_with(cx, |shell, cx| shell.input.focus_handle(cx));
    cx.update(|window, _| assert!(input.is_focused(window)));
    cx.simulate_keystrokes("escape");
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.overlays.borrow().active(), None)
    });
}
