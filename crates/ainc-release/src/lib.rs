//! Authenticated update metadata: the signed manifest, release notes and the updater. The
//! product identity it builds on is `ainc-identity`, re-exported below.
pub mod notes;
// Everything a binary needs without the updater lives in `ainc-identity`; re-exported so
// `ainc_release::VERSION`, `ainc_release::identity` and `ainc_release::process` keep working.
pub use ainc_identity::*;
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// Set this one value to the production Ed25519 public key (32 bytes, base64).
// Production Ed25519 public key; private counterpart lives only in repository secrets.
pub const UPDATE_PUBLIC_KEY: &str = "mscHoRK2B71KNlJGlNqTCkMSqvuW9YWOHa1owg7S2yc=";
pub const FEED_URL: &str =
    "https://github.com/0x63616c/agentinc/releases/latest/download/feed.json";

pub fn update_public_key() -> &'static str {
    #[cfg(ainc_upgrade_test)]
    {
        return env!("AINC_UPGRADE_TEST_PUBLIC_KEY");
    }
    #[cfg(not(ainc_upgrade_test))]
    {
        UPDATE_PUBLIC_KEY
    }
}

pub fn update_feed_url() -> String {
    #[cfg(ainc_upgrade_test)]
    {
        return std::env::var("AINC_UPGRADE_TEST_FEED_URL").expect("upgrade test feed URL");
    }
    #[cfg(not(ainc_upgrade_test))]
    {
        FEED_URL.into()
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
    /// All changes since the installed version; legacy feeds retain their latest notes.
    pub fn notes_since(&self, current: &str) -> String {
        let (Ok(current), Some(releases)) = (
            Version::parse(current),
            notes::parse_changelog(&self.changelog),
        ) else {
            return self.notes.clone();
        };
        if !releases
            .iter()
            .any(|release| release.version == self.version)
        {
            return self.notes.clone();
        }
        let releases = releases
            .into_iter()
            .filter(|release| release.version > current && release.version <= self.version)
            .collect::<Vec<_>>();
        notes::changelog(&releases)
    }

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
            version: Version::parse(VERSION).unwrap(),
            daemon_version: Version::parse(VERSION).unwrap(),
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
        assert!(!manifest.is_upgrade(VERSION, "aarch64").unwrap());
        let newer = Version::new(manifest.version.major, manifest.version.minor + 1, 0);
        assert!(!manifest.is_upgrade(&newer.to_string(), "aarch64").unwrap());
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
    fn skipped_versions_show_only_missed_releases_newest_first() {
        let mut manifest = fixture();
        manifest.version = Version::new(0, 4, 3);
        manifest.changelog = notes::changelog(&[0, 1, 2, 3, 4].map(|patch| notes::ReleaseNotes {
            version: Version::new(0, 4, patch),
            notes: format!("- Change {patch}"),
        }));
        let selected = notes::parse_changelog(&manifest.notes_since("0.4.0")).unwrap();
        assert_eq!(
            selected.iter().map(|r| r.version.patch).collect::<Vec<_>>(),
            [3, 2, 1]
        );
        assert_eq!(selected[0].notes, "- Change 3");
        assert!(
            notes::parse_changelog(&manifest.notes_since("0.4.3"))
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            notes::parse_changelog(&manifest.changelog).unwrap().len(),
            5
        );
    }

    #[test]
    fn legacy_feeds_and_incomplete_history_keep_latest_notes() {
        let mut manifest = fixture();
        assert_eq!(manifest.notes_since("0.1.0"), "Notes");
        manifest.changelog = notes::changelog(&[notes::ReleaseNotes {
            version: Version::new(0, 1, 0),
            notes: "Old".into(),
        }]);
        assert_eq!(manifest.notes_since("0.1.0"), "Notes");
        assert_eq!(manifest.notes_since("invalid"), "Notes");
    }

    #[test]
    fn versioned_history_is_authenticated_without_changing_schema_one_fields() {
        let mut manifest = fixture();
        manifest.changelog = notes::changelog(&[notes::ReleaseNotes {
            version: manifest.version.clone(),
            notes: "- A direct commit".into(),
        }]);
        let key = SigningKey::from_bytes(&[7; 32]);
        let public = STANDARD.encode(key.verifying_key().to_bytes());
        let mut signed = SignedManifest::sign(&manifest, &key).unwrap();
        let mut payload: serde_json::Value =
            serde_json::from_slice(&STANDARD.decode(&signed.payload).unwrap()).unwrap();
        // This is the exact schema accepted by installed deny_unknown_fields readers.
        let mut fields = payload
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>();
        fields.sort();
        assert_eq!(
            fields,
            [
                "api",
                "architecture",
                "archive_bytes",
                "archive_sha256",
                "archive_url",
                "build",
                "changelog",
                "commit",
                "daemon_version",
                "minimum_client",
                "notes",
                "schema",
                "version"
            ]
        );
        assert_eq!(
            signed.verify(&public).unwrap().changelog,
            manifest.changelog
        );
        payload["changelog"] = "changed history".into();
        signed.payload = STANDARD.encode(serde_json::to_vec(&payload).unwrap());
        assert!(signed.verify(&public).is_err());
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
}

pub mod updater;
