//! The one shape for asynchronous work in the app: run an operation on the
//! background executor, hand its result back to the entity on the foreground,
//! and keep a second submission out while the first is in flight.
//!
//! This is the only place that touches the background executor.
use crate::daemon::DaemonError;
use gpui::Context;
use std::{cell::Cell, rc::Rc};

/// Whether an entity has an action in flight. Clone it to share one guard
/// between the entity and the completion that releases it.
#[derive(Clone, Default, Debug)]
pub struct Pending(Rc<Cell<bool>>);
impl Pending {
    pub fn busy(&self) -> bool {
        self.0.get()
    }
    /// Rendered fixtures: stay busy so a page keeps showing its in-flight state.
    #[cfg(all(test, feature = "rendered-tests"))]
    pub(crate) fn hold(&self) {
        self.0.set(true);
    }
    fn claim(&self) -> Option<Claim> {
        if self.0.replace(true) {
            return None;
        }
        Some(Claim(self.0.clone()))
    }
}
struct Claim(Rc<Cell<bool>>);
impl Drop for Claim {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

/// Why an action did not complete, worded for a person. Pages show
/// [`Failure::message`]; the raw detail stays behind for a disclosure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    recovery: &'static str,
    detail: String,
}
impl Failure {
    /// `{Thing} is unavailable. {Recovery}.`
    pub fn message(&self, thing: &str) -> String {
        format!("{thing} is unavailable. {}", self.recovery)
    }
}
impl From<anyhow::Error> for Failure {
    fn from(error: anyhow::Error) -> Self {
        let recovery = match error.downcast_ref::<DaemonError>() {
            Some(DaemonError::UpdateRequired) => "Update AgentInc.",
            Some(DaemonError::Rejected(body)) if body.code == "pending" => {
                "Retry the unacknowledged change first."
            }
            Some(DaemonError::Rejected(_)) => "Check the change and try again.",
            Some(DaemonError::Retryable(_) | DaemonError::Unavailable(_)) | None => "Try again.",
        };
        Self {
            recovery,
            detail: format!("{error:#}"),
        }
    }
}

pub trait Run<T: 'static> {
    /// Run `op` on the background executor, then `apply` its result to this
    /// entity. Refused (returns `false`) while `pending` is busy. Notifies
    /// observers when the action starts and when it lands.
    fn run<R: Send + 'static>(
        &mut self,
        pending: &Pending,
        op: impl FnOnce() -> anyhow::Result<R> + Send + 'static,
        apply: impl FnOnce(&mut T, Result<R, Failure>, &mut Context<T>) + 'static,
    ) -> bool;
}
impl<T: 'static> Run<T> for Context<'_, T> {
    fn run<R: Send + 'static>(
        &mut self,
        pending: &Pending,
        op: impl FnOnce() -> anyhow::Result<R> + Send + 'static,
        apply: impl FnOnce(&mut T, Result<R, Failure>, &mut Context<T>) + 'static,
    ) -> bool {
        let Some(claim) = pending.claim() else {
            return false;
        };
        let task = self.background_executor().spawn(async move { op() });
        self.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                drop(claim);
                apply(this, result.map_err(Failure::from), cx);
                cx.notify();
            });
        })
        .detach();
        self.notify();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{AppContext, TestAppContext};

    struct Counter {
        pending: Pending,
        landed: Vec<Result<u32, Failure>>,
    }

    #[gpui::test]
    fn a_second_submission_is_refused_until_the_first_lands(cx: &mut TestAppContext) {
        let counter = cx.new(|_| Counter {
            pending: Pending::default(),
            landed: vec![],
        });
        counter.update(cx, |this, cx| {
            let pending = this.pending.clone();
            assert!(cx.run(
                &pending,
                || Ok(1),
                |this, result, _| this.landed.push(result)
            ));
            assert!(this.pending.busy());
            assert!(!cx.run(
                &pending,
                || Ok(2),
                |this, result, _| this.landed.push(result)
            ));
        });
        cx.run_until_parked();
        counter.read_with(cx, |this, _| {
            assert!(!this.pending.busy());
            assert_eq!(this.landed, vec![Ok(1)]);
        });
    }

    #[gpui::test]
    fn failures_are_worded_once(cx: &mut TestAppContext) {
        let counter = cx.new(|_| Counter {
            pending: Pending::default(),
            landed: vec![],
        });
        counter.update(cx, |this, cx| {
            let pending = this.pending.clone();
            cx.run(
                &pending,
                || Err(DaemonError::Unavailable("connection refused".into()).into()),
                |this, result, _| this.landed.push(result),
            );
        });
        cx.run_until_parked();
        counter.read_with(cx, |this, _| {
            let failure = this.landed[0].as_ref().unwrap_err();
            assert_eq!(
                failure.message("The Ticket board"),
                "The Ticket board is unavailable. Try again."
            );
            assert!(failure.detail.contains("connection refused"));
        });
    }
}
