//! The one owner of daemon polling. `Sync` fetches every cached slice through
//! the [`Daemon`] on one cadence, tells pages which slices changed, and holds
//! the load state they used to each keep for themselves.
//!
//! Cadence: 2 s while the window is visible and nothing while it is not. The
//! clock is injected so tests drive ticks without timers.
use crate::{
    action::{Failure, Pending, Run},
    daemon::Daemon,
    ui::LoadState,
};
use gpui::{BackgroundExecutor, Context, EventEmitter};
use std::{pin::Pin, rc::Rc, sync::Arc, time::Duration, time::Instant};

/// One cached slice of daemon state changed since the last fetch set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliceChanged {
    Workspaces,
    Todos,
    Calendar,
}
const SLICES: [SliceChanged; 3] = [
    SliceChanged::Workspaces,
    SliceChanged::Todos,
    SliceChanged::Calendar,
];

/// How `Sync` waits between fetch sets.
pub trait Clock: 'static {
    fn wait(&self, interval: Duration) -> Pin<Box<dyn Future<Output = ()>>>;
}
/// Real timers on the background executor.
pub struct Timers(pub BackgroundExecutor);
impl Clock for Timers {
    fn wait(&self, interval: Duration) -> Pin<Box<dyn Future<Output = ()>>> {
        Box::pin(self.0.timer(interval))
    }
}
/// A clock the test advances by hand: every [`ManualClock::tick`] releases
/// every wait in progress, and the intervals `Sync` asked for are recorded.
#[cfg(test)]
#[derive(Clone, Default)]
pub struct ManualClock(Arc<manual::Inner>);
#[cfg(test)]
mod manual {
    use super::*;
    use std::sync::Mutex;
    #[derive(Default)]
    pub struct Inner {
        pub ticks: tokio::sync::Notify,
        pub intervals: Mutex<Vec<Duration>>,
    }
    impl ManualClock {
        pub fn tick(&self) {
            self.0.ticks.notify_waiters();
        }
        pub fn intervals(&self) -> Vec<Duration> {
            self.0.intervals.lock().expect("intervals").clone()
        }
    }
    impl Clock for ManualClock {
        fn wait(&self, interval: Duration) -> Pin<Box<dyn Future<Output = ()>>> {
            self.0.intervals.lock().expect("intervals").push(interval);
            let inner = self.0.clone();
            Box::pin(async move { inner.ticks.notified().await })
        }
    }
}

const IDLE: Duration = Duration::from_secs(2);

/// What one fetch set learned, computed on the background executor.
struct Fetched {
    fingerprints: [String; 3],
}
fn fetch(daemon: &Daemon) -> anyhow::Result<Fetched> {
    daemon.refresh()?;
    let fingerprints = [
        serde_json::to_string(&daemon.workspaces())?,
        serde_json::to_string(&daemon.todos())?,
        serde_json::to_string(&daemon.calendar())?,
    ];
    Ok(Fetched { fingerprints })
}

