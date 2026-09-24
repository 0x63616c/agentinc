use crate::{Manifest, SignedManifest};
use anyhow::{Context, Result, ensure};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

pub const TEAM_ID: &str = "X9E4HG27NK";
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub automatic_checks: bool,
    pub interval_hours: u64,
    pub automatic_download: bool,
    pub skipped_version: Option<String>,
    pub last_check: u64,
    pub remind_after: u64,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            automatic_checks: true,
            interval_hours: 24,
            automatic_download: false,
            skipped_version: None,
            last_check: 0,
            remind_after: 0,
        }
    }
}
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
impl Preferences {
    pub fn load(path: &Path) -> Result<Self> {
        match fs::read(path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        fs::create_dir_all(path.parent().context("update preference directory")?)?;
        let temp = path.with_extension("tmp");
        fs::write(&temp, serde_json::to_vec_pretty(self)?)?;
        fs::rename(temp, path)?;
        Ok(())
    }
    pub fn due(&self, at: u64) -> bool {
        self.automatic_checks
            && at >= self.remind_after
            && at.saturating_sub(self.last_check) >= self.interval_hours.max(1).saturating_mul(3600)
    }
}
fn http() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent("AgentInc updater")
        .https_only(true)
        .timeout(std::time::Duration::from_secs(300))
        .build()?)
}
pub async fn check(feed: &str, key: &str) -> Result<(SignedManifest, Manifest)> {
    ensure!(!key.is_empty(), "update public key is not configured");
    let response = http()?.get(feed).send().await?.error_for_status()?;
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        bytes.extend_from_slice(&chunk?);
        ensure!(bytes.len() <= 4 * 1024 * 1024, "update feed is too large");
    }
    let signed: SignedManifest = serde_json::from_slice(&bytes)?;
    let manifest = signed.verify(key)?;
    Ok((signed, manifest))
}
pub async fn download(manifest: &Manifest, path: &Path, progress: Arc<AtomicU64>) -> Result<()> {
    let mut stream = http()?
        .get(&manifest.archive_url)
        .send()
        .await?
        .error_for_status()?
        .bytes_stream();
    let temp = path.with_extension("partial");
    let mut file = fs::File::create(&temp)?;
    let mut size = 0;
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    while let Some(bytes) = stream.next().await {
        let bytes = bytes?;
        size += bytes.len() as u64;
        ensure!(
            size <= manifest.archive_bytes,
            "update archive exceeds signed size"
        );
        hash.update(&bytes);
        file.write_all(&bytes)?;
        progress.store(size, Ordering::Relaxed);
    }
    ensure!(
        size == manifest.archive_bytes
            && format!("{:x}", hash.finalize()) == manifest.archive_sha256,
        "update archive verification failed"
    );
    file.sync_all()?;
    fs::rename(temp, path)?;
    Ok(())
}

/// Signature and digest are checked again in the installer process, before extraction.
pub fn extract(
    signed: &SignedManifest,
    key: &str,
    archive: &Path,
    stage: &Path,
) -> Result<Manifest> {
    let manifest = signed.verify(key)?;
    let bytes = fs::read(archive)?;
    manifest.verify_archive(&bytes)?;
    ensure!(!stage.exists(), "update staging directory already exists");
    fs::create_dir(stage)?;
    let result = (|| {
        let decoder = flate2::read::GzDecoder::new(bytes.as_slice());
        let mut tar = tar::Archive::new(decoder);
        for entry in tar.entries()? {
            let mut entry = entry?;
            let path = entry.path()?.into_owned();
            ensure!(
                path.components().next()
                    == Some(std::path::Component::Normal(std::ffi::OsStr::new(
                        "AgentInc.app"
                    ))),
                "unexpected archive root"
            );
            ensure!(entry.unpack_in(stage)?, "unsafe archive entry");
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(stage);
    }
    result?;
    Ok(manifest)
}

pub fn verify_bundle(bundle: &Path, manifest: &Manifest) -> Result<()> {
    let identity: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("Contents/Resources/release.json"))?)?;
    ensure!(
        identity["version"] == manifest.version.to_string()
            && identity["commit"] == manifest.commit
            && identity["build"] == manifest.build,
        "bundle does not match signed release identity"
    );
    let requirement = format!(
        "anchor apple generic and certificate leaf[subject.OU] = \"{TEAM_ID}\" and identifier \"co.worldwidewebb.agentinc\""
    );
    ensure!(
        Command::new("/usr/bin/codesign")
            .args(["--verify", "--deep", "--strict", "-R", &requirement])
            .arg(bundle)
            .status()?
            .success(),
        "update code signature rejected"
    );
    ensure!(
        Command::new("/usr/sbin/spctl")
            .args(["--assess", "--type", "execute"])
            .arg(bundle)
            .status()?
            .success(),
        "update notarization assessment rejected"
    );
    Ok(())
}

/// Two renames on the same volume; preserve the old app until restart succeeds.
pub fn replace_bundle(installed: &Path, staged: &Path) -> Result<PathBuf> {
    let backup = installed.with_extension("previous.app");
    ensure!(
        !backup.exists(),
        "previous installation backup needs recovery before updating"
    );
    fs::rename(installed, &backup)
        .context("move installed app aside; check directory permissions")?;
    if let Err(error) = fs::rename(staged, installed) {
        fs::rename(&backup, installed).context("restore previous app after replacement failure")?;
        return Err(error.into());
    }
    Ok(backup)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preferences_preserve_skip_and_remind_without_delaying_manual_checks() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("updates.json");
        let mut prefs = Preferences::default();
        assert!(prefs.due(100_000));
        prefs.remind_after = 200_000;
        prefs.skipped_version = Some("0.2.0".into());
        prefs.save(&path).unwrap();
        let loaded = Preferences::load(&path).unwrap();
        assert!(!loaded.due(100_000));
        assert!(loaded.due(200_000));
        assert_eq!(loaded.skipped_version.as_deref(), Some("0.2.0"));
    }
    #[test]
    fn failed_replace_restores_original() {
        let root = tempfile::tempdir().unwrap();
        let app = root.path().join("AgentInc.app");
        fs::create_dir(&app).unwrap();
        fs::write(app.join("identity"), "old").unwrap();
        assert!(replace_bundle(&app, &root.path().join("missing")).is_err());
        assert_eq!(fs::read_to_string(app.join("identity")).unwrap(), "old");
    }
}
