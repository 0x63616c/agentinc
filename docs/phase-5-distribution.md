# Personal distribution

Product version lives in `crates/ainc-release/Cargo.toml`; the SDK version is independent.
`ainc-release` owns request compatibility, the signed manifest, update preferences and
archive validation. ADRs 0008 and 0009 remain authoritative. This phase excludes OIDC,
membership and additional-user setup.

## Native build and Linux signing

GPUI depends on AppKit, Metal and the Apple SDK. Linux cannot build this checkout's
native macOS client with the installed Linux toolchain. Keep CI Linux-only and prepare
the native artifact on a Mac at the exact release commit:

```
cargo xtask release
```

For local acceptance, `cargo xtask release --profile debug` produces the same complete
bundle without optimization. The handoff is `.local/release/COMMIT/unsigned.tar.gz`.
It includes the app, daemon, installer, runtime executables and a hashed file inventory.
Create a **draft** `build-COMMIT` release targeted at that commit, upload the archive,
and run Distribution for that commit. No build-input draft is published.

Distribution uses rcodesign 0.29.0 on Ubuntu to sign all nested code with hardened
runtime, submit to Apple's Notary API, staple, archive and Ed25519-sign the manifest.
The manifest binds the archive digest/size, product and daemon versions, API window,
architecture, build ID and commit. A published version cannot be reassigned to another
commit. Release notes are generated once and stored in the draft; retries reuse them.
The same notes are in the manifest and downloadable notes/changelog artifacts.

Only the main branch may publish. The branch acceptance job and `test=true` dispatch
leave signed artifacts in a draft and use a throwaway Ed25519 key. The public feed is
`https://github.com/0x63616c/agentinc/releases/latest/download/feed.json`.
Production publishing refuses if `UPDATE_SIGNING_KEY_ED25519_PEM` is missing or empty.
The production public key is the single `UPDATE_PUBLIC_KEY` constant in
`crates/ainc-release/src/lib.rs`; it is deliberately empty until provisioned.

Apple credentials are read only in the signing job, written to a temporary private
directory, and never passed as secret values on command lines. Required secrets:
`APPLE_DEVELOPER_ID_P12_BASE64`, `APPLE_DEVELOPER_ID_P12_PASSWORD`, `APPLE_TEAM_ID`,
`NOTARY_KEY_ID`, `NOTARY_ISSUER_ID`, `NOTARY_KEY_P8`.

## Personal runtime

The Apple Silicon bundle (macOS 15 or later) includes portable Postgres 16.15, the Temporal CLI's persistent local server and the
Codex executable. Postgres comes from the pinned Theseus 16.15.0 archive and SHA-256 in
`scripts/release/prepare.py`. Temporal and Codex are taken from the build machine and
recorded by hash in the handoff. Required portable libraries travel with Postgres;
development headers and test executables are excluded. Every Mach-O is audited with
`otool -L` before packaging: only system paths, `@loader_path` and `@rpath` are allowed.
A fresh Mac does not need Homebrew, Docker, Tilt or a manual database setup.

The daemon discovery lock owns startup. Without a configured `DATABASE_URL`, `aincd`
initializes its private runtime directory beside discovery, protected mode 0700.
Postgres uses a random persisted password, SCRAM authentication and loopback only;
Temporal uses a persistent SQLite file, a stable namespace/queue and no web UI.
An external configured database/runtime keeps its existing ownership model.

This is a personal, single-machine runtime, not a hosted or multi-user deployment.
Temporal's local server is explicitly a development server; this choice is limited
to the accepted single-user scope. Its loopback API is accessible to local processes.
Data stays outside the app bundle and survives replacement. Postgres major-version
changes are refused until an explicit migration exists. Automatic down migrations
are never run. Back up the profile with the daemon stopped for a consistent copy.

## Update protocol

Every generated client request sends the product version and API. Product routes
return a typed HTTP 426 compatibility failure before a handler runs; every response
carries the server version. The generated client reports `Update to continue` centrally.

Update checks and settings belong to the native app, independent of backend readiness.
The update window exposes notes, changelog, download progress, Install and Relaunch,
Remind Me Later and Skip This Version. Manual checks ignore skipped versions.
Downloaded archives are authenticated before installation. The signed helper repeats
verification, validates the Apple team/bundle identity and Gatekeeper assessment,
waits for the UI to close, asks the owned daemon to drain, waits for its discovery lock,
and replaces the complete app/daemon bundle. It never signals a process from a PID file.

## Acceptance status

Implementation is in progress. Signature/tamper, compatibility, preferences and failed
replacement recovery tests pass; workspace Clippy passed at the initial checkpoint.
The missing-production-secret refusal has been exercised locally. Native bundled
runtime, Apple notarization, rendered/pilot acceptance and Linux CI are pending.
No public release or tag has been created for acceptance.

Upstream references: [rcodesign](https://gregoryszorc.com/docs/apple-codesign/stable/apple_codesign_rcodesign.html),
[Temporal local server](https://github.com/temporalio/documentation/blob/main/docs/cli/command-reference/server.mdx).
