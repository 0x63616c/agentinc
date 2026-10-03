//! Linux signing/notarization and idempotent GitHub distribution.
//! Only main may publish. Test jobs leave a draft; credentials never enter arguments.
use super::{
    checked, ci_gate, create_tar_gz, extract_tar_gz, inventory, notes as release_notes, output,
    output_bytes, parse_args, read_to_string, resolve, truthy,
};
use anyhow::{Result, anyhow, bail};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::Value;
use std::{
    collections::{HashMap, VecDeque},
    env, fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::Mutex,
    thread,
    time::Instant,
};

/// One unit of concurrent work: a bundle to sign or the manifest tool to build.
pub type Task<'a> = Box<dyn FnOnce() -> Result<()> + Send + 'a>;

/// Everything distribution does to the outside world, so the flow can be tested without it.
pub trait Shell: Sync {
    /// `subprocess.check_output(args, text=True).strip()`.
    fn output(&self, args: &[String]) -> Result<String>;
    /// `subprocess.check_output(args)` as bytes.
    fn output_bytes(&self, args: &[String]) -> Result<Vec<u8>>;
    /// `subprocess.run(args, check=True)`, with the whole environment replaced when given.
    fn run(&self, args: &[String], env: Option<&HashMap<String, String>>) -> Result<()>;
    /// `subprocess.run(args, capture_output=True)`: the exit code and stdout, no check.
    fn probe(&self, args: &[String]) -> Result<(i32, String)>;
    fn sign_all(&self, tasks: Vec<Task<'_>>, build_manifest: Task<'_>) -> Result<()> {
        sign_all(tasks, build_manifest)
    }
    fn wait_for_ci(&self, repo: &str, commit: &str) -> Result<()>;
}

struct Real;

fn strs(args: &[String]) -> Vec<&str> {
    args.iter().map(String::as_str).collect()
}

impl Shell for Real {
    fn output(&self, args: &[String]) -> Result<String> {
        output(&args[0], &strs(&args[1..]))
    }

    fn output_bytes(&self, args: &[String]) -> Result<Vec<u8>> {
        output_bytes(&args[0], &strs(&args[1..]))
    }

    fn run(&self, args: &[String], env: Option<&HashMap<String, String>>) -> Result<()> {
        let mut command = crate::spawn::command(&args[0]);
        command.args(&args[1..]);
        if let Some(env) = env {
            command.env_clear().envs(env);
        }
        checked(&mut command)
    }

    fn probe(&self, args: &[String]) -> Result<(i32, String)> {
        let result = crate::spawn::command(&args[0]).args(&args[1..]).output()?;
        Ok((
            result.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&result.stdout).into_owned(),
        ))
    }

    fn wait_for_ci(&self, _repo: &str, commit: &str) -> Result<()> {
        ci_gate::wait_for_ci(commit)
    }
}

fn args(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| item.to_string()).collect()
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub fn sign_bundle(
    shell: &dyn Shell,
    env: &HashMap<String, String>,
    bundle: &Path,
    archive: &Path,
    private: &Path,
) -> Result<()> {
    let started = Instant::now();
    println!("Signing/notarizing {}", name(archive));
    let team = env
        .get("APPLE_TEAM_ID")
        .ok_or_else(|| anyhow!("APPLE_TEAM_ID"))?;
    // rcodesign's bundle signer recursively signs deepest nested bundles first:
    // Sparkle's XPC services and Updater.app, then Versions/B (including
    // Autoupdate), then the host. Do not use --shallow or re-sign after archiving;
    // the native Sparkle job verifies --deep --strict before creating deltas.
    shell.run(
        &[
            "rcodesign".into(),
            "sign".into(),
            "--p12-file".into(),
            text(&private.join("identity.p12")),
            "--p12-password-file".into(),
            text(&private.join("password")),
            "--team-name".into(),
            team.clone(),
            "--for-notarization".into(),
            text(bundle),
        ],
        None,
    )?;
    shell.run(
        &[
            "rcodesign".into(),
            "notary-submit".into(),
            "--api-key-file".into(),
            text(&private.join("notary.json")),
            "--wait".into(),
            "--staple".into(),
            text(bundle),
        ],
        None,
    )?;
    create_tar_gz(archive, 9, &[(bundle, "AgentInc.app")])?;
    println!(
        "Signed, notarized, stapled {}: {:.1}s",
        name(archive),
        started.elapsed().as_secs_f64()
    );
    Ok(())
}

