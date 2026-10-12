//! Test fixtures: a shell over an in-memory daemon, and hooks that put pages
//! into the states the rendered and interaction tests capture.
use super::*;

impl Shell {
    pub fn fixture(path: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let daemon = crate::daemon::Daemon::in_memory();
        let mut shell = Self::with_state(
            path,
            Arc::new(daemon),
            crate::sync::Timers(cx.background_executor().clone()),
            crate::profile::Profile {
                name: "QA Profile".into(),
                photo: None,
            },
            window,
            cx,
        );
        shell.launch_started = Some(Instant::now() - std::time::Duration::from_secs(1));
        shell
    }
    pub fn fixture_state(&self) -> (Route, Option<Overlay>, bool) {
        (
            self.ui_state.current().page(),
            self.overlays.borrow().active(),
            false,
        )
    }
    pub fn fixture_navigate(&mut self, route: Route, cx: &mut Context<Self>) {
        self.ui_state.navigate(route);
        cx.notify();
    }
    pub fn fixture_launch_elapsed(&mut self, elapsed: std::time::Duration, cx: &mut Context<Self>) {
        self.launch_started = Some(Instant::now() - elapsed);
        cx.notify();
    }
    pub fn fixture_settings(&self) -> Entity<crate::settings::SettingsPage> {
        self.page_entity::<crate::settings::SettingsPage>()
    }
    pub fn fixture_profile_name(&mut self, name: &str, cx: &mut Context<Self>) {
        self.profile.name = name.into();
        cx.notify();
    }
    pub fn fixture_toast(&mut self, cx: &mut Context<Self>) {
        self.toasts.push(
            "Todo added",
            Some("Reconcile weekly budget and receipts".into()),
            Tone::Success,
        );
        self.toasts.push(
            "Todo update failed",
            Some("The daemon did not acknowledge the change.".into()),
            Tone::Danger,
        );
        cx.notify();
    }
    pub fn fixture_components(
        &mut self,
        section: usize,
        select_open: bool,
        cx: &mut Context<Self>,
    ) {
        self.ui_state.navigate(Route::Components);
        self.page_entity::<crate::components::ComponentsPage>()
            .update(cx, |page, cx| {
                page.fixture_section(section, cx);
                page.fixture_select_open(select_open, cx);
            });
        cx.notify();
    }
    /// A few Todos and Events, so the list pages have something to show.
    pub fn fixture_populated(&mut self, cx: &mut Context<Self>) {
        use ainc_client::types::{CalendarCommand, TodoCommand};
        let daemon = self.daemon.clone();
        for title in [
            "Reconcile weekly budget and receipts",
            "Book the dentist",
            "Water the plants",
            "Call Mum",
        ] {
            daemon
                .send(TodoCommand::Add {
                    title: title.into(),
                })
                .expect("fixture todo");
        }
        let done = daemon
            .send(TodoCommand::Add {
                title: "Take out the recycling".into(),
            })
            .expect("fixture todo");
        daemon
            .send(TodoCommand::Complete {
                id: done,
                completed: true,
            })
            .expect("fixture todo");
        let hour = 3600;
        let start = (crate::ui::time::now() / hour + 2) * hour;
        for (offset, length, title) in [
            (0, 1, "Standup"),
            (3, 2, "Dinner with Sam"),
            (26, 1, "Dentist"),
            (50, 3, "Hike with Alex"),
        ] {
            daemon
                .send(CalendarCommand::Create {
                    title: title.into(),
                    starts_at: start + offset * hour,
                    ends_at: start + (offset + length) * hour,
                })
                .expect("fixture event");
        }
        self.page_entity::<crate::todos::TodosPage>()
            .update(cx, |page, cx| {
                page.reload();
                cx.notify();
            });
        self.page_entity::<crate::calendar::CalendarPage>()
            .update(cx, |page, cx| {
                page.reload();
                cx.notify();
            });
        self.page_entity::<crate::overview::OverviewPage>()
            .update(cx, |page, cx| {
                page.reload();
                cx.notify();
            });
        cx.notify();
    }
    pub fn fixture_todos(&self) -> Entity<crate::todos::TodosPage> {
        self.page_entity()
    }
    pub fn fixture_calendar(&self) -> Entity<crate::calendar::CalendarPage> {
        self.page_entity()
    }
    pub fn fixture_recent_commands(&mut self, ids: &[&str], cx: &mut Context<Self>) {
        for id in ids.iter().rev() {
            self.ui_state.remember_command(id);
        }
        cx.notify();
    }
}
