//! Product release identity, wire compatibility and authenticated update metadata.
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const API: u32 = 1;
pub const MIN_CLIENT: &str = "0.1.0";
pub const BUILD: &str = match option_env!("AINC_BUILD_ID") {
    Some(value) => value,
    None => "development",
};
// Set this one value to the production Ed25519 public key (32 bytes, base64).
// Empty deliberately disables production updates until the key is provisioned.
pub const UPDATE_PUBLIC_KEY: &str = "";
pub const FEED_URL: &str =
    "https://github.com/0x63616c/agentinc/releases/latest/download/feed.json";

pub fn client_header() -> String {
    format!("mac/{VERSION} (build {BUILD}; api {API})")
}
pub fn server_header() -> String {
    format!("aincd/{VERSION} (build {BUILD}; api {API})")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityError {
    UpgradeRequired,
    ServerTooOld,
    InvalidClient,
}
impl std::fmt::Display for CompatibilityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Update to continue")
    }
}
impl std::error::Error for CompatibilityError {}

pub fn check_client(header: &str, minimum: &str, api: u32) -> Result<(), CompatibilityError> {
    let parse = || -> Option<(Version, u32)> {
        let (identity, details) = header.split_once(" (")?;
        let (_, version) = identity.split_once('/')?;
        let api = details
            .strip_suffix(')')?
            .rsplit_once("api ")?
            .1
            .parse()
            .ok()?;
        Some((Version::parse(version).ok()?, api))
    };
    let (version, client_api) = parse().ok_or(CompatibilityError::InvalidClient)?;
    let minimum = Version::parse(minimum).map_err(|_| CompatibilityError::InvalidClient)?;
    if client_api > api {
        Err(CompatibilityError::ServerTooOld)
    } else if client_api < api || version < minimum {
        Err(CompatibilityError::UpgradeRequired)
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: Version,
    pub build: String,
    pub commit: String,
    pub daemon_version: Version,
    pub minimum_client: Version,
    pub api: u32,
    pub schema: u32,
    pub architecture: String,
    pub archive_url: String,
    pub archive_sha256: String,
    pub archive_bytes: u64,
    pub notes: String,
    pub changelog: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedManifest {
    // Authenticate these exact bytes. Never reserialize before verification.
    pub payload: String,
    pub signature: String,
}
impl SignedManifest {
    pub fn sign(manifest: &Manifest, key: &SigningKey) -> Result<Self> {
        let payload = serde_json::to_vec(manifest)?;
        Ok(Self {
            signature: STANDARD.encode(key.sign(&payload).to_bytes()),
            payload: STANDARD.encode(payload),
        })
    }
    pub fn verify(&self, public_key: &str) -> Result<Manifest> {
        let bytes: [u8; 32] = STANDARD
            .decode(public_key)
            .context("update public key is not configured")?
            .try_into()
            .map_err(|_| anyhow::anyhow!("update public key must be 32 bytes"))?;
        let key = VerifyingKey::from_bytes(&bytes)?;
        let payload = STANDARD.decode(&self.payload)?;
        let signature = Signature::from_slice(&STANDARD.decode(&self.signature)?)?;
        key.verify_strict(&payload, &signature)
            .context("update signature rejected")?;
        let manifest: Manifest = serde_json::from_slice(&payload)?;
        ensure!(manifest.api == API, "update API is incompatible");
        ensure!(
            manifest.minimum_client <= manifest.version,
            "invalid compatibility window"
        );
        ensure!(
            manifest.daemon_version == manifest.version,
            "app/daemon version mismatch"
        );
        ensure!(
            manifest.commit.len() == 40 && manifest.commit.bytes().all(|b| b.is_ascii_hexdigit()),
            "invalid release commit"
        );
        ensure!(manifest.schema == 1, "unsupported release schema");
        Ok(manifest)
    }
}
impl Manifest {
    pub fn verify_archive(&self, archive: &[u8]) -> Result<()> {
        ensure!(
            archive.len() as u64 == self.archive_bytes,
            "update archive length mismatch"
        );
        ensure!(
            format!("{:x}", Sha256::digest(archive)) == self.archive_sha256,
            "update archive digest mismatch"
        );
        Ok(())
    }
    pub fn is_upgrade(&self, current: &str, architecture: &str) -> Result<bool> {
        ensure!(
            self.architecture == architecture,
            "update architecture mismatch"
        );
        Ok(self.version > Version::parse(current)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Manifest {
        Manifest {
            version: Version::new(0, 2, 0),
            daemon_version: Version::new(0, 2, 0),
            minimum_client: Version::new(0, 1, 0),
            api: API,
            schema: 1,
            build: "42".into(),
            commit: "a".repeat(40),
            architecture: "aarch64".into(),
            archive_url: "https://example.com/app.tar.gz".into(),
            archive_sha256: format!("{:x}", Sha256::digest(b"archive")),
            archive_bytes: 7,
            notes: "Notes".into(),
            changelog: "History".into(),
        }
    }
    #[test]
    fn authenticates_metadata_and_archive_before_upgrade() {
        let key = SigningKey::from_bytes(&[7; 32]);
        let public = STANDARD.encode(key.verifying_key().to_bytes());
        let manifest = fixture();
        let signed = SignedManifest::sign(&manifest, &key).unwrap();
        assert_eq!(signed.verify(&public).unwrap(), manifest);
        assert!(manifest.verify_archive(b"archive").is_ok());
        assert!(manifest.verify_archive(b"tamper!").is_err());
        assert!(manifest.is_upgrade("0.1.0", "aarch64").unwrap());
        assert!(!manifest.is_upgrade("0.2.0", "aarch64").unwrap());
        assert!(!manifest.is_upgrade("0.3.0", "aarch64").unwrap());
        assert!(manifest.is_upgrade("0.1.0", "x86_64").is_err());
        let mut tampered = signed.clone();
        tampered.payload = STANDARD.encode(b"{}");
        assert!(tampered.verify(&public).is_err());
        let other = SigningKey::from_bytes(&[8; 32]);
        assert!(
            signed
                .verify(&STANDARD.encode(other.verifying_key().to_bytes()))
                .is_err()
        );
        assert!(signed.verify(UPDATE_PUBLIC_KEY).is_err());
    }
    #[test]
    fn refuses_mixed_versions_and_unsupported_contracts() {
        let key = SigningKey::from_bytes(&[7; 32]);
        let public = STANDARD.encode(key.verifying_key().to_bytes());
        let mut manifest = fixture();
        manifest.daemon_version = Version::new(0, 1, 0);
        assert!(
            SignedManifest::sign(&manifest, &key)
                .unwrap()
                .verify(&public)
                .is_err()
        );
    }
    #[test]
    fn compatibility_is_reciprocal() {
        assert_eq!(check_client(&client_header(), MIN_CLIENT, API), Ok(()));
        assert_eq!(
            check_client("mac/0.0.1 (api 1)", MIN_CLIENT, API),
            Err(CompatibilityError::UpgradeRequired)
        );
        assert_eq!(
            check_client("mac/0.1.0 (api 2)", MIN_CLIENT, API),
            Err(CompatibilityError::ServerTooOld)
        );
        assert_eq!(
            check_client("", MIN_CLIENT, API),
            Err(CompatibilityError::InvalidClient)
        );
    }
}

pub mod updater;