/// Independent bundles and output paths; each worker still waits for Apple's acceptance and
/// staples its bundle. Any failure prevents upload/publication. Up to four bundles sign while
/// the manifest tool builds, and everything finishes before the first error is reported.
pub fn sign_all(tasks: Vec<Task<'_>>, build_manifest: Task<'_>) -> Result<()> {
    let count = tasks.len();
    let queue = Mutex::new(tasks.into_iter().enumerate().collect::<VecDeque<_>>());
    let results = Mutex::new(
        (0..count)
            .map(|_| None)
            .collect::<Vec<Option<Result<()>>>>(),
    );
    let manifest = thread::scope(|scope| {
        let manifest = scope.spawn(build_manifest);
        let workers = (0..count.min(4))
            .map(|_| {
                scope.spawn(|| {
                    loop {
                        let next = queue.lock().unwrap().pop_front();
                        let Some((index, task)) = next else { break };
                        let result = task();
                        results.lock().unwrap()[index] = Some(result);
                    }
                })
            })
            .collect::<Vec<_>>();
        for worker in workers {
            let _ = worker.join();
        }
        manifest
            .join()
            .unwrap_or_else(|_| Err(anyhow!("manifest build panicked")))
    });
    for result in results.into_inner().unwrap() {
        result.unwrap_or_else(|| Err(anyhow!("signing task did not run")))?;
    }
    manifest
}

fn key<'a>(value: &'a Value, name: &str) -> Result<&'a Value> {
    value.get(name).ok_or_else(|| anyhow!("missing {name}"))
}

fn files_match(bundle: &Path, identity: &Value) -> Result<bool> {
    Ok(Value::Object(inventory(bundle)?) == *key(identity, "files")?)
}

/// Sign an upgrade fixture only after its identity and file inventory check out.
pub fn sign_upgrade_fixture(
    source: &Path,
    destination: &Path,
    commit: &str,
    version: Option<&str>,
    sign: &dyn Fn(&Path, &Path) -> Result<()>,
) -> Result<()> {
    let directory = tempfile::tempdir()?;
    let directory = directory.path();
    extract_tar_gz(source, directory)?;
    let identity: Value = serde_json::from_str(&read_to_string(&directory.join("handoff.json"))?)?;
    if key(&identity, "commit")?.as_str() != Some(commit)
        || identity.get("upgrade_test") != Some(&Value::Bool(true))
        || version.is_some_and(|version| {
            key(&identity, "version").ok().and_then(Value::as_str) != Some(version)
        })
    {
        bail!("upgrade fixture is not a test build from {commit}");
    }
    let bundle = directory.join("AgentInc.app");
    if !files_match(&bundle, &identity)? {
        bail!("upgrade fixture inventory mismatch");
    }
    sign(&bundle, destination)
}

fn rev_parse(shell: &dyn Shell, rev: &str) -> Result<String> {
    shell.output(&args(&["git", "rev-parse", rev]))
}

fn product_version(metadata: &Value) -> Result<String> {
    key(metadata, "packages")?
        .as_array()
        .and_then(|packages| {
            packages
                .iter()
                .find(|package| package["name"] == "ainc-release")
        })
        .and_then(|package| package["version"].as_str())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("ainc-release is not in cargo metadata"))
}

fn json_output(shell: &dyn Shell, command: &[&str]) -> Result<Value> {
    Ok(serde_json::from_str(&shell.output(&args(command))?)?)
}

pub fn cli(raw: &[String]) -> Result<()> {
    let env: HashMap<String, String> = env::vars().collect();
    let base = env::current_dir()?;
    main(raw, &env, &base, &Real)
}

