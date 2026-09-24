use ainc_release::{Manifest, SignedManifest};
use anyhow::{Context, Result, ensure};
use ed25519_dalek::{SigningKey, pkcs8::DecodePrivateKey};
use sha2::{Digest, Sha256};
use std::{env, fs};
fn main() -> Result<()> {
    // Check this before touching artifacts. CI must fail clearly, never silently
    // publish an unsigned feed when the production key is unavailable.
    let pem = env::var("UPDATE_SIGNING_KEY_ED25519_PEM")
        .context("publish refused: UPDATE_SIGNING_KEY_ED25519_PEM is missing")?;
    ensure!(
        !pem.trim().is_empty(),
        "publish refused: UPDATE_SIGNING_KEY_ED25519_PEM is missing"
    );
    let key = SigningKey::from_pkcs8_pem(&pem).context("invalid Ed25519 signing key")?;
    let args: Vec<_> = env::args().skip(1).collect();
    ensure!(
        args.len() == 6,
        "usage: manifest IDENTITY ARCHIVE NOTES CHANGELOG ARCHIVE_URL OUTPUT"
    );
    let identity: serde_json::Value = serde_json::from_slice(&fs::read(&args[0])?)?;
    let text = |name: &str| {
        identity[name]
            .as_str()
            .with_context(|| format!("missing release {name}"))
    };
    let bytes = fs::read(&args[1])?;
    let manifest = Manifest {
        version: text("version")?.parse()?,
        daemon_version: text("version")?.parse()?,
        minimum_client: text("minimum_client")?.parse()?,
        build: text("build")?.into(),
        commit: text("commit")?.into(),
        architecture: text("architecture")?.into(),
        api: identity["api"].as_u64().context("API")?.try_into()?,
        schema: 1,
        archive_url: args[4].clone(),
        archive_sha256: format!("{:x}", Sha256::digest(&bytes)),
        archive_bytes: bytes.len() as u64,
        notes: fs::read_to_string(&args[2])?,
        changelog: fs::read_to_string(&args[3])?,
    };
    let signed = SignedManifest::sign(&manifest, &key)?;
    use base64::Engine;
    signed.verify(
        &base64::engine::general_purpose::STANDARD.encode(key.verifying_key().to_bytes()),
    )?;
    fs::write(&args[5], serde_json::to_vec_pretty(&signed)?)?;
    Ok(())
}