pub struct Sync {
    daemon: Arc<Daemon>,
    clock: Rc<dyn Clock>,
    pending: Pending,
    seen: [Option<String>; 3],
    /// A wake supersedes the wait that was already scheduled.
    generation: u64,
    paused: bool,
    /// The first fetch set has landed.
    pub loaded: bool,
    /// The latest fetch set failed; cleared by the next one that succeeds.
    pub error: Option<Failure>,
    /// Set when a fetch set starts after a failure, for a "Reconnecting…" hint.
    pub loading_started: Instant,
}
impl EventEmitter<SliceChanged> for Sync {}
impl Sync {
    pub fn new(daemon: Arc<Daemon>, clock: impl Clock, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            loaded: false,
            daemon,
            clock: Rc::new(clock),
            pending: Pending::default(),
            seen: Default::default(),
            generation: 0,
            paused: false,
            error: None,
            loading_started: Instant::now(),
        };
        this.step(cx);
        this
    }
    /// A fetch set is in flight.
    pub fn fetching(&self) -> bool {
        self.pending.busy()
    }
    /// A fetch set is in flight after a failure.
    pub fn reconnecting(&self) -> bool {
        self.error.is_some() && self.pending.busy()
    }
    /// How the page frame shows this data: loading, failed or reconnecting.
    pub fn load_state(&self) -> LoadState {
        LoadState {
            loaded: self.loaded,
            error: self.message(),
            reconnecting: self.reconnecting(),
            started: self.loading_started,
        }
    }
    /// The load error, worded for a banner.
    pub fn message(&self) -> Option<String> {
        self.error.as_ref().map(|e| e.message("The daemon"))
    }
    /// Stop fetching while the window cannot be seen; resume on the next tick.
    pub fn set_paused(&mut self, paused: bool, cx: &mut Context<Self>) {
        if self.paused == paused {
            return;
        }
        self.paused = paused;
        if !paused {
            self.wake(cx);
        }
    }
    /// Fetch now instead of at the next tick, and re-plan the cadence after it.
    pub fn wake(&mut self, cx: &mut Context<Self>) {
        self.generation += 1;
        self.step(cx);
    }
    fn interval(&self) -> Duration {
        IDLE
    }

    fn step(&mut self, cx: &mut Context<Self>) {
        if self.paused {
            self.schedule(cx);
            return;
        }
        let daemon = self.daemon.clone();
        if self.error.is_some() {
            self.loading_started = Instant::now();
        }
        // Refused while a fetch set is in flight; that set's landing schedules the next.
        cx.run(
            &self.pending,
            move || fetch(&daemon),
            |this, result, cx| {
                this.land(result, cx);
                this.schedule(cx);
            },
        );
    }
    fn land(&mut self, result: Result<Fetched, Failure>, cx: &mut Context<Self>) {
        match result {
            Ok(fetched) => {
                self.loaded = true;
                self.error = None;
                for (slot, (seen, fresh)) in SLICES
                    .into_iter()
                    .zip(self.seen.iter_mut().zip(fetched.fingerprints))
                {
                    if seen.as_deref() != Some(fresh.as_str()) {
                        *seen = Some(fresh);
                        cx.emit(slot);
                    }
                }
            }
            Err(failure) => self.error = Some(failure),
        }
    }
    fn schedule(&mut self, cx: &mut Context<Self>) {
        let generation = self.generation;
        let wait = self.clock.wait(self.interval());
        cx.spawn(async move |this, cx| {
            wait.await;
            let _ = this.update(cx, |this, cx| {
                if this.generation == generation {
                    this.step(cx);
                }
            });
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::transport::{Request, Slice};
    use ainc_client::types::TodoCommand;
    use gpui::{AppContext, TestAppContext};
    use std::{cell::RefCell, rc::Rc};

    fn fetch_sets(daemon: &Daemon) -> Vec<Vec<Slice>> {
        daemon
            .memory()
            .requests()
            .into_iter()
            .filter_map(|request| match request {
                Request::Fetch(slice) => Some(slice),
                _ => None,
            })
            .collect::<Vec<_>>()
            .chunks(3)
            .map(<[Slice]>::to_vec)
            .collect()
    }
    fn start(cx: &mut TestAppContext) -> (Arc<Daemon>, ManualClock, gpui::Entity<Sync>) {
        let daemon = Arc::new(Daemon::in_memory());
        let clock = ManualClock::default();
        let sync = cx.new(|cx| Sync::new(daemon.clone(), clock.clone(), cx));
        cx.run_until_parked();
        (daemon, clock, sync)
    }

    #[gpui::test]
    fn each_tick_fetches_one_set_of_slices(cx: &mut TestAppContext) {
        let (daemon, clock, sync) = start(cx);
        let one_set = vec![Slice::Workspaces, Slice::Todos, Slice::Calendar];
        assert_eq!(
            fetch_sets(&daemon),
            vec![one_set.clone()],
            "the first set is immediate"
        );
        sync.read_with(cx, |sync, _| {
            assert!(sync.loaded);
            assert!(sync.error.is_none());
        });
        for ticks in 2..=3 {
            clock.tick();
            cx.run_until_parked();
            assert_eq!(fetch_sets(&daemon), vec![one_set.clone(); ticks]);
        }
        assert_eq!(clock.intervals(), vec![IDLE; 3]);
    }

    #[gpui::test]
    fn only_the_slice_that_changed_is_announced(cx: &mut TestAppContext) {
        let (daemon, clock, sync) = start(cx);
        let changed = Rc::new(RefCell::new(vec![]));
        let seen = changed.clone();
        let _subscription = cx.update(|cx| {
            cx.subscribe(&sync, move |_, event: &SliceChanged, _| {
                seen.borrow_mut().push(*event);
            })
        });
        daemon
            .send(TodoCommand::Add {
                title: "Pay rent".into(),
            })
            .unwrap();
        clock.tick();
        cx.run_until_parked();
        assert_eq!(changed.borrow().as_slice(), &[SliceChanged::Todos]);
        clock.tick();
        cx.run_until_parked();
        assert_eq!(
            changed.borrow().len(),
            1,
            "an unchanged set announces nothing"
        );
    }

    #[gpui::test]
    fn a_failure_is_reported_once_and_cleared_by_the_next_success(cx: &mut TestAppContext) {
        let (daemon, clock, sync) = start(cx);
        daemon
            .memory()
            .fail_next(crate::daemon::DaemonError::Unavailable(
                "daemon down".into(),
            ));
        clock.tick();
        cx.run_until_parked();
        sync.read_with(cx, |sync, _| {
            assert_eq!(
                sync.message().as_deref(),
                Some("The daemon is unavailable. Try again.")
            );
            assert!(sync.loaded, "an earlier set already loaded");
        });
        clock.tick();
        cx.run_until_parked();
        sync.read_with(cx, |sync, _| assert!(sync.error.is_none()));
    }

    #[gpui::test]
    fn paused_sync_fetches_nothing_and_resumes_at_once(cx: &mut TestAppContext) {
        let (daemon, clock, sync) = start(cx);
        sync.update(cx, |sync, cx| sync.set_paused(true, cx));
        clock.tick();
        cx.run_until_parked();
        assert_eq!(fetch_sets(&daemon).len(), 1);
        daemon
            .send(TodoCommand::Add {
                title: "Pay rent".into(),
            })
            .unwrap();
        daemon.memory().clear_requests();
        sync.update(cx, |sync, cx| sync.set_paused(false, cx));
        cx.run_until_parked();
        assert_eq!(
            fetch_sets(&daemon).len(),
            1,
            "resuming fetches without a tick"
        );
    }
}
