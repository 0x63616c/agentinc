# Personal distribution

Product version lives in `crates/ainc-release/Cargo.toml`; the SDK version is independent.
`ainc-release::identity` is the build-channel source of truth shared by the app and daemon.
Only `cargo xtask release` stamps `AINC_CHANNEL=production` (plus build ID and commit);
ordinary Cargo, Tilt and local bundle builds are development. The release app remains
`co.worldwidewebb.agentinc` in `~/Library/Application Support/Agentinc OS` to preserve
installed data. Development uses `co.worldwidewebb.agentinc.dev` and
`~/Library/Application Support/AgentInc Development`. The bundle script writes
`AgentInc Dev.app`; both bundle executables are named `AgentInc`, so macOS shows the
correct app menu and Dock name. The app's About panel and sidebar read the compiled
version/channel; development shows `VERSION-dev` and its commit.

Channel inventory: discovery file, owner token, daemon lock/log, managed Postgres
directory, Temporal SQLite and helper lock all live below the channel's support
directory. Session, Codex credentials and update preferences/cache live there too.
The daemon, Postgres and Temporal choose free loopback ports independently; Temporal
namespace and worker group have channel-specific names. The bundle identifier is
also the GPUI window app ID, separating LaunchServices identity and preferences.
Development does not check or install production updates. Explicit `AINC_*` profile
overrides remain for isolated tests and external development stacks; they can connect
to a chosen endpoint intentionally.
`ainc-release` owns request compatibility, the signed manifest, update preferences and
archive validation. ADRs 0008 and 0009 remain authoritative. This phase excludes OIDC,
membership and additional-user setup.

## Native build and Linux signing

GPUI depends on AppKit, Metal and the Apple SDK. Linux cannot build this checkout's
native macOS client. On a version change pushed to `main`, Distribution builds
the unsigned bundle on the repository's [macOS release runner](release-runner.md)
at the exact commit and passes it to the existing Linux signing job through
an Actions artifact. For a local or fallback build at that commit:

```
cargo xtask release
```

For local acceptance, `cargo xtask release --profile debug` produces the same complete
bundle without optimization. The handoff is `.local/release/COMMIT/unsigned.tar.gz`.
It includes the app, daemon, installer, runtime executables and a hashed file inventory.
Create a **draft** `build-COMMIT` release targeted at that commit, upload the archive,
and dispatch Distribution with `build=false` for that commit. `--upload` performs
this fallback handoff and dispatches the workflow on main. No build-input draft
is published.

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
`crates/ainc-release/src/lib.rs`; Firstmate provisioned it and the repository secret
on 2026-09-24. Acceptance jobs still use throwaway keys and stay draft.

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
An external configured database/runtime keeps its existing ownership model. A lifetime-pipe
helper owns the bundled services: daemon exit or SIGKILL closes the pipe, then the helper
flushes both stores. Its separate lock serializes immediate restart. PostgreSQL uses
fast shutdown to disconnect residual pool sockets while safely rolling back and checkpointing.
Companion startup diagnostics are retained in `daemon.log` beside discovery.
Native process startup and pre-exec launch clear inherited signal masks and reset
SIGCHLD, so launching from a UI dispatch thread cannot disable child reaping.
`runtime-smoke.py --blocked-signals` verifies readiness and reaping with SIGCHLD
blocked and ignored, including drain/restart and crash recovery.

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

Validated on code commit `024756ae1a94f63e239bd98640bb3a2dc24dac40`:

- Workspace fmt, Clippy and tests passed. Tests used a fresh bundled database.
- [Linux CI](https://github.com/0x63616c/agentinc/actions/runs/35998889853) passed.
- [Linux Distribution](https://github.com/0x63616c/agentinc/actions/runs/35998882862)
  signed, notarized and stapled the complete native debug-profile bundle. Apple
  submission `b85e4441-7829-41e5-8768-1cdc998cc7ae` was Accepted.
- `spctl --assess --type execute -vv` accepted it as Notarized Developer ID,
  Calum Webb (`X9E4HG27NK`); `xcrun stapler validate` succeeded.
- The ignored `notarized_install` acceptance test authenticated the local draft
  manifest, extracted and installed the actual notarized bundle, and rejected a
  corrupt archive before extraction.
- Fresh bundled startup, state-preserving drain/restart and immediate daemon
  SIGKILL recovery passed without external services.
- 38 real Metal frames and both GPUI pilot tests passed. Pilot also exercised
  automatic-check and interval settings; see
  [native settings](../crates/ainc-mac/docs/verification/phase5/update-settings.png).
- Both the manifest tool and publish script refused the absent production secret.

The full ignored installer acceptance test also passed: it authenticated the draft,
drained the daemon, replaced the bundle, launched the real signed native app,
confirmed the new daemon's version/readiness and retired the backup. The test uses
`0.0.0` as its fixture predecessor version; both installed payloads are actual
notarized artifacts. Production uses its compiled current version and embedded key.
No test-key override is exposed by the shipping installer.

An earlier acceptance failure exposed inherited SIGCHLD state in native launches.
The process-boundary reset fixes this; the blocked/ignored-SIGCHLD regression,
workspace tests, GPUI pilot and full signed installer now pass. The missing-secret
refusal was exercised with the environment variable explicitly absent, even after
Firstmate provisioned the production public key and repository secret.

All distribution artifacts remain drafts; no public release or tag was created.
Acceptance output is recorded in
[the verification evidence](../crates/ainc-mac/docs/verification/phase5/acceptance.txt).
The signed code commit above precedes the final documentation-only evidence commit.

Upstream references: [rcodesign](https://gregoryszorc.com/docs/apple-codesign/stable/apple_codesign_rcodesign.html),
[Temporal local server](https://github.com/temporalio/documentation/blob/main/docs/cli/command-reference/server.mdx).
