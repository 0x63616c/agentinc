//! `cargo xtask vendor-pilot-gpui SOURCE`: reproduce the pinned GPUI source plus the opt-in
//! pilot seam into `vendor/gpui` (no cache edits). Ported from the Python script of the same
//! name; same argument, same output, same failure on a wrong revision.
use anyhow::{Context, Result, ensure};
use std::{
    fs,
    path::Path,
    process::{Command, exit},
};
use toml::{Table, Value};

const REV: &str = "4c902c9db22a82f5f3a14c02442e7f60ec40d9c8";

pub fn cli(args: &[String], root: &Path) -> Result<()> {
    let result = match args {
        [source] => run(Path::new(source), root),
        _ => Err(anyhow::anyhow!(
            "usage: cargo xtask vendor-pilot-gpui SOURCE"
        )),
    };
    if let Err(error) = result {
        eprintln!("{error:#}");
        exit(1);
    }
    Ok(())
}

fn table<'a>(value: &'a Value, what: &str) -> Result<&'a Table> {
    value
        .as_table()
        .with_context(|| format!("{what} is not a table"))
}

fn run(source: &Path, root: &Path) -> Result<()> {
    let source = source.canonicalize().context("resolve source")?;
    let head = Command::new("git")
        .arg("-C")
        .arg(&source)
        .args(["rev-parse", "HEAD"])
        .output()?;
    ensure!(
        String::from_utf8_lossy(&head.stdout).trim() == REV,
        "source is not at pinned revision {REV}"
    );
    let dest = root.join("vendor/gpui");
    let workspace: Table = fs::read_to_string(source.join("Cargo.toml"))?.parse()?;
    let crate_dir = source.join("crates/gpui");
    let manifest: Table = fs::read_to_string(crate_dir.join("Cargo.toml"))?.parse()?;
    let manifest = rewrite_manifest(&workspace, manifest)?;

    fs::create_dir_all(&dest)?;
    for name in ["src", "resources"] {
        copy_dir(&crate_dir.join(name), &dest.join(name))?;
    }
    for name in ["build.rs", "README.md"] {
        fs::copy(crate_dir.join(name), dest.join(name))?;
    }
    fs::copy(source.join("LICENSE-APACHE"), dest.join("LICENSE-APACHE"))?;
    fs::write(dest.join("Cargo.toml"), emit(&manifest, &[]).join("\n"))?;
    let patch = root.join("vendor/gpui-pilot.patch");
    if patch.exists() {
        let status = Command::new("git")
            .args(["apply", "--directory=vendor/gpui"])
            .arg(&patch)
            .current_dir(root)
            .status()?;
        ensure!(status.success(), "git apply failed: {status}");
    }
    Ok(())
}

/// Recursive copy that merges into an existing directory, like `copytree(dirs_exist_ok=True)`.
fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Turn the upstream crate manifest into a standalone one: workspace-inherited fields and
/// dependencies are expanded, upstream-only tables dropped, and the `pilot` feature added.
pub fn rewrite_manifest(workspace: &Table, mut manifest: Table) -> Result<Table> {
    let ws = table(
        workspace.get("workspace").context("no [workspace]")?,
        "workspace",
    )?;
    let edition = ws
        .get("package")
        .and_then(|p| p.get("edition"))
        .context("workspace.package.edition")?
        .clone();
    let package = manifest
        .get_mut("package")
        .and_then(Value::as_table_mut)
        .context("no [package]")?;
    package.insert("edition".into(), edition);
    for key in ["publish", "autoexamples", "autotests", "autobenches"] {
        package.insert(key.into(), false.into());
    }

    let check_cfg = Table::from_iter([
        ("level".to_string(), Value::from("warn")),
        (
            "check-cfg".to_string(),
            Value::Array(vec!["cfg(rust_analyzer)".into()]),
        ),
    ]);
    let unexpected = Table::from_iter([("unexpected_cfgs".to_string(), Value::Table(check_cfg))]);
    manifest.insert(
        "lints".into(),
        Value::Table(Table::from_iter([(
            "rust".to_string(),
            Value::Table(unexpected),
        )])),
    );
    manifest.remove("example");
    manifest
        .get_mut("features")
        .and_then(Value::as_table_mut)
        .context("no [features]")?
        .insert("pilot".into(), Value::Array(vec![]));

    resolve(ws, &mut manifest)?;
    // Dependency tests/examples are not part of the application build.
    manifest.remove("dev-dependencies");
    if let Some(targets) = manifest.get_mut("target").and_then(Value::as_table_mut) {
        for (_, target) in targets.iter_mut() {
            if let Some(target) = target.as_table_mut() {
                target.remove("dev-dependencies");
            }
        }
    }
    Ok(manifest)
}

