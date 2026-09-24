//! Presentation adapter for daemon-owned Codex sign-in. No provider process or
//! credentials are owned by this app.
use crate::storage::{background, client};
pub use ainc_client::types::Model;
use anyhow::{Result, bail};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub fn status() -> Result<(Option<String>, Vec<Model>)> {
    background(async {
        let status = client()
            .await?
            .connection_status()
            .send()
            .await?
            .into_inner();
        if let Some(error) = status.error {
            bail!("{error}");
        }
        Ok((status.account, status.models))
    })
}
pub fn login(cancel: Arc<AtomicBool>, open: impl FnOnce(String)) -> Result<()> {
    background(async {
        let client = client().await?;
        client.connection_login().send().await?;
        let mut open = Some(open);
        loop {
            if cancel.load(Ordering::Relaxed) {
                client.connection_cancel().send().await?;
            }
            let state = client.connection_status().send().await?.into_inner();
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
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
    })
}
pub fn logout() -> Result<()> {
    background(async {
        client().await?.connection_logout().send().await?;
        Ok(())
    })
}
