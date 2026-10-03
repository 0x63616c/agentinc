//! Presentation adapter for daemon-owned Codex sign-in. No provider process or
//! credentials are owned by this app.
use crate::daemon::{ConnectionAction, Daemon};
pub use ainc_client::types::Model;
use anyhow::{Result, bail};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

fn present(daemon: Option<&Daemon>) -> Result<&Daemon> {
    daemon.ok_or_else(|| anyhow::anyhow!("Daemon unavailable"))
}
pub fn status(daemon: Option<&Daemon>) -> Result<(Option<String>, Vec<Model>)> {
    let status = present(daemon)?.connection_status()?;
    if let Some(error) = status.error {
        bail!("{error}");
    }
    Ok((status.signed_in_as, status.models))
}
/// Blocks until sign-in finishes or is cancelled; run it on the background executor.
pub fn login(
    daemon: Option<&Daemon>,
    cancel: Arc<AtomicBool>,
    open: impl FnOnce(String),
) -> Result<()> {
    let daemon = present(daemon)?;
    daemon.connection(ConnectionAction::Login)?;
    let mut open = Some(open);
    loop {
        if cancel.load(Ordering::Relaxed) {
            daemon.connection(ConnectionAction::Cancel)?;
        }
        let state = daemon.connection_status()?;
        if let Some(url) = state.auth_url
            && let Some(open) = open.take()
        {
            open(url);
        }
        if !state.signing_in {
            if let Some(error) = state.error {
                bail!("{error}");
            }
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
}
pub fn logout(daemon: Option<&Daemon>) -> Result<()> {
    Ok(present(daemon)?.connection(ConnectionAction::Logout)?)
}