fn resolve(ws: &Table, table_: &mut Table) -> Result<()> {
    for (key, value) in table_.iter_mut() {
        let Some(inner) = value.as_table_mut() else {
            continue;
        };
        if key.ends_with("dependencies") {
            for (name, dep) in inner.iter_mut() {
                let Some(dep_table) = dep.as_table() else {
                    continue;
                };
                if dep_table.get("workspace").and_then(Value::as_bool) != Some(true) {
                    continue;
                }
                *dep = Value::Table(expand(ws, name, dep_table)?);
            }
        } else {
            resolve(ws, inner)?;
        }
    }
    Ok(())
}

fn expand(ws: &Table, name: &str, dep: &Table) -> Result<Table> {
    let base = ws
        .get("dependencies")
        .and_then(|d| d.get(name))
        .with_context(|| format!("workspace has no dependency {name}"))?;
    let mut base = match base {
        Value::String(version) => {
            Table::from_iter([("version".to_string(), version.clone().into())])
        }
        other => table(other, name)?.clone(),
    };
    let mut overrides = dep.clone();
    overrides.remove("workspace");
    if let Some(Value::Array(extra)) = overrides.get("features").cloned() {
        let mut features = match base.get("features") {
            Some(Value::Array(existing)) => existing.clone(),
            _ => vec![],
        };
        for feature in extra {
            if !features.contains(&feature) {
                features.push(feature);
            }
        }
        overrides.insert("features".into(), Value::Array(features));
    }
    for (key, value) in overrides {
        base.insert(key, value);
    }
    if base.remove("path").is_some() {
        base.insert("git".into(), "https://github.com/zed-industries/zed".into());
        base.insert("rev".into(), REV.into());
    }
    Ok(base)
}

/// Python `json.dumps` of a string (ASCII-only output).
fn json_str(text: &str) -> String {
    let mut out = String::from("\"");
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            ch if (' '..='~').contains(&ch) => out.push(ch),
            ch => {
                let mut units = [0u16; 2];
                for unit in ch.encode_utf16(&mut units) {
                    out.push_str(&format!("\\u{unit:04x}"));
                }
            }
        }
    }
    out.push('"');
    out
}

