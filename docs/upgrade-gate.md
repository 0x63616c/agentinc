# Native upgrade release gate

Distribution stages a signed and notarized release as a draft. It publishes only
after the macOS `upgrade` job succeeds. A branch dispatch with `test=true`
exercises the same gate and keeps every artifact in a draft.

Visible AppKit updater fixtures are ignored by default, so `just test` and ordinary
agent test runs do not open updater windows on the user's desktop. Distribution
runs them on `homelab-mini` with `AINC_NATIVE_UPDATE_TESTS=1 cargo test --locked
-p ainc-mac --test update_window_ownership -- --ignored --test-threads=1`.
The explicit environment opt-in is required even with `--ignored`. Live Sparkle
failure/retry testing must also use the dedicated Mac, not the user's active desktop.

The native build job makes three handoffs from the exact commit: the production
candidate, an upgrade-test candidate, and the same upgrade-test code reporting
the next patch version and a strictly newer `CFBundleVersion`. Sparkle compares build
numbers, so changing the display version alone is insufficient. The test builds
have the production bundle identity but
a separate compiled Ed25519 public key. The Mac generates the matching private
key for that workflow run. Linux signs and notarizes all three apps. The test
key travels only in the one-day Actions handoff; it is never a release asset.
The shipping handoff carries `upgrade_test: false`, which `cargo xtask release-distribute`
requires before staging it. Test feed overrides are compiled out of shipping builds.
Signature, archive hash, Developer ID, and Gatekeeper checks remain mandatory in
both builds. Fixtures carry a defaults domain derived from their test key before
signing, isolating Sparkle preferences from the installed application. Relaunch uses
a private one-shot environment record; the gate never edits a signed bundle's plist
to insert profile paths or feed URLs.

`cargo xtask release-upgrade-gate` installs the signed test candidate in an
isolated profile and serves the signed newer build from a local feed. The manual
pass triggers Check for Updates, displays the existing AppKit offer and clicks its
Install Update button. The automatic pass downloads the update and clicks the same
button when ready. Sparkle drives downloading, verification and installation through
our custom user driver. Each pass must exit normally, replace the app bundle, and
relaunch with a compatible, ready daemon. The resulting bundle must pass Developer ID
and Gatekeeper checks again. The test hook calls the real AppKit button's
`performClick`; the old window ownership bug crashes that call.

The Sparkle candidate also runs a real binary-delta upgrade and a signed, invalid
delta that must fall back to the full archive. The local HTTP server records completed
payload responses: a delta pass must fetch the patch without fetching the full app,
and fallback must fetch the patch before the full app. The gate reports actual patch
and archive byte sizes. Merely producing a patch or successfully installing a full
archive is not evidence that delta updating works.

A download-only crash pass waits for a real cached delta and a ready current daemon,
then sends SIGKILL to the UI. No Sparkle extraction/preparation callback may have
occurred. The original signed bundle and daemon must remain intact and usable.
This catches accidentally handing Sparkle an Install reply during background
prefetch: after preparation, Sparkle may install on host termination even while
the user driver's ready reply remains pending.
## Previously published apps

The gate does not rebuild or exercise older versions. Each shipped version's own
updater was proven by its candidate passes when it shipped, and shipped binaries
only trust the production key, so a test-key rebuild of old source proved little
while adding about four minutes of build and five of notarization per release,
growing with every version. Versions 0.2.0 and 0.3.1 have updater windows that crash
when closing or replacing an offer; they need a one-time manual installation of
0.3.2 or later. Legacy clients still use the signed JSON feed and `ainc-update`
helper, so the release keeps publishing that feed and the full archive.

To validate a branch, dispatch `Distribution` at its exact commit with
`test=true` and `build=true`. Never use `test=false` for branch validation.
