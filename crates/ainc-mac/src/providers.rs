//! Presentation adapter for daemon-owned model providers. The app never sees
//! a credential: keys go straight to the daemon, which keeps them in the Keychain.
use crate::storage::{background, client};
use ainc_client::types::{ConnectRequest, TestRequest};
pub use ainc_client::types::{
    ProviderId, ProviderModel, ProviderStatus, ProviderTest, ProvidersState,
};
use anyhow::Result;

pub fn state(refresh: bool) -> Result<ProvidersState> {
    background(async {
        Ok(client()
            .await?
            .providers_state()
            .refresh(refresh)
            .send()
            .await?
            .into_inner())
    })
}
pub fn connect(
    id: ProviderId,
    api_key: Option<String>,
    code: Option<String>,
) -> Result<ProvidersState> {
    background(async {
        Ok(client()
            .await?
            .provider_connect()
            .id(id.to_string())
            .body(ConnectRequest { api_key, code })
            .send()
            .await
            .map_err(describe)?
            .into_inner())
    })
}
pub fn cancel(id: ProviderId) -> Result<ProvidersState> {
    background(async {
        Ok(client()
            .await?
            .provider_cancel()
            .id(id.to_string())
            .send()
            .await
            .map_err(describe)?
            .into_inner())
    })
}
pub fn disconnect(id: ProviderId) -> Result<ProvidersState> {
    background(async {
        Ok(client()
            .await?
            .provider_disconnect()
            .id(id.to_string())
            .send()
            .await
            .map_err(describe)?
            .into_inner())
    })
}
pub fn test(id: ProviderId, model: Option<String>) -> Result<ProviderTest> {
    background(async {
        Ok(client()
            .await?
            .provider_test()
            .id(id.to_string())
            .body(TestRequest { model })
            .send()
            .await
            .map_err(describe)?
            .into_inner())
    })
}
/// Surface the daemon's own message for refused requests.
fn describe(error: ainc_client::Error<ainc_client::types::ErrorBody>) -> anyhow::Error {
    match error {
        ainc_client::Error::ErrorResponse(response) => anyhow::anyhow!("{}", response.message),
        other => anyhow::anyhow!("{other}"),
    }
}