/// Python `json.dumps` of a TOML value (`", "` and `": "` separators).
fn json_dumps(value: &Value) -> String {
    match value {
        Value::String(s) => json_str(s),
        Value::Integer(i) => i.to_string(),
        Value::Float(f) => format!("{f:?}"),
        Value::Boolean(b) => b.to_string(),
        Value::Datetime(d) => json_str(&d.to_string()),
        Value::Array(items) => format!(
            "[{}]",
            items.iter().map(json_dumps).collect::<Vec<_>>().join(", ")
        ),
        Value::Table(t) => format!(
            "{{{}}}",
            t.iter()
                .map(|(k, v)| format!("{}: {}", json_str(k), json_dumps(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// Inline form: tables as `{ "k" = v }`, everything else as JSON.
fn literal(value: &Value) -> String {
    match value {
        Value::Table(t) => format!(
            "{{ {} }}",
            t.iter()
                .map(|(k, v)| format!("{} = {}", json_str(k), literal(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        other => json_dumps(other),
    }
}

/// The manifest as lines: scalars first, then nested tables under quoted dotted headers.
pub fn emit(table_: &Table, prefix: &[&str]) -> Vec<String> {
    let mut lines = Vec::new();
    if !prefix.is_empty() {
        let header: Vec<_> = prefix.iter().map(|p| json_str(p)).collect();
        lines.push(format!("[{}]", header.join(".")));
    }
    for (key, value) in table_ {
        if !value.is_table() {
            lines.push(format!("{} = {}", json_str(key), literal(value)));
        }
    }
    for (key, value) in table_ {
        if let Value::Table(inner) = value {
            let mut next = prefix.to_vec();
            next.push(key);
            lines.extend(emit(inner, &next));
        }
    }
    lines.push(String::new());
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Table {
        text.parse().unwrap()
    }

    #[test]
    fn workspace_dependencies_expand_with_merged_features() {
        let ws = parse(
            r#"
[workspace.package]
edition = "2024"
[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
log = "0.4"
util = { path = "crates/util" }
"#,
        );
        let manifest = parse(
            r#"
[package]
name = "gpui"
edition.workspace = true
[features]
default = []
[dependencies]
serde = { workspace = true, features = ["derive", "rc"] }
log.workspace = true
util.workspace = true
local = { version = "2" }
[dev-dependencies]
x = "1"
[target.'cfg(unix)'.dev-dependencies]
y = "1"
[target.'cfg(unix)'.dependencies]
log = { workspace = true, optional = true }
[[example]]
name = "e"
"#,
        );
        let out = rewrite_manifest(&ws, manifest).unwrap();
        let text = emit(&out, &[]).join("\n");
        assert!(text.contains("\"edition\" = \"2024\""));
        assert!(text.contains("\"publish\" = false"));
        assert!(text.contains("\"pilot\" = []"));
        let lines: Vec<_> = text.lines().collect();
        let section = |header: &str| {
            let at = lines.iter().position(|l| *l == header).expect(header);
            lines[at + 1..]
                .iter()
                .take_while(|l| !l.is_empty())
                .copied()
                .collect::<Vec<_>>()
        };
        assert_eq!(
            section("[\"dependencies\".\"serde\"]"),
            ["\"version\" = \"1\"", "\"features\" = [\"derive\", \"rc\"]"]
        );
        assert_eq!(
            section("[\"dependencies\".\"log\"]"),
            ["\"version\" = \"0.4\""]
        );
        assert_eq!(
            section("[\"dependencies\".\"util\"]"),
            [
                "\"git\" = \"https://github.com/zed-industries/zed\"".to_string(),
                format!("\"rev\" = \"{REV}\"")
            ]
        );
        assert_eq!(
            section("[\"target\".\"cfg(unix)\".\"dependencies\".\"log\"]"),
            ["\"version\" = \"0.4\"", "\"optional\" = true"]
        );
        assert_eq!(
            section("[\"dependencies\".\"local\"]"),
            ["\"version\" = \"2\""]
        );
        assert!(!text.contains("dev-dependencies"));
        assert!(!text.contains("\"example\""));
        assert!(text.contains("\"check-cfg\" = [\"cfg(rust_analyzer)\"]"));
    }

    #[test]
    fn missing_workspace_dependency_is_an_error() {
        let ws = parse("[workspace.package]\nedition = \"2024\"\n[workspace.dependencies]\n");
        let manifest =
            parse("[package]\nname = \"g\"\n[features]\n[dependencies]\nnope.workspace = true\n");
        assert!(rewrite_manifest(&ws, manifest).is_err());
    }

    #[test]
    fn emit_orders_scalars_before_tables_and_escapes_like_json() {
        let table = parse("[a]\nz = \"é\\\"\"\nsub.k = 1\ny = [\"p\", \"q\"]\n[b]\nn = true\n");
        let lines = emit(&table, &[]);
        assert_eq!(
            lines,
            vec![
                "[\"a\"]",
                "\"z\" = \"\\u00e9\\\"\"",
                "\"y\" = [\"p\", \"q\"]",
                "[\"a\".\"sub\"]",
                "\"k\" = 1",
                "",
                "",
                "[\"b\"]",
                "\"n\" = true",
                "",
                "",
            ]
        );
    }
}
