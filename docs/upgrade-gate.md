# Native upgrade release gate

Distribution stages a signed and notarized release as a draft. It publishes only
after the macOS `upgrade` job succeeds. A branch dispatch with `test=true`
exercises the same gate and keeps every artifact in a draft.

The native build job makes three handoffs from the exact commit: the production
candidate, an upgrade-test candidate, and the same upgrade-test code reporting
the next patch version. The test builds have the production bundle identity but
a separate compiled Ed25519 public key. The Mac generates the matching private
key for that workflow run. Linux signs and notarizes all three apps. The test
key travels only in the one-day Actions handoff; it is never a release asset.
The shipping handoff carries `upgrade_test: false`, which `distribute.py`
requires before staging it. The test-only feed override and local HTTP client
are compiled out of the shipping app and helper. Signature, archive hash,
Developer ID, and Gatekeeper checks remain mandatory in both builds.

`scripts/release/upgrade-gate.py` installs the signed test candidate in an
isolated profile and serves the signed newer build from a local feed. It starts
the actual app twice. The manual pass triggers Check for Updates, displays the
AppKit offer and clicks its Install Update button. The automatic pass checks
silently, downloads and verifies the archive, displays Update ready, and clicks
the same button. Each pass must exit normally, run the signed `ainc-update`
helper, replace the app bundle, relaunch as the newer version, and retire the
backup after runtime readiness. The test hook calls the real AppKit button's
`performClick`; the old window ownership bug crashes that call.

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
The gate checks the rebuilt version and commit against the installed published
asset. This begins with versions shipped after this gate lands; older binaries
cannot be rebuilt with the override. The candidate's two in-app passes prevent
a newly broken updater from shipping.

To validate a branch, dispatch `Distribution` at its exact commit with
`test=true` and `build=true`. Never use `test=false` for branch validation.
