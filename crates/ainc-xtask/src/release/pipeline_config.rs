//! Regressions for the required-status and rolling-cache contracts of the CI and release
//! workflows. Test-only: it reads `.github/workflows` as data.
use regex::Regex;
use serde_json::{Value, json};
use std::{collections::HashMap, path::Path, process::Command};

type Context = HashMap<&'static str, Value>;

fn workflow(name: &str) -> Value {
    // Psych ships with Ruby on macOS and the hosted Ubuntu runner. Parse the declarative
    // contract, so comments/formatting cannot satisfy these checks.
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(format!(".github/workflows/{name}.yml"));
    let output = Command::new("ruby")
        .args([
            "-ryaml",
            "-rjson",
            "-e",
            "puts JSON.generate(YAML.safe_load(File.read(ARGV[0])))",
        ])
        .arg(path)
        .output()
        .expect("ruby is required to parse the workflows");
    assert!(output.status.success(), "ruby failed to parse {name}.yml");
    serde_json::from_slice(&output.stdout).unwrap()
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::String(text) => !text.is_empty(),
        _ => true,
    }
}

fn expression(value: &str, context: &Context) -> Value {
    if value.contains("||") {
        return value
            .split("||")
            .map(|part| expression(part.trim(), context))
            .find(truthy)
            .unwrap_or_else(|| json!(""));
    }
    if value.starts_with("hashFiles(") {
        return json!("unchanged-manifest-hash");
    }
    if value.len() >= 2 && value.starts_with('\'') && value.ends_with('\'') {
        return json!(&value[1..value.len() - 1]);
    }
    context
        .get(value)
        .unwrap_or_else(|| panic!("unknown expression {value}"))
        .clone()
}

fn interpolate(value: &str, context: &Context) -> String {
    let pattern = Regex::new(r"\$\{\{\s*(.*?)\s*\}\}").unwrap();
    pattern
        .replace_all(value, |captures: &regex::Captures| {
            match expression(&captures[1], context) {
                Value::String(text) => text,
                other => other.to_string(),
            }
        })
        .into_owned()
}

fn condition(value: &str, context: &Context) -> bool {
    let comparison = Regex::new(r"^(.+?)\s*(==|!=)\s*(.+)$").unwrap();
    for term in value.split("&&").map(str::trim) {
        if term == "success()" {
            if !truthy(&context["success"]) {
                return false;
            }
            continue;
        }
        let captures = comparison.captures(term).expect("unsupported condition");
        let equal =
            expression(captures[1].trim(), context) == expression(captures[3].trim(), context);
        if equal != (&captures[2] == "==") {
            return false;
        }
    }
    true
}