/// The whole script. `base` is the working directory that relative paths resolve against.
pub fn main(
    raw: &[String],
    env: &HashMap<String, String>,
    base: &Path,
    shell: &dyn Shell,
) -> Result<()> {
    let opts = parse_args(
        "release-distribute",
        raw,
        &["--test", "--stage"],
        &[
            "--commit",
            "--archive",
            "--upgrade-candidate",
            "--upgrade-newer",
            "--upgrade-prior",
        ],
    );
    let requested = opts.required("--commit").to_string();
    let test = opts.flag("--test");
    let stage = opts.flag("--stage");
    if !test && !stage {
        bail!("Sparkle releases must use --stage, then the native delta/signing/publication gates");
    }
    let archive_arg = opts.one("--archive").map(PathBuf::from);
    let candidate_arg = opts.one("--upgrade-candidate").map(PathBuf::from);
    let newer_arg = opts.one("--upgrade-newer").map(PathBuf::from);
    let priors: Vec<PathBuf> = opts
        .many("--upgrade-prior")
        .iter()
        .map(PathBuf::from)
        .collect();
    let in_base = |path: &Path| base.join(path);

    if !test
        && env
            .get("UPDATE_SIGNING_KEY_ED25519_PEM")
            .is_none_or(|key| key.trim().is_empty())
    {
        bail!("publish refused: UPDATE_SIGNING_KEY_ED25519_PEM is missing");
    }
    if !test && env.get("GITHUB_REF").map(String::as_str) != Some("refs/heads/main") {
        bail!("publish refused: only main may publish");
    }
    let commit = rev_parse(shell, &format!("{requested}^{{commit}}"))?;
    if commit != requested || rev_parse(shell, "HEAD")? != commit {
        bail!("release checkout does not match requested commit");
    }
    let repo = env
        .get("GITHUB_REPOSITORY")
        .ok_or_else(|| anyhow!("GITHUB_REPOSITORY"))?
        .clone();
    if !test {
        shell.run(
            &args(&["git", "merge-base", "--is-ancestor", &commit, "origin/main"]),
            None,
        )?;
    }
    let handoff_tag = format!("build-{commit}");
    fs::create_dir_all(base.join(".local/distribution"))?;
    let out = resolve(&base.join(".local/distribution"));
    if let Some(archive) = &archive_arg {
        if resolve(&in_base(archive)) != out.join("unsigned.tar.gz") {
            fs::copy(in_base(archive), out.join("unsigned.tar.gz"))?;
        }
    } else {
        shell.run(
            &args(&[
                "gh",
                "release",
                "download",
                &handoff_tag,
                "--pattern",
                "unsigned.tar.gz",
                "--dir",
                &text(&out),
                "--clobber",
            ]),
            None,
        )?;
    }
    extract_tar_gz(&out.join("unsigned.tar.gz"), &out)?;
    let identity: Value = serde_json::from_str(&read_to_string(&out.join("handoff.json"))?)?;
    if key(&identity, "commit")?.as_str() != Some(commit.as_str()) {
        bail!("handoff commit mismatch");
    }
    if truthy(identity.get("upgrade_test")) {
        bail!("publish refused: production handoff contains upgrade-test code");
    }
    let metadata = json_output(
        shell,
        &["cargo", "metadata", "--no-deps", "--format-version=1"],
    )?;
    let version = product_version(&metadata)?;
    if key(&identity, "version")?.as_str() != Some(version.as_str()) {
        bail!("handoff version mismatch");
    }
    let bundle = out.join("AgentInc.app");
    if !files_match(&bundle, &identity)? {
        bail!("handoff inventory mismatch");
    }
    let tag = if test {
        format!("phase5-test-{}", &commit[..12.min(commit.len())])
    } else {
        format!("v{version}")
    };
    let (code, stdout) = shell.probe(&args(&[
        "gh",
        "release",
        "view",
        &tag,
        "--json",
        "targetCommitish,isDraft,body",
    ]))?;
    let mut notes = None;
    if code == 0 {
        let release: Value = serde_json::from_str(&stdout)?;
        if key(&release, "targetCommitish")?.as_str() != Some(commit.as_str()) {
            bail!("release version already belongs to another commit");
        }
        if key(&release, "isDraft")? != &Value::Bool(true) {
            println!("Release already published for this commit");
            return Ok(());
        }
        // A null body would have crashed the Python; treat it as empty.
        notes = Some(key(&release, "body")?.as_str().unwrap_or("").to_string());
    }
    let published = release_notes::published(shell, &repo, &version)?;
    let notes = if let Some(notes) = notes {
        notes
    } else {
        let notes = release_notes::generate(
            shell,
            base,
            &repo,
            &version,
            &commit,
            published.first().map(|release| release.tag.as_str()),
        )?;
        let notes_path = out.join("notes.md");
        fs::write(&notes_path, &notes)?;
        shell.run(
            &args(&[
                "gh",
                "release",
                "create",
                &tag,
                "--draft",
                "--target",
                &commit,
                "--title",
                &format!("AgentInc {version}"),
                "--notes-file",
                &text(&notes_path),
            ]),
            None,
        )?;
        notes
    };
    // Notes are generated once, saved in the draft, then reused on every retry.
    fs::write(out.join("notes.md"), &notes)?;
    fs::write(
        out.join("changelog.md"),
        release_notes::history(shell, base, &repo, &version, &notes, &published)?,
    )?;

    let private = tempfile::tempdir()?;
    let private = private.path();
    fs::set_permissions(private, fs::Permissions::from_mode(0o700))?;
    let get = |name: &str| env.get(name).cloned().ok_or_else(|| anyhow!("{name}"));
    let encoded: String = get("APPLE_DEVELOPER_ID_P12_BASE64")?
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '='))
        .collect();
    fs::write(private.join("identity.p12"), STANDARD.decode(encoded)?)?;
    fs::write(
        private.join("password"),
        get("APPLE_DEVELOPER_ID_P12_PASSWORD")?,
    )?;
    fs::write(private.join("notary.p8"), get("NOTARY_KEY_P8")?)?;
    // rcodesign uses the modern Notary API and runs on Linux.
    shell.output(&args(&[
        "rcodesign",
        "encode-app-store-connect-api-key",
        &get("NOTARY_ISSUER_ID")?,
        &get("NOTARY_KEY_ID")?,
        &text(&private.join("notary.p8")),
        "--output-path",
        &text(&private.join("notary.json")),
    ]))?;
    let archive = out.join("AgentInc.tar.gz");
    let sign =
        |bundle: &Path, destination: &Path| sign_bundle(shell, env, bundle, destination, private);
    let mut tasks: Vec<Task<'_>> = vec![Box::new({
        let (bundle, archive, sign) = (bundle.clone(), archive.clone(), &sign);
        move || sign(&bundle, &archive)
    })];
    if candidate_arg.is_some() != newer_arg.is_some() {
        bail!("both upgrade fixture handoffs are required");
    }
    let mut fixture =
        |source: PathBuf, destination: PathBuf, commit: String, version: Option<String>| {
            let sign = &sign;
            tasks.push(Box::new(move || {
                sign_upgrade_fixture(&source, &destination, &commit, version.as_deref(), sign)
            }));
        };
    if let (Some(candidate), Some(newer)) = (&candidate_arg, &newer_arg) {
        fixture(
            in_base(candidate),
            out.join("upgrade-candidate.tar.gz"),
            commit.clone(),
            None,
        );
        fixture(
            in_base(newer),
            out.join("upgrade-newer.tar.gz"),
            commit.clone(),
            None,
        );
    }
    for prior in &priors {
        let file = name(prior);
        let prior_version = file.strip_prefix("upgrade-prior-").unwrap_or(&file);
        let prior_version = prior_version
            .strip_suffix("-unsigned.tar.gz")
            .unwrap_or(prior_version)
            .to_string();
        let prior_commit = rev_parse(shell, &format!("v{prior_version}^{{commit}}"))?;
        fixture(
            in_base(prior),
            out.join(format!("upgrade-prior-{prior_version}.tar.gz")),
            prior_commit,
            Some(prior_version),
        );
    }
    shell.sign_all(
        tasks,
        Box::new(|| {
            shell.run(
                &args(&[
                    "cargo",
                    "build",
                    "--locked",
                    "--profile",
                    "ci",
                    "-p",
                    "ainc-release",
                    "--bin",
                    "ainc-release-manifest",
                ]),
                None,
            )
        }),
    )?;
    let metadata = json_output(
        shell,
        &["cargo", "metadata", "--no-deps", "--format-version=1"],
    )?;
    let target = key(&metadata, "target_directory")?
        .as_str()
        .ok_or_else(|| anyhow!("target_directory is not a string"))?;
    let manifest_tool = Path::new(target).join("ci/ainc-release-manifest");
    let mut tool_env = env.clone();
    tool_env.remove("AINC_RELEASE_TEST_KEY");
    if test {
        tool_env.insert("AINC_RELEASE_TEST_KEY".into(), "1".into());
        // Throwaway key, never used by the production channel.
        let pem = text(&private.join("update.pem"));
        shell.output(&args(&[
            "openssl",
            "genpkey",
            "-algorithm",
            "ED25519",
            "-out",
            &pem,
        ]))?;
        tool_env.insert(
            "UPDATE_SIGNING_KEY_ED25519_PEM".into(),
            read_to_string(&private.join("update.pem"))?,
        );
        let der = shell.output_bytes(&args(&[
            "openssl", "pkey", "-in", &pem, "-pubout", "-outform", "DER",
        ]))?;
        let public = &der[der.len().saturating_sub(32)..];
        fs::write(
            out.join("test-public-key.txt"),
            format!("{}\n", STANDARD.encode(public)),
        )?;
    }
    let url = format!("https://github.com/{repo}/releases/download/{tag}/AgentInc.tar.gz");
    shell.run(
        &[
            text(&manifest_tool),
            text(&bundle.join("Contents/Resources/release.json")),
            text(&archive),
            text(&out.join("notes.md")),
            text(&out.join("changelog.md")),
            url,
            text(&out.join("feed.json")),
        ],
        Some(&tool_env),
    )?;
    // Staging work overlaps CI, but assets still cannot be uploaded (or published)
    // until the complete exact-commit workflow succeeds.
    shell.wait_for_ci(&repo, &commit)?;
    let mut upload = args(&["gh", "release", "upload", &tag]);
    for asset in ["AgentInc.tar.gz", "feed.json", "notes.md", "changelog.md"] {
        upload.push(text(&out.join(asset)));
    }
    if test {
        upload.push(text(&out.join("test-public-key.txt")));
    }
    upload.push("--clobber".into());
    shell.run(&upload, None)?;
    println!(
        "Draft staged {tag}; Sparkle signing and native gates are required before publication"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::{Barrier, Mutex};

    const COMMIT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    #[test]
    fn four_bundles_and_manifest_build_overlap_without_timers() {
        let ready = Barrier::new(5);
        let completed = Mutex::new(Vec::new());
        let task = |name: String| -> Task<'_> {
            let (ready, completed) = (&ready, &completed);
            Box::new(move || {
                ready.wait();
                completed.lock().unwrap().push(name);
                Ok(())
            })
        };
        let tasks = (0..4).map(|n| task(n.to_string())).collect();
        sign_all(tasks, task("manifest".into())).unwrap();
        let mut done = completed.into_inner().unwrap();
        done.sort();
        assert_eq!(done, ["0", "1", "2", "3", "manifest"]);
    }

    #[test]
    fn every_fixture_and_manifest_failure_propagates() {
        let ok = || -> Task<'static> { Box::new(|| Ok(())) };
        let fail = || -> Task<'static> { Box::new(|| bail!("failure")) };
        let error = sign_all(vec![ok(), fail()], ok()).unwrap_err();
        assert!(error.to_string().contains("failure"));
        let error = sign_all(vec![ok()], fail()).unwrap_err();
        assert!(error.to_string().contains("failure"));
    }

    /// Records every command; fails the one at `fail_at`.
    #[derive(Default)]
    struct Recorder {
        calls: Mutex<Vec<Vec<String>>>,
        fail_at: Option<usize>,
        outputs: Option<fn(&[String]) -> String>,
        draft: Option<String>,
        ci_error: bool,
    }

    impl Shell for Recorder {
        fn output(&self, args: &[String]) -> Result<String> {
            Ok(self.outputs.map(|f| f(args)).unwrap_or_default())
        }
        fn output_bytes(&self, _: &[String]) -> Result<Vec<u8>> {
            Ok(Vec::new())
        }
        fn run(&self, args: &[String], _: Option<&HashMap<String, String>>) -> Result<()> {
            let mut calls = self.calls.lock().unwrap();
            calls.push(args.to_vec());
            if self.fail_at == Some(calls.len() - 1) {
                bail!("notary");
            }
            Ok(())
        }
        fn probe(&self, _: &[String]) -> Result<(i32, String)> {
            Ok(self.draft.as_ref().map_or((1, String::new()), |body| {
                (
                    0,
                    json!({"targetCommitish": COMMIT, "isDraft": true, "body": body}).to_string(),
                )
            }))
        }
        fn sign_all(&self, _: Vec<Task<'_>>, _: Task<'_>) -> Result<()> {
            self.calls.lock().unwrap().push(vec!["sign_all".into()]);
            Ok(())
        }
        fn wait_for_ci(&self, _: &str, _: &str) -> Result<()> {
            if self.ci_error {
                bail!("CI failed");
            }
            Ok(())
        }
    }

    #[test]
    fn notary_wait_and_staple_are_still_required() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();
        let bundle = root.join("AgentInc.app");
        fs::create_dir(&bundle).unwrap();
        let env = HashMap::from([("APPLE_TEAM_ID".to_string(), "team".to_string())]);
        let shell = Recorder::default();
        sign_bundle(&shell, &env, &bundle, &root.join("signed.tar.gz"), root).unwrap();
        let calls = shell.calls.lock().unwrap();
        assert_eq!(calls[1][..2], ["rcodesign", "notary-submit"]);
        assert!(calls[1].contains(&"--wait".to_string()));
        assert!(calls[1].contains(&"--staple".to_string()));
        assert!(root.join("signed.tar.gz").is_file());
        drop(calls);
        let shell = Recorder {
            fail_at: Some(1),
            ..Default::default()
        };
        assert!(sign_bundle(&shell, &env, &bundle, &root.join("failed.tar.gz"), root).is_err());
        assert!(!root.join("failed.tar.gz").exists());
    }

    #[test]
    fn distribution_cannot_publish_before_sparkle_gates() {
        let root = tempfile::tempdir().unwrap();
        let shell = Recorder::default();
        let error = main(
            &args(&["--commit", COMMIT]),
            &HashMap::new(),
            root.path(),
            &shell,
        )
        .unwrap_err();
        assert!(error.to_string().contains("must use --stage"));
        assert!(shell.calls.lock().unwrap().is_empty());
    }

    fn fixture_archive(root: &Path, identity: &Value) -> PathBuf {
        let source = root.join("fixture.tar.gz");
        fs::write(root.join("handoff.json"), identity.to_string()).unwrap();
        let app = root.join("AgentInc.app");
        create_tar_gz(
            &source,
            6,
            &[
                (&app, "AgentInc.app"),
                (&root.join("handoff.json"), "handoff.json"),
            ],
        )
        .unwrap();
        source
    }

    #[test]
    fn fixture_identity_and_inventory_are_checked_before_signing() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();
        let app = root.join("AgentInc.app");
        fs::create_dir(&app).unwrap();
        fs::write(app.join("binary"), b"fixture").unwrap();
        let digest = hex_sha(b"fixture");
        let valid = json!({
            "commit": COMMIT, "version": "0.3.5", "upgrade_test": true,
            "files": {"binary": digest},
        });
        let overrides = [
            json!({}),
            json!({"commit": "b".repeat(40)}),
            json!({"version": "0.3.3"}),
            json!({"upgrade_test": false}),
            json!({"files": {}}),
        ];
        for (index, over) in overrides.iter().enumerate() {
            let mut identity = valid.clone();
            for (key, value) in over.as_object().unwrap() {
                identity[key] = value.clone();
            }
            let source = fixture_archive(root, &identity);
            let signed = Mutex::new(0);
            let sign = |_: &Path, _: &Path| {
                *signed.lock().unwrap() += 1;
                Ok(())
            };
            let result = sign_upgrade_fixture(
                &source,
                &root.join("signed.tar.gz"),
                COMMIT,
                Some("0.3.5"),
                &sign,
            );
            if index == 0 {
                result.unwrap();
                assert_eq!(*signed.lock().unwrap(), 1);
            } else {
                assert!(result.is_err(), "{over}");
                assert_eq!(*signed.lock().unwrap(), 0, "{over}");
            }
        }
    }

    fn hex_sha(data: &[u8]) -> String {
        super::super::hex(&sha2::Sha256::digest(data))
    }

    use sha2::Digest;

    #[test]
    fn ci_failure_prevents_assets_upload_and_publication() {
        // Exercise main, not merely the gate helper: speculative signing can finish, but no
        // feed/archive becomes available before complete CI.
        let root = tempfile::tempdir().unwrap();
        let root = root.path();
        let app = root.join("AgentInc.app");
        fs::create_dir(&app).unwrap();
        let identity = json!({
            "commit": COMMIT, "version": "0.3.5", "upgrade_test": false, "files": {},
        });
        let source = fixture_archive(root, &identity);
        fn outputs(args: &[String]) -> String {
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            match args[..] {
                ["git", "rev-parse", ..] => COMMIT.to_string(),
                ["cargo", "metadata", ..] => json!({
                    "packages": [{"name": "ainc-release", "version": "0.3.5"}],
                    "target_directory": "/nonexistent/target",
                })
                .to_string(),
                ["git", "log", ..] => format!("{COMMIT}\tA direct commit"),
                ["gh", "api", ..] => "[[]]".to_string(),
                _ => String::new(),
            }
        }
        let shell = Recorder {
            outputs: Some(outputs),
            ci_error: true,
            ..Default::default()
        };
        let env: HashMap<String, String> = [
            ("GITHUB_REF", "refs/heads/main"),
            ("GITHUB_REPOSITORY", "owner/repo"),
            ("UPDATE_SIGNING_KEY_ED25519_PEM", "private"),
            ("APPLE_DEVELOPER_ID_P12_BASE64", "YQ=="),
            ("APPLE_DEVELOPER_ID_P12_PASSWORD", "password"),
            ("NOTARY_KEY_P8", "private"),
            ("NOTARY_ISSUER_ID", "issuer"),
            ("NOTARY_KEY_ID", "key"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        let raw = args(&["--commit", COMMIT, "--archive", &text(&source), "--stage"]);
        let error = main(&raw, &env, root, &shell).unwrap_err();
        assert!(error.to_string().contains("CI failed"));
        let notes = fs::read_to_string(root.join(".local/distribution/notes.md")).unwrap();
        assert!(notes.contains("A direct commit"));
        let history = fs::read_to_string(root.join(".local/distribution/changelog.md")).unwrap();
        assert_eq!(
            ainc_release::notes::parse_changelog(&history).unwrap()[0].notes,
            notes.strip_prefix("# AgentInc 0.3.5\n\n").unwrap().trim()
        );
        let calls = shell.calls.lock().unwrap();
        assert_eq!(calls.iter().filter(|c| *c == &["sign_all"]).count(), 1);
        assert!(
            !calls
                .iter()
                .any(|c| c.starts_with(&args(&["gh", "release", "upload"])))
        );
        assert!(
            !calls
                .iter()
                .any(|c| c.contains(&"--draft=false".to_string()))
        );
        drop(calls);

        // A retry must use the draft verbatim, even when the override and Git history changed.
        fs::create_dir_all(root.join("docs/releases")).unwrap();
        fs::write(root.join("docs/releases/0.3.5.md"), "Changed override").unwrap();
        let retry = Recorder {
            outputs: Some(outputs),
            draft: Some("Original draft **notes**".into()),
            ci_error: true,
            ..Default::default()
        };
        assert!(
            main(&raw, &env, root, &retry)
                .unwrap_err()
                .to_string()
                .contains("CI failed")
        );
        assert_eq!(
            fs::read_to_string(root.join(".local/distribution/notes.md")).unwrap(),
            "Original draft **notes**"
        );
        let history = fs::read_to_string(root.join(".local/distribution/changelog.md")).unwrap();
        assert_eq!(
            ainc_release::notes::parse_changelog(&history).unwrap()[0].notes,
            "Original draft **notes**"
        );
        assert!(
            !retry
                .calls
                .lock()
                .unwrap()
                .iter()
                .any(|call| call.starts_with(&args(&["gh", "release", "create"])))
        );
    }
}
