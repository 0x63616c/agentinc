//! `cargo xtask generate`: rewrite `api/openapi-3.0.json`, `crates/ainc-client/src/generated.rs`
//! and `crates/ainc-cli/src/generated.rs` from the daemon's OpenAPI document. Its own crate so
//! xtask does not link the daemon (and Temporal) to run checks or releases.
use anyhow::{Context, Result, bail};
use progenitor::{GenerationSettings, Generator, InterfaceStyle};
use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn generate(root: &Path, check: bool) -> Result<()> {
    let mut spec = ainc_daemon::openapi();
    // This API uses the common 3.0/3.1 schema subset. Validate that assumption
    // before changing the dialect marker; never silently reinterpret nullability.
    fn compatible(value: &serde_json::Value) -> Result<()> {
        match value {
            serde_json::Value::Object(map) => {
                for (key, child) in map {
                    if [
                        "const",
                        "if",
                        "then",
                        "else",
                        "unevaluatedProperties",
                        "$schema",
                    ]
                    .contains(&key.as_str())
                    {
                        bail!("OpenAPI 3.1-only schema key: {key}");
                    }
                    if key == "type" && child.is_array() {
                        bail!("OpenAPI 3.1 union type needs conversion");
                    }
                    compatible(child)?;
                }
            }
            serde_json::Value::Array(items) => {
                for child in items {
                    compatible(child)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    // Utoipa represents nullable primitives as a JSON Schema type array.
    // Convert exactly that shape to the OpenAPI 3.0 nullable keyword.
    fn nullable(value: &mut serde_json::Value) -> Result<()> {
        match value {
            serde_json::Value::Object(map) => {
                if let Some(serde_json::Value::Array(types)) = map.get("type") {
                    let non_null: Vec<_> =
                        types.iter().filter(|v| **v != "null").cloned().collect();
                    if types.len() != 2 || non_null.len() != 1 {
                        bail!("unsupported schema type union");
                    }
                    map.insert("type".into(), non_null[0].clone());
                    map.insert("nullable".into(), true.into());
                }
                for child in map.values_mut() {
                    nullable(child)?;
                }
            }
            serde_json::Value::Array(items) => {
                for child in items {
                    nullable(child)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    nullable(&mut spec)?;
    compatible(&spec)?;
    if let Some(license) = spec["info"]["license"].as_object_mut() {
        license.remove("identifier"); // OpenAPI 3.1 field; name remains for 3.0.3.
    }
    spec["openapi"] = "3.0.3".into();
    // Workspace feature unification can enable serde_json's preserve_order.
    // Generated output must stay canonical regardless of that dependency feature.
    spec.sort_all_objects();
    let spec_text = format!("{}\n", serde_json::to_string_pretty(&spec)?);
    let parsed: openapiv3::OpenAPI = serde_json::from_value(spec)?;
    let mut settings = GenerationSettings::default();
    settings
        .with_interface(InterfaceStyle::Builder)
        .with_derive("schemars::JsonSchema")
        .with_pre_hook_async(syn::parse_quote!(crate::client_header))
        .with_post_hook_async(syn::parse_quote!(crate::server_compatibility));
    let mut generator = Generator::new(&settings);
    fn format(source: String) -> Result<String> {
        // Progenitor emits block-doc examples whose fences rustfmt indents into
        // invalid doctests. Keep them as comments in the checked-in output.
        let source = source.replace("/**", "/*");
        let mut rustfmt = Command::new("rustfmt")
            .args(["--edition", "2024", "--emit", "stdout"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;
        rustfmt
            .stdin
            .take()
            .context("rustfmt stdin")?
            .write_all(source.as_bytes())?;
        let output = rustfmt.wait_with_output()?;
        if !output.status.success() {
            bail!("rustfmt failed");
        }
        Ok(String::from_utf8(output.stdout)?)
    }
    let client = format(prettyplease::unparse(&syn::parse2(
        generator.generate_tokens(&parsed)?,
    )?))?;
    let cli = format(prettyplease::unparse(&syn::parse2(
        generator.cli(&parsed, "ainc_client")?,
    )?))?;
    for (path, content) in [
        (root.join("api/openapi-3.0.json"), spec_text),
        (root.join("crates/ainc-client/src/generated.rs"), client),
        (root.join("crates/ainc-cli/src/generated.rs"), cli),
    ] {
        if check {
            if fs::read_to_string(&path).ok().as_deref() != Some(&content) {
                bail!("generated file differs: {}", path.display());
            }
        } else {
            fs::create_dir_all(path.parent().context("generated file parent")?)?;
            fs::write(path, content)?;
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let check = env::args().nth(1).as_deref() == Some("--check");
    generate(&root(), check)
}

#[cfg(test)]
mod tests {
    /// Runs in `cargo test --workspace` against the same compiled daemon, so a stale
    /// `api/` or generated client fails the test run without a second build.
    #[test]
    fn generated_api_and_clients_are_current() {
        super::generate(&super::root(), true).unwrap();
    }
}
