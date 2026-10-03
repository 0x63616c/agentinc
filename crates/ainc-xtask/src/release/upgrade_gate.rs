//! Run signed, notarized upgrades through the native UI, including Sparkle deltas.
use super::{
    checked, file_server::FileServer, http, output, parse_args, proc::ManagedChild, read_to_string,
    resolve, sparkle, terminal_smoke,
};
use anyhow::{Result, anyhow, bail};
use serde_json::Value;
use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const KNOWN_BROKEN: &[(&str, &str)] = &[
    (
        "0.2.0",
        "update window crashes when Check for Updates closes it",
    ),
    (
        "0.3.1",
        "update window crashes on Check for Updates and Update ready",
    ),
];

fn release_json(app: &Path) -> Result<Value> {
    Ok(serde_json::from_str(&read_to_string(
        &app.join("Contents/Resources/release.json"),
    )?)?)
}

fn field(identity: &Value, name: &str) -> Result<String> {
    identity[name]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("release.json has no {name}"))
}

fn unpack(archive: &Path, destination: &Path) -> Result<PathBuf> {
    fs::create_dir_all(destination)?;
    checked(
        Command::new("tar")
            .arg("-xzf")
            .arg(archive)
            .arg("-C")
            .arg(destination),
    )?;
    let app = destination.join("AgentInc.app");
    if !app.is_dir() {
        bail!("{} has no AgentInc.app", archive.display());
    }
    verify_app(&app)?;
    Ok(app)
}

fn verify_app(app: &Path) -> Result<()> {
    let requirement = "=anchor apple generic and certificate leaf[subject.OU] = \"X9E4HG27NK\" and identifier \"co.worldwidewebb.agentinc\"";
    checked(
        Command::new("/usr/bin/codesign")
            .args(["--verify", "--deep", "--strict", "-R", requirement])
            .arg(app),
    )?;
    checked(
        Command::new("/usr/sbin/spctl")
            .args(["--assess", "--type", "execute"])
            .arg(app),
    )?;
    Ok(())
}

/// Wait until `ready()` holds, woken by writes to `directory`; errors after `seconds`.
#[cfg(target_os = "macos")]
fn wait_for(
    directory: &Path,
    ready: impl Fn() -> bool,
    seconds: u64,
    description: &str,
) -> Result<()> {
    use std::os::fd::{AsRawFd, FromRawFd};
    let watched = fs::File::open(directory)?;
    let queue = unsafe { libc::kqueue() };
    if queue < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let queue = unsafe { std::os::fd::OwnedFd::from_raw_fd(queue) };
    let mut event: libc::kevent = unsafe { std::mem::zeroed() };
    event.ident = watched.as_raw_fd() as usize;
    event.filter = libc::EVFILT_VNODE;
    event.flags = libc::EV_ADD | libc::EV_CLEAR;
    event.fflags = libc::NOTE_WRITE;
    // Register before the first check, so a write in between is not missed.
    let registered = unsafe {
        libc::kevent(
            queue.as_raw_fd(),
            &event,
            1,
            std::ptr::null_mut(),
            0,
            std::ptr::null(),
        )
    };
    if registered < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let deadline = Instant::now() + Duration::from_secs(seconds);
    while !ready() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            bail!("{description}");
        }
        let timeout = libc::timespec {
            tv_sec: remaining.as_secs() as libc::time_t,
            tv_nsec: remaining.subsec_nanos() as libc::c_long,
        };
        let mut fired: libc::kevent = unsafe { std::mem::zeroed() };
        let count = unsafe {
            libc::kevent(
                queue.as_raw_fd(),
                std::ptr::null(),
                0,
                &mut fired,
                1,
                &timeout,
            )
        };
        if count == 0 {
            bail!("{description}");
        }
        if count < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn wait_for(_: &Path, _: impl Fn() -> bool, _: u64, _: &str) -> Result<()> {
    bail!("the upgrade gate needs kqueue, so it runs on macOS only")
}

