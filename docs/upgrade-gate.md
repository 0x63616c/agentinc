# Native upgrade release gate

Distribution stages a signed and notarized release as a draft. It publishes only
after the macOS `upgrade` job succeeds. A branch dispatch with `test=true`
exercises the same gate and keeps every artifact in a draft.

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
After relaunch, the gate runs the signed bundle's `aincd --terminal-attach`
through a PTY. A new pane and a saved pane whose daemon session is gone must
both show startup output before input and run a command.

## Previously published apps

The gate downloads every older published `AgentInc.tar.gz`, installs it in an
isolated directory, and checks its code signature, Gatekeeper assessment and
version. Versions 0.2.0 and 0.3.1 are explicitly reported as known broken:
their updater windows crash when closing or replacing an offer. Both require
a one-time manual installation of 0.3.2 or later.

Already shipped binaries have the production GitHub feed URL and production
public key compiled in. Before the candidate is published, they cannot accept
the local feed signed with the test key. Signing a local test feed with the
production update key would make it valid for every production client and is
forbidden. Thus the predecessor check can verify installed release assets but
cannot prove an old binary updates to the unpublished candidate. The gate
reports this limitation for every version; it does not silently omit one.
For a prior release whose tag contains the test-only feed build, Distribution
also rebuilds that exact tagged source with this run's test key, signs and
notarizes it, and drives both in-app paths from that version to the candidate.
Legacy fixtures still use the original signed JSON feed and `ainc-update` helper;
the migration release continues publishing that feed and full archive so already
installed clients can acquire Sparkle. Their first migration download is necessarily
full. Subsequent Sparkle-enabled versions can use deltas.
The gate checks the rebuilt version and commit against the installed published
asset. This begins with versions shipped after this gate lands; older binaries
cannot be rebuilt with the override. The candidate's two in-app passes prevent
a newly broken updater from shipping.

To validate a branch, dispatch `Distribution` at its exact commit with
`test=true` and `build=true`. Never use `test=false` for branch validation.
