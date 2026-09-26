//! Provider secrets live in the macOS Keychain. They never reach plain files,
//! logs, product records, workflow history or API responses.
use anyhow::{Context, Result};
use std::{collections::HashMap, sync::Mutex};

pub trait SecretStore: Send + Sync + 'static {
    fn get(&self, name: &str) -> Result<Option<String>>;
    fn set(&self, name: &str, value: &str) -> Result<()>;
    fn delete(&self, name: &str) -> Result<()>;
}

/// An in-memory store for tests and for platforms without a Keychain.
#[derive(Default)]
pub struct MemoryStore(Mutex<HashMap<String, String>>);
impl SecretStore for MemoryStore {
    fn get(&self, name: &str) -> Result<Option<String>> {
        Ok(self.0.lock().expect("secret store").get(name).cloned())
    }
    fn set(&self, name: &str, value: &str) -> Result<()> {
        self.0
            .lock()
            .expect("secret store")
            .insert(name.into(), value.into());
        Ok(())
    }
    fn delete(&self, name: &str) -> Result<()> {
        self.0.lock().expect("secret store").remove(name);
        Ok(())
    }
}

/// The login Keychain, keyed by the product bundle ID so the development and
/// production channels never share provider credentials.
pub struct Keychain {
    service: String,
}
impl Keychain {
    pub fn product() -> Self {
        Self {
            service: format!("{}.providers", ainc_release::identity::BUNDLE_ID),
        }
    }
}
#[cfg(target_os = "macos")]
impl SecretStore for Keychain {
    fn get(&self, name: &str) -> Result<Option<String>> {
        match security_framework::passwords::get_generic_password(&self.service, name) {
            Ok(bytes) => Ok(Some(
                String::from_utf8(bytes).context("Keychain secret is not text")?,
            )),
            Err(error) if error.code() == -25300 => Ok(None), // errSecItemNotFound
            Err(_) => anyhow::bail!("Keychain is unavailable. Unlock the login Keychain."),
        }
    }
    fn set(&self, name: &str, value: &str) -> Result<()> {
        security_framework::passwords::set_generic_password(&self.service, name, value.as_bytes())
            .map_err(|_| anyhow::anyhow!("Keychain refused to store the secret."))
    }
    fn delete(&self, name: &str) -> Result<()> {
        match security_framework::passwords::delete_generic_password(&self.service, name) {
            Ok(()) => Ok(()),
            Err(error) if error.code() == -25300 => Ok(()),
            Err(_) => anyhow::bail!("Keychain refused to remove the secret."),
        }
    }
}
#[cfg(not(target_os = "macos"))]
impl SecretStore for Keychain {
    fn get(&self, _: &str) -> Result<Option<String>> {
        anyhow::bail!(
            "Keychain storage is only available on macOS ({}).",
            self.service
        )
    }
    fn set(&self, _: &str, _: &str) -> Result<()> {
        anyhow::bail!(
            "Keychain storage is only available on macOS ({}).",
            self.service
        )
    }
    fn delete(&self, _: &str) -> Result<()> {
        anyhow::bail!(
            "Keychain storage is only available on macOS ({}).",
            self.service
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn memory_store_round_trips_and_forgets() {
        let store = MemoryStore::default();
        assert_eq!(store.get("openrouter").unwrap(), None);
        store.set("openrouter", "sk-fixture").unwrap();
        assert_eq!(
            store.get("openrouter").unwrap().as_deref(),
            Some("sk-fixture")
        );
        store.delete("openrouter").unwrap();
        assert_eq!(store.get("openrouter").unwrap(), None);
    }
}