fn drain_profile(profile: &Path) {
    let discovery = profile.join("daemon/api-url");
    let token = discovery.with_file_name("owner-token");
    if !discovery.is_file() || !token.is_file() {
        return;
    }
    let (Ok(url), Ok(token)) = (fs::read_to_string(&discovery), fs::read_to_string(&token)) else {
        return;
    };
    // Best effort, like the OSError the Python ignored.
    let _ = http::request(
        &format!("{}/internal/drain", url.trim()),
        "POST",
        &[("Authorization", format!("Bearer {}", token.trim()))],
        Some(b""),
        Duration::from_secs(10),
    );
}

fn tail_chars(bytes: &[u8], count: usize) -> String {
    let text = String::from_utf8_lossy(bytes);
    let skip = text.chars().count().saturating_sub(count);
    text.chars().skip(skip).collect()
}

#[derive(Clone, Copy, Debug)]
enum Payload {
    Full,
    Delta,
    BrokenDelta,
}

impl Payload {
    fn verify(self, requests: &[super::file_server::ServedRequest]) -> Result<()> {
        let downloaded = |name: &str| requests.iter().any(|request| request.path == name);
        let delta = downloaded("update.delta");
        let full = downloaded("AgentInc.tar.gz");
        let expected = match self {
            Self::Full => (false, true),
            Self::Delta => (true, false),
            Self::BrokenDelta => (true, true),
        };
        anyhow::ensure!(
            (delta, full) == expected,
            "{self:?}: expected delta/full downloads {expected:?}, got {requests:?}"
        );
        if matches!(self, Self::BrokenDelta) {
            let delta = requests.iter().position(|r| r.path == "update.delta");
            let full = requests.iter().position(|r| r.path == "AgentInc.tar.gz");
            anyhow::ensure!(delta < full, "fallback must follow the failed delta");
        }
        Ok(())
    }
}

fn sparkle_feed(
    serving: &Path,
    app: &Path,
    next_app: &Path,
    key: &Path,
    port: u16,
    payload: Payload,
) -> Result<()> {
    let pem = read_to_string(key)?;
    let current = release_json(app)?;
    let next = release_json(next_app)?;
    let old_build: u64 = field(&current, "build")?.parse()?;
    let next_build: u64 = field(&next, "build")?.parse()?;
    anyhow::ensure!(
        next_build > old_build,
        "Sparkle fixture build must increase"
    );
    let version = field(&next, "version")?;
    let archive = fs::read(serving.join("AgentInc.tar.gz"))?;
    let signature = sparkle::sign_bytes(&archive, &pem)?;
    let mut deltas = String::new();
    if !matches!(payload, Payload::Full) {
        let patch = serving.join("update.delta");
        match payload {
            Payload::Delta => {
                sparkle::create_delta(&crate::root()?, app, next_app, &patch)?;
            }
            Payload::BrokenDelta => {
                // A signed but inapplicable patch exercises real extraction fallback.
                fs::write(&patch, b"deliberately invalid delta for the upgrade gate")?;
            }
            Payload::Full => unreachable!(),
        }
        let bytes = fs::read(&patch)?;
        let signature = sparkle::sign_bytes(&bytes, &pem)?;
        anyhow::ensure!(bytes.len() < archive.len(), "fixture delta must save bytes");
        println!(
            "{payload:?}: {} bytes versus {} full bytes ({:.1}% smaller)",
            bytes.len(),
            archive.len(),
            100.0 * (1.0 - bytes.len() as f64 / archive.len() as f64)
        );
        deltas = format!(
            "<sparkle:deltas><enclosure url=\"http://127.0.0.1:{port}/update.delta\" length=\"{}\" type=\"application/octet-stream\" sparkle:deltaFrom=\"{old_build}\" sparkle:edSignature=\"{signature}\" /></sparkle:deltas>",
            bytes.len()
        );
    }
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<rss version=\"2.0\" xmlns:sparkle=\"http://www.andymatuschak.org/xml-namespaces/sparkle\"><channel><title>AgentInc upgrade gate</title><item><title>AgentInc {version}</title><sparkle:version>{next_build}</sparkle:version><sparkle:shortVersionString>{version}</sparkle:shortVersionString><sparkle:minimumSystemVersion>15.0</sparkle:minimumSystemVersion><description>Upgrade gate fixture</description><enclosure url=\"http://127.0.0.1:{port}/AgentInc.tar.gz\" length=\"{}\" type=\"application/gzip\" sparkle:edSignature=\"{signature}\" />{deltas}</item></channel></rss>\n",
        archive.len()
    );
    fs::write(serving.join("appcast.xml"), sparkle::sign_feed(&xml, &pem)?)?;
    Ok(())
}

