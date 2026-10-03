//! Test fixtures: a shell over an in-memory daemon, and hooks that put pages
//! into the states the rendered and interaction tests capture.
#![allow(dead_code)]
use super::*;

impl Shell {
    pub(crate) fn fixture(path: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
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
    pub(crate) fn fixture_state(&self) -> (Route, Option<Overlay>, bool) {
        (
            self.ui_state.current(),
            self.overlays.borrow().active(),
            false,
        )
    }
    pub(crate) fn fixture_navigate(&mut self, route: Route, cx: &mut Context<Self>) {
        self.ui_state.navigate(route);
        cx.notify();
    }
    pub(crate) fn fixture_launch_elapsed(
        &mut self,
        elapsed: std::time::Duration,
        cx: &mut Context<Self>,
    ) {
        self.launch_started = Some(Instant::now() - elapsed);
        cx.notify();
    }
    #[cfg(feature = "rendered-tests")]
    pub(crate) fn fixture_temporal(
        &mut self,
        page: ainc_client::types::WorkPage,
        cx: &mut Context<Self>,
    ) {
        self.page_entity::<crate::temporal::TemporalPage>()
            .update(cx, |view, cx| view.fixture(page, cx));
    }
    #[cfg(feature = "rendered-tests")]
    pub(crate) fn fixture_temporal_error(&mut self, cx: &mut Context<Self>) {
        self.page_entity::<crate::temporal::TemporalPage>()
            .update(cx, |view, cx| view.fixture_error(cx));
    }
    #[cfg(feature = "rendered-tests")]
    pub(crate) fn fixture_temporal_loading(&mut self, cx: &mut Context<Self>) {
        self.page_entity::<crate::temporal::TemporalPage>()
            .update(cx, |view, cx| view.fixture_loading(cx));
    }
    pub(crate) fn fixture_profile_name(&mut self, name: &str, cx: &mut Context<Self>) {
        self.profile.name = name.into();
        cx.notify();
    }
    pub(crate) fn fixture_toast(&mut self, cx: &mut Context<Self>) {
        self.toasts.push(
            "Ticket assigned to Evee",
            Some("Reconcile weekly budget and receipts".into()),
            Tone::Success,
        );
        self.toasts.push(
            "Ticket update failed",
            Some("The daemon did not acknowledge the change.".into()),
            Tone::Danger,
        );
        cx.notify();
    }
    pub(crate) fn fixture_signed_out(&mut self, cx: &mut Context<Self>) {
        self.ui_state.navigate(Route::Assistant);
        self.page_entity::<crate::evee::AssistantPage>()
            .update(cx, |assistant, cx| assistant.fixture_signed_out(cx));
        cx.notify();
    }
    #[cfg(feature = "rendered-tests")]
    pub(crate) fn fixture_components(
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
    pub(crate) fn fixture_tickets_page(&self) -> Entity<crate::tickets::TicketsPage> {
        self.page_entity()
    }
    pub(crate) fn fixture_ticket_detail(&mut self, cx: &mut Context<Self>) {
        use ainc_client::types::TicketCommand;
        let daemon = self.daemon.clone();
        daemon
            .send(TicketCommand::RegisterAgent {
                name: "Evee".into(),
                instructions: "Plan and execute".into(),
                model: "connection-default".into(),
            })
            .expect("fixture agent");
        let id = daemon
            .send(TicketCommand::Create {
                title: "Reconcile weekly budget and receipts".into(),
            })
            .expect("fixture ticket")
            .expect("ticket id");
        daemon
            .send(TicketCommand::AddComment {
                ticket_id: id,
                body: "Pulled the last four statements; two receipts are still missing.".into(),
            })
            .expect("fixture comment");
        self.ui_state.navigate(Route::Tickets);
        self.page_entity::<crate::tickets::TicketsPage>()
            .update(cx, |tickets, cx| {
                tickets.reload();
                tickets.select(id, cx);
            });
        cx.notify();
    }
    #[cfg(feature = "rendered-tests")]
    pub(crate) fn fixture_terminal_unavailable(&mut self, cx: &mut Context<Self>) {
        self.page_entity::<crate::terminal::TerminalPage>()
            .update(cx, |page, cx| page.fixture_unavailable(cx));
        self.ui_state.navigate(Route::Terminal);
        cx.notify();
    }
    pub(crate) fn fixture_recent_commands(&mut self, ids: &[&str], cx: &mut Context<Self>) {
        for id in ids.iter().rev() {
            self.ui_state.remember_command(id);
        }
        cx.notify();
    }
    pub(crate) fn fixture_conversation(&mut self, populated: bool, cx: &mut Context<Self>) {
        self.ui_state.navigate(Route::Assistant);
        self.page_entity::<crate::evee::AssistantPage>()
            .update(cx, |assistant, cx| {
                assistant.fixture_conversation(populated, cx)
            });
        cx.notify();
    }
    pub(crate) fn fixture_selected_model(&self, cx: &App) -> Option<String> {
        self.page_entity::<crate::connections::ConnectionsPage>()
            .read(cx)
            .fixture_selected_model()
    }
    pub(crate) fn fixture_models(&mut self, cx: &mut Context<Self>) {
        self.page_entity::<crate::connections::ConnectionsPage>()
            .update(cx, |page, cx| page.fixture_models(cx));
        cx.notify();
    }
}
