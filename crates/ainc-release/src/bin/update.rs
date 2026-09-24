//! Out-of-process installer. The running app launches its own signed helper.
use ainc_release::{SignedManifest, updater};
use anyhow::{Context, Result, ensure};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command},
    time::{Duration, Instant},
};
fn main() {
    if let Err(error) = install() {
        eprintln!("Update failed: {error:#}");
        // Restore an app window even when pre-install checks or draining fail.
        if let Some(path) = std::env::args_os().nth(1) {
            let path = PathBuf::from(path);
            if path.is_dir() {
                let _ = launch(&path);
            }
        }
        std::process::exit(1);
    }
}
fn install() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let _ = install_from(
        &args,
        ainc_release::UPDATE_PUBLIC_KEY,
        ainc_release::VERSION,
    )?;
    Ok(())
}
fn launch(installed: &Path) -> Result<Child> {
    // Launch the exact new executable, preserving the profile environment.
    // LaunchServices can otherwise activate an unrelated copy with the same ID.
    Ok(Command::new(installed.join("Contents/MacOS/agentinc-os")).spawn()?)
}
fn install_from(args: &[std::ffi::OsString], key: &str, current: &str) -> Result<Child> {
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
    let manifest = updater::extract(&signed, key, &update.join("app.tar.gz"), &stage)?;
    ensure!(
        manifest.is_upgrade(current, std::env::consts::ARCH)?,
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
    let app = match launch(&installed) {
        Ok(app) => app,
        Err(error) => {
            let failed = stage.join("failed.app");
            fs::rename(&installed, failed)?;
            fs::rename(&backup, &installed)?;
            let _ = launch(&installed);
            return Err(error.context("relaunch failed; previous installation restored"));
        }
    };
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
    Ok(app)
}

#[cfg(all(test, target_os = "macos"))]
mod acceptance {
    use super::*;
    use std::io::{BufRead, Read};
    #[test]
    #[ignore = "requires a notarized draft and an isolated native app profile"]
    fn signed_update_drains_replaces_and_relaunches() {
        let source = PathBuf::from(
            std::env::var_os("AINC_SIGNED_UPDATE_DIR").expect("signed artifact input"),
        );
        let root = PathBuf::from(
            std::env::var_os("AINC_INSTALL_TEST_ROOT").expect("isolated install root"),
        );
        let discovery =
            PathBuf::from(std::env::var_os("AINC_DISCOVERY_FILE").expect("isolated discovery"));
        assert!(discovery.starts_with(&root));
        assert!(!root.exists(), "use a fresh test profile");
        for name in [
            "AGENTINC_SESSION_PATH",
            "AINC_LEGACY_DIR",
            "AGENTINC_CODEX_HOME",
        ] {
            assert!(PathBuf::from(std::env::var_os(name).expect(name)).starts_with(&root));
        }
        fs::create_dir_all(&root).unwrap();
        let key = fs::read_to_string(source.join("test-public-key.txt")).unwrap();
        let signed: SignedManifest =
            serde_json::from_slice(&fs::read(source.join("feed.json")).unwrap()).unwrap();
        let initial = root.join("initial");
        let manifest = updater::extract(
            &signed,
            key.trim(),
            &source.join("AgentInc.tar.gz"),
            &initial,
        )
        .unwrap();
        let installed = root.join("AgentInc.app");
        fs::rename(initial.join("AgentInc.app"), &installed).unwrap();
        let cache = root.join("cache");
        fs::create_dir(&cache).unwrap();
        fs::copy(source.join("AgentInc.tar.gz"), cache.join("app.tar.gz")).unwrap();
        fs::copy(source.join("feed.json"), cache.join("feed.json")).unwrap();
        let mut daemon = Command::new(installed.join("Contents/MacOS/aincd"))
            .env("RUST_LOG", "info")
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let output = daemon.stdout.take().unwrap();
        let mut reader = std::io::BufReader::new(output);
        let mut ready = false;
        for line in reader.by_ref().lines() {
            if line.unwrap().contains("AgentInc daemon ready") {
                ready = true;
                break;
            }
        }
        assert!(ready, "initial signed daemon did not become ready");
        let logs = std::thread::spawn(move || {
            for line in reader.lines() {
                let _ = line;
            }
        });
        let mut parent = Command::new("/usr/bin/true").spawn().unwrap();
        let pid = parent.id();
        parent.wait().unwrap();
        let args = [
            installed.as_os_str().to_owned(),
            cache.into_os_string(),
            pid.to_string().into(),
            discovery.as_os_str().to_owned(),
        ];
        // The predecessor version is supplied by this fixture; the complete
        // production installer path and real notarized app are used unchanged.
        let mut app = install_from(&args, key.trim(), "0.0.0").unwrap();
        assert!(daemon.wait().unwrap().success());
        logs.join().unwrap();
        assert!(!installed.with_extension("previous.app").exists());
        updater::verify_bundle(&installed, &manifest).unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let url = fs::read_to_string(&discovery).unwrap();
            let token = fs::read_to_string(discovery.with_file_name("owner-token")).unwrap();
            let response = reqwest::Client::new()
                .post(format!("{}/internal/drain", url.trim()))
                .bearer_auth(token.trim())
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), reqwest::StatusCode::ACCEPTED);
        });
        app.kill().unwrap();
        app.wait().unwrap();
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(discovery.with_extension("lock"))
            .unwrap();
        lock.lock().unwrap();
        println!(
            "Real signed installer drained the daemon, replaced the bundle, relaunched AgentInc, verified readiness and retired the backup: {}",
            manifest.commit
        );
    }
}