fn verify_runtime(profile: &Path, version: &str) -> Result<()> {
    let url = read_to_string(&profile.join("daemon/api-url"))?;
    let identity: Value = serde_json::from_slice(&http::request(
        &format!("{}/version", url.trim()),
        "GET",
        &[],
        None,
        Duration::from_secs(10),
    )?)?;
    anyhow::ensure!(
        identity["version"] == version,
        "running daemon must match installed app"
    );
    http::request(
        &format!("{}/health/ready", url.trim()),
        "GET",
        &[],
        None,
        Duration::from_secs(10),
    )?;
    Ok(())
}

fn exercise(
    mode: &str,
    payload: Payload,
    candidate: &Path,
    newer: &Path,
    key: &Path,
    manifest_tool: &Path,
) -> Result<()> {
    let temporary = tempfile::Builder::new()
        .prefix(&format!("agentinc-upgrade-{mode}-"))
        .tempdir()?;
    let root = temporary.path();
    let app = unpack(candidate, &root.join("install"))?;
    let uses_sparkle = app.join("Contents/Frameworks/Sparkle.framework").is_dir();
    anyhow::ensure!(
        uses_sparkle || matches!(payload, Payload::Full),
        "delta gate requires a Sparkle-enabled candidate"
    );
    let serving = tempfile::Builder::new().tempdir_in(root)?;
    let serving = serving.path();
    fs::copy(newer, serving.join("AgentInc.tar.gz"))?;
    let mut server = FileServer::start(serving)?;
    let port = server.port();
    let next_app = unpack(newer, &root.join("newer"))?;
    let identity = next_app.join("Contents/Resources/release.json");
    let old_version = field(&release_json(&app)?, "version")?;
    let next_identity = release_json(&next_app)?;
    let next_version = field(&next_identity, "version")?;
    if old_version == next_version {
        bail!("fixture must be newer than candidate");
    }
    fs::write(serving.join("notes.md"), "Upgrade gate fixture\n")?;
    fs::write(serving.join("changelog.md"), "Upgrade gate fixture\n")?;
    checked(
        Command::new(manifest_tool)
            .args([
                &identity,
                &serving.join("AgentInc.tar.gz"),
                &serving.join("notes.md"),
                &serving.join("changelog.md"),
            ])
            .arg(format!("http://127.0.0.1:{port}/AgentInc.tar.gz"))
            .arg(serving.join("feed.json"))
            .env("AINC_RELEASE_TEST_KEY", "1")
            .env("UPDATE_SIGNING_KEY_ED25519_PEM", read_to_string(key)?),
    )?;
    if uses_sparkle {
        sparkle_feed(serving, &app, &next_app, key, port, payload)?;
    }
    let profile = root.join("profile");
    fs::create_dir(&profile)?;
    let marker = root.join("relaunch.txt");
    let downloaded = root.join("downloaded.txt");
    let preparation = root.join("preparation.txt");
    let stderr_path = root.join("app.stderr.log");
    let mut command = Command::new(app.join("Contents/MacOS/AgentInc"));
    command
        .env(
            "AINC_UPGRADE_TEST_FEED_URL",
            format!("http://127.0.0.1:{port}/feed.json"),
        )
        .env("AINC_UPGRADE_TEST_MODE", mode)
        .env(
            "AINC_UPGRADE_TEST_SPARKLE_FEED_URL",
            format!("http://127.0.0.1:{port}/appcast.xml"),
        )
        .env("AINC_UPGRADE_TEST_FROM", &old_version)
        .env("AINC_UPGRADE_TEST_SUCCESS_FILE", &marker)
        .env("AINC_UPGRADE_TEST_DOWNLOADED_FILE", &downloaded)
        .env("AINC_UPGRADE_TEST_PREPARATION_FILE", &preparation)
        .env("AGENTINC_SESSION_PATH", profile.join("sessions.json"))
        .env("AINC_DISCOVERY_FILE", profile.join("daemon/api-url"))
        .env("AINC_LEGACY_DIR", profile.join("legacy"))
        .env("AGENTINC_CODEX_HOME", profile.join("codex"))
        .stdout(Stdio::null())
        .stderr(fs::File::create(&stderr_path)?);
    let mut process = ManagedChild::spawn(command)?;
    let result = (|| -> Result<()> {
        if mode == "download-only" {
            use std::os::unix::process::ExitStatusExt;
            wait_for(
                root,
                || downloaded.is_file(),
                240,
                "background update was not cached",
            )?;
            anyhow::ensure!(
                !preparation.exists(),
                "background download armed Sparkle installation before consent"
            );
            verify_runtime(&profile, &old_version)?;
            process.kill();
            let status = process.wait_timeout(Duration::from_secs(30))?;
            anyhow::ensure!(
                status.signal() == Some(libc::SIGKILL),
                "crash gate did not kill the actual UI process"
            );
            anyhow::ensure!(
                !preparation.exists(),
                "background update prepared an installer before UI death"
            );
            anyhow::ensure!(
                field(&release_json(&app)?, "version")? == old_version,
                "download-only crash replaced the app"
            );
            verify_app(&app)?;
            verify_runtime(&profile, &old_version)?;
            terminal_smoke::check(&app, &profile)?;
            payload.verify(&server.requests())?;
            println!(
                "PASS download-only {payload:?}: UI SIGKILL left {old_version} and its runtime intact; no Sparkle preparation"
            );
            return Ok(());
        }
        let status = process.wait_timeout(Duration::from_secs(240))?;
        let stderr = fs::read(&stderr_path)?;
        if !status.success() {
            bail!(
                "{mode} candidate exited {}: {}",
                status.code().unwrap_or(-1),
                tail_chars(&stderr, 4000)
            );
        }
        wait_for(
            root,
            || marker.is_file(),
            240,
            &format!("updated app did not relaunch: {}", marker.display()),
        )?;
        let relaunch = read_to_string(&marker)?;
        let mut fields = relaunch.split_whitespace();
        let (version, pid_text) = (
            fields
                .next()
                .ok_or_else(|| anyhow!("empty relaunch marker"))?,
            fields
                .next()
                .ok_or_else(|| anyhow!("relaunch marker has no PID"))?,
        );
        if version != next_version {
            bail!("{mode} relaunched {version}, expected {next_version}");
        }
        if field(&release_json(&app)?, "version")? != next_version {
            bail!("{mode} install did not replace bundle");
        }
        let backup = app.with_extension("previous.app");
        wait_for(
            app.parent().unwrap(),
            || !backup.exists(),
            120,
            &format!(
                "{mode} installer did not finish; see {}",
                profile.join("updates/install.log").display()
            ),
        )?;
        if unsafe { libc::kill(pid_text.parse()?, 0) } != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        verify_app(&app)?;
        verify_runtime(&profile, &next_version)?;
        terminal_smoke::check(&app, &profile)?;
        if uses_sparkle {
            anyhow::ensure!(
                read_to_string(&preparation)?.trim()
                    == format!("{} prepared=1 fence=armed", field(&next_identity, "build")?),
                "Sparkle extraction must follow completed daemon drain and an armed startup fence"
            );
            let requests = server.requests();
            payload.verify(&requests)?;
            let transferred: usize = requests
                .iter()
                .filter(|request| {
                    matches!(request.path.as_str(), "update.delta" | "AgentInc.tar.gz")
                })
                .map(|request| request.bytes)
                .sum();
            println!("PASS {payload:?} transport: {transferred} payload bytes downloaded");
        }
        println!(
            "PASS {mode} {payload:?}: {old_version} -> {next_version}, relaunch PID {pid_text}"
        );
        Ok(())
    })();
    if result.is_err() {
        println!("{}", tail_chars(&fs::read(&stderr_path)?, 8000));
        let log = profile.join("updates/install.log");
        if log.is_file() {
            println!("{}", tail_chars(&fs::read(&log)?, 4000));
        }
    }
    process.terminate();
    let stopped = process.wait_timeout(Duration::from_secs(10));
    let mut cleanup = Ok(());
    if marker.is_file()
        && let Some(pid) = fs::read_to_string(&marker)?.split_whitespace().nth(1)
        && unsafe { libc::kill(pid.parse()?, 15) } != 0
    {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::ESRCH) {
            cleanup = Err(error.into());
        }
    }
    drain_profile(&profile);
    server.shutdown();
    result?;
    stopped?;
    cleanup
}