fn only(job: &Value, predicate: impl Fn(&Value) -> bool) -> &Value {
    let mut found = job["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| predicate(s));
    let step = found.next().expect("no matching step");
    assert!(found.next().is_none(), "more than one matching step");
    step
}

fn uses(name: &'static str) -> impl Fn(&Value) -> bool {
    move |step| step["uses"] == name
}

#[test]
fn upgrade_fixture_environment_satisfies_the_real_release_build_guard() {
    let release = workflow("release");
    let fixture = only(&release["jobs"]["native"], |step| {
        step["name"] == "Build isolated upgrade fixtures"
    });
    let channel = fixture["env"]["AINC_CHANNEL"].as_str().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let executable = directory.path().join("release-build-guard");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ainc-release/build.rs");
    assert!(
        Command::new("rustc")
            .arg("--edition=2024")
            .arg(source)
            .arg("-o")
            .arg(&executable)
            .status()
            .unwrap()
            .success()
    );
    // Execute the same guard Cargo runs, with the workflow's declared channel.
    for version in [None, Some("0.5.1")] {
        let mut command = Command::new(&executable);
        command
            .env("AINC_CHANNEL", channel)
            .env("AINC_UPGRADE_TEST_PUBLIC_KEY", "fixture-key")
            .env_remove("AINC_UPGRADE_TEST_VERSION")
            .env("AINC_COMMIT", "a".repeat(40));
        if let Some(version) = version {
            command.env("AINC_UPGRADE_TEST_VERSION", version);
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = String::from_utf8(output.stdout).unwrap();
        assert!(
            output
                .lines()
                .any(|line| line == "cargo:rustc-cfg=ainc_production")
        );
        assert!(
            output
                .lines()
                .any(|line| line == "cargo:rustc-cfg=ainc_upgrade_test")
        );
    }
    assert!(
        !Command::new(executable)
            .env_remove("AINC_CHANNEL")
            .env("AINC_UPGRADE_TEST_PUBLIC_KEY", "fixture-key")
            .env_remove("AINC_UPGRADE_TEST_VERSION")
            .output()
            .unwrap()
            .status
            .success()
    );
}

#[test]
fn historical_publication_uses_the_workflow_revision_gate() {
    let release = workflow("release");
    let publish = &release["jobs"]["publish"];
    let checkout = only(publish, uses("actions/checkout@v4"));
    assert_eq!(checkout["with"]["ref"], "${{ github.workflow_sha }}");
    let gate = only(publish, |step| {
        step["name"] == "Recheck exact-commit CI before publication"
    });
    assert_eq!(gate["env"]["COMMIT"], "${{ inputs.commit || github.sha }}");
}

#[test]
fn required_rust_aggregate_only_accepts_all_success() {
    let ci = workflow("ci");
    let aggregate = &ci["jobs"]["rust"];
    let mut needs: Vec<&str> = aggregate["needs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_str().unwrap())
        .collect();
    needs.sort();
    assert_eq!(needs, ["workspace"]);
    assert_eq!(aggregate["if"], "always()");
    let steps = aggregate["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 1);
    let step = &steps[0];
    assert_eq!(
        step["env"],
        json!({
            "WORKSPACE": "${{ needs.workspace.result }}",
        })
    );
    let command = step["run"].as_str().unwrap();
    for workspace in ["success", "failure", "cancelled", "skipped"] {
        let status = Command::new("bash")
            .args(["-c", command])
            .env("WORKSPACE", workspace)
            .status()
            .unwrap();
        assert_eq!(status.success(), workspace == "success", "{workspace}");
    }
}

#[test]
fn ci_dependency_cache_is_written_only_from_main() {
    let ci = workflow("ci");
    let cache = only(&ci["jobs"]["workspace"], uses("Swatinem/rust-cache@v2"));
    assert_eq!(cache["with"]["key"], "${{ matrix.check }}");
    assert_eq!(
        cache["with"]["save-if"],
        "${{ github.ref == 'refs/heads/main' }}"
    );
}

#[test]
fn cache_primary_keys_roll_but_restore_prefixes_do_not() {
    for (name, job_name) in [("release", "distribute")] {
        let workflow = workflow(name);
        let job = &workflow["jobs"][job_name];
        let restore = only(job, uses("actions/cache/restore@v4"));
        let save = only(job, uses("actions/cache/save@v4"));
        let mut context: Context = HashMap::from([
            ("runner.os", json!("Linux")),
            ("runner.arch", json!("X64")),
            ("matrix.check", json!("test")),
            ("inputs.commit", json!("")),
            ("github.ref", json!("refs/heads/main")),
            ("github.event_name", json!("push")),
            ("success", json!(true)),
            ("steps.cache.outputs.cache-hit", json!("false")),
        ]);
        let (mut keys, mut prefixes) = (Vec::new(), Vec::new());
        for commit in ["a".repeat(40), "b".repeat(40)] {
            context.insert("github.sha", json!(commit));
            keys.push(interpolate(
                restore["with"]["key"].as_str().unwrap(),
                &context,
            ));
            prefixes.push(interpolate(
                restore["with"]["restore-keys"].as_str().unwrap(),
                &context,
            ));
            context.insert(
                "steps.cache.outputs.cache-primary-key",
                json!(keys.last().unwrap()),
            );
            assert_eq!(
                &interpolate(save["with"]["key"].as_str().unwrap(), &context),
                keys.last().unwrap()
            );
        }
        assert_ne!(keys[0], keys[1]);
        assert_eq!(prefixes[0], prefixes[1]);
        let save_if = save["if"].as_str().unwrap();
        assert!(condition(save_if, &context));
        let overrides: [(&'static str, Value); 3] = [
            ("github.ref", json!("refs/pull/1/merge")),
            ("success", json!(false)),
            ("steps.cache.outputs.cache-hit", json!("true")),
        ];
        for (key, value) in overrides {
            let mut changed = context.clone();
            changed.insert(key, value);
            assert!(!condition(save_if, &changed), "{key}");
        }
        if name == "release" {
            let mut changed = context.clone();
            changed.insert("github.event_name", json!("workflow_dispatch"));
            assert!(!condition(save_if, &changed));
        }
    }
}
