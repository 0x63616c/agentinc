//! Out-of-process installer. The running app launches its own signed helper.
use ainc_release::{SignedManifest, updater};
use anyhow::{Context, Result, ensure};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};
fn main() {
    if let Err(error) = install() {
        eprintln!("Update failed: {error:#}");
        // Restore an app window even when pre-install checks or draining fail.
        if let Some(path) = std::env::args_os().nth(1) {
            let path = PathBuf::from(path);
            if path.is_dir() {
                let _ = Command::new("/usr/bin/open").arg(path).status();
            }
        }
        std::process::exit(1);
    }
}
fn install() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    ensure!(
        args.len() == 4,
        "usage: ainc-update INSTALLED_APP UPDATE_DIRECTORY PARENT_PID DISCOVERY_FILE"
    );
    let installed = PathBuf::from(&args[0]).canonicalize()?;
    let update = PathBuf::from(&args[1]);
    let parent: i32 = args[2].to_str().context("parent PID")?.parse()?;
    ensure!(parent > 1, "invalid parent PID");
    let discovery = PathBuf::from(&args[3]);
    let signed: SignedManifest = serde_json::from_slice(&fs::read(update.join("feed.json"))?)?;
    let stage = installed
        .parent()
        .context("install parent")?
        .join(format!(".agentinc-update-{}", std::process::id()));
    let manifest = updater::extract(
        &signed,
        ainc_release::UPDATE_PUBLIC_KEY,
        &update.join("app.tar.gz"),
        &stage,
    )?;
    ensure!(
        manifest.is_upgrade(ainc_release::VERSION, std::env::consts::ARCH)?,
        "refusing update rollback"
    );
    let staged = stage.join("AgentInc.app");
    updater::verify_bundle(&staged, &manifest)?;
    // Wait for the UI to flush its drafts and exit before draining the daemon.
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        // SAFETY: signal zero only checks liveness; no process is modified.
        if unsafe { libc::kill(parent, 0) } != 0 {
            break;
        }
        ensure!(
            Instant::now() < deadline,
            "app did not close; installation left untouched"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        if let Ok(url) = fs::read_to_string(&discovery) {
            let token = fs::read_to_string(discovery.with_file_name("owner-token"))?;
            let response = reqwest::Client::new()
                .post(format!("{}/internal/drain", url.trim()))
                .bearer_auth(token.trim())
                .timeout(Duration::from_secs(10))
                .send()
                .await?;
            ensure!(
                response.status() == reqwest::StatusCode::ACCEPTED,
                "daemon refused update drain"
            );
        }
        anyhow::Ok(())
    })?;
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(discovery.with_extension("lock"))?;
    let deadline = Instant::now() + Duration::from_secs(120);
    while lock.try_lock().is_err() {
        ensure!(
            Instant::now() < deadline,
            "daemon has not drained; installation left untouched"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    let backup = updater::replace_bundle(&installed, &staged)?;
    // Let the new daemon acquire its profile lock.
    drop(lock);
    let status = Command::new("/usr/bin/open").arg(&installed).status()?;
    if !status.success() {
        let failed = stage.join("failed.app");
        fs::rename(&installed, failed)?;
        fs::rename(&backup, &installed)?;
        let _ = Command::new("/usr/bin/open").arg(&installed).status();
        anyhow::bail!("relaunch failed; previous installation restored");
    }
    // The new app must reconnect to a compatible, ready daemon before retiring
    // the previous bundle. Keep it on failure; never roll back migrated data.
    runtime.block_on(async {
        let deadline = Instant::now() + Duration::from_secs(90);
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()?;
        loop {
            if let Ok(url) = fs::read_to_string(&discovery) {
                let version = http.get(format!("{}/version", url.trim())).send().await;
                if let Ok(response) = version
                    && let Ok(identity) = response.json::<serde_json::Value>().await
                    && identity["version"] == manifest.version.to_string()
                {
                    let ready = http
                        .get(format!("{}/health/ready", url.trim()))
                        .send()
                        .await;
                    if ready.is_ok_and(|response| response.status().is_success()) {
                        break;
                    }
                }
            }
            ensure!(
                Instant::now() < deadline,
                "new runtime did not become ready; previous bundle retained for recovery"
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        anyhow::Ok(())
    })?;
    fs::remove_dir_all(backup)?;
    fs::remove_dir_all(stage)?;
    Ok(())
}