fn parse_version(version: &str) -> Result<Vec<u64>> {
    version
        .split('.')
        .map(|part| part.parse::<u64>().map_err(|e| anyhow!("{version}: {e}")))
        .collect()
}

/// Shipped clients have the GitHub feed URL and production key compiled in.
/// Their candidate upgrade cannot be staged with a test key before publication.
fn published_predecessors(candidate_version: &str, root: &Path) -> Result<HashMap<String, String>> {
    let repo = env::var("GITHUB_REPOSITORY").map_err(|_| anyhow!("GITHUB_REPOSITORY"))?;
    let mut releases = Vec::new();
    for page in 1.. {
        let batch: Value = serde_json::from_str(&output(
            "gh",
            &[
                "api",
                &format!("repos/{repo}/releases?per_page=100&page={page}"),
            ],
        )?)?;
        let batch = batch.as_array().cloned().unwrap_or_default();
        let last = batch.len() < 100;
        releases.extend(batch);
        if last {
            break;
        }
    }
    let candidate = parse_version(candidate_version)?;
    let mut published = HashMap::new();
    for release in releases {
        let tag = release["tag_name"].as_str().unwrap_or("");
        if release["draft"].as_bool().unwrap_or(false) || !tag.starts_with('v') {
            continue;
        }
        let version = &tag[1..];
        if parse_version(version)? >= candidate {
            continue;
        }
        let has_asset = release["assets"]
            .as_array()
            .is_some_and(|assets| assets.iter().any(|a| a["name"] == "AgentInc.tar.gz"));
        if !has_asset {
            bail!("{version} has no published app archive");
        }
        let directory = root.join(version);
        fs::create_dir(&directory)?;
        checked(
            Command::new("gh")
                .args([
                    "release",
                    "download",
                    tag,
                    "--pattern",
                    "AgentInc.tar.gz",
                    "--dir",
                ])
                .arg(&directory),
        )?;
        let app = unpack(
            &directory.join("AgentInc.tar.gz"),
            &directory.join("installed"),
        )?;
        let identity = release_json(&app)?;
        if field(&identity, "version")? != version {
            bail!(
                "published {version} contains {}",
                field(&identity, "version")?
            );
        }
        published.insert(version.to_string(), field(&identity, "commit")?);
        match KNOWN_BROKEN.iter().find(|(known, _)| *known == version) {
            Some((_, reason)) => println!("KNOWN BROKEN {version}: {reason}"),
            None => println!(
                "SHIPPED FEED LOCKED {version}: signed install verified; test-key rebuild required for candidate upgrade"
            ),
        }
    }
    Ok(published)
}

pub fn cli(args: &[String]) -> Result<()> {
    let opts = parse_args(
        "release-upgrade-gate",
        args,
        &[],
        &[
            "--candidate",
            "--newer",
            "--key",
            "--manifest-tool",
            "--prior",
        ],
    );
    let candidate = resolve(Path::new(opts.required("--candidate")));
    let newer = resolve(Path::new(opts.required("--newer")));
    let key = resolve(Path::new(opts.required("--key")));
    let manifest_tool = resolve(Path::new(opts.required("--manifest-tool")));
    let published = {
        let temporary = tempfile::Builder::new()
            .prefix("agentinc-published-")
            .tempdir()?;
        let candidate_version = {
            let install = tempfile::Builder::new()
                .prefix("agentinc-candidate-")
                .tempdir()?;
            field(
                &release_json(&unpack(&candidate, install.path())?)?,
                "version",
            )?
        };
        published_predecessors(&candidate_version, temporary.path())?
    };
    for prior in opts.many("--prior") {
        let prior = resolve(Path::new(prior));
        let identity = {
            let install = tempfile::Builder::new()
                .prefix("agentinc-prior-")
                .tempdir()?;
            release_json(&unpack(&prior, install.path())?)?
        };
        let (version, commit) = (field(&identity, "version")?, field(&identity, "commit")?);
        if published.get(&version) != Some(&commit) {
            bail!("prior test build does not match published {version}");
        }
        for mode in ["manual", "automatic"] {
            exercise(
                mode,
                Payload::Full,
                &prior,
                &candidate,
                &key,
                &manifest_tool,
            )?;
        }
    }
    for mode in ["manual", "automatic"] {
        exercise(
            mode,
            Payload::Full,
            &candidate,
            &newer,
            &key,
            &manifest_tool,
        )?;
    }
    for payload in [Payload::Delta, Payload::BrokenDelta] {
        for mode in ["manual", "automatic"] {
            exercise(mode, payload, &candidate, &newer, &key, &manifest_tool)?;
        }
    }
    exercise(
        "download-only",
        Payload::Delta,
        &candidate,
        &newer,
        &key,
        &manifest_tool,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_evidence_requires_delta_only_or_ordered_fallback() {
        use super::super::file_server::ServedRequest;
        let delta = ServedRequest {
            path: "update.delta".into(),
            bytes: 10,
        };
        let full = ServedRequest {
            path: "AgentInc.tar.gz".into(),
            bytes: 100,
        };
        assert!(Payload::Full.verify(std::slice::from_ref(&full)).is_ok());
        assert!(Payload::Delta.verify(std::slice::from_ref(&delta)).is_ok());
        assert!(
            Payload::Delta
                .verify(&[delta.clone(), full.clone()])
                .is_err()
        );
        assert!(
            Payload::BrokenDelta
                .verify(&[delta.clone(), full.clone()])
                .is_ok()
        );
        assert!(Payload::BrokenDelta.verify(&[full, delta]).is_err());
        assert!(Payload::Full.verify(&[]).is_err());
    }

    #[test]
    fn versions_compare_numerically_like_python_tuples() {
        assert!(parse_version("0.3.5").unwrap() < parse_version("0.10.0").unwrap());
        assert!(parse_version("0.4.0").unwrap() >= parse_version("0.4.0").unwrap());
        assert!(parse_version("0.4.x").is_err());
    }

    #[test]
    fn stderr_tail_keeps_the_last_characters() {
        assert_eq!(tail_chars("héllo wörld".as_bytes(), 5), "wörld");
        assert_eq!(tail_chars(b"abc", 4000), "abc");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn wait_for_wakes_on_directory_writes_and_times_out() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("marker");
        let writer = {
            let marker = marker.clone();
            std::thread::spawn(move || fs::write(marker, "x").unwrap())
        };
        wait_for(dir.path(), || marker.is_file(), 60, "no marker").unwrap();
        writer.join().unwrap();
        let error = wait_for(dir.path(), || false, 0, "never").unwrap_err();
        assert_eq!(error.to_string(), "never");
    }
}
