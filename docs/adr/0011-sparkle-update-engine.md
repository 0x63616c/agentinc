# Sparkle update engine with AgentInc presentation

Adopt Sparkle for macOS update checking, downloading, binary-delta application,
verification and installation. Keep AgentInc's existing AppKit update windows and
GPUI settings through a custom `SPUUserDriver`. The application remains responsible
for flushing UI state and draining its owned daemon before installation.

Automatic **download-only** uses an AgentInc-owned archive cache. Sparkle 2.9.6's
prepared installer can install when the UI exits unexpectedly; holding its ready
reply does not prevent this. Therefore we never release Sparkle's initial Install
reply until the user has chosen installation and the runtime has drained. Cached
bytes enter Sparkle through its public mutable download-request hook, retaining
Sparkle's signature verification, delta application and full-download fallback.
If an armed installation is interrupted, the startup fence pins that target's signed
appcast, published beside its immutable release assets, so the same version stays
retryable after the latest feed advances; a newer release cannot supersede it.

The full compressed bundle was approximately 174 MB at version 0.5.0 because it
includes the app, daemon and local runtimes. Unchanged runtime binaries should not
need downloading with every app change. Sparkle already implements delta selection,
patching and full-download fallback; maintaining a second implementation offers no
product benefit. This supersedes [0009](0009-own-rust-updater.md).

Existing clients still receive the signed JSON feed and full archive for the
migration release. After that full update, Sparkle-capable clients use the appcast
and any applicable delta. Product data remains outside the replaceable bundle.

The [native upgrade gate](../upgrade-gate.md) must demonstrate the legacy migration,
Sparkle full updates, real delta downloads and invalid-delta fallback before release.
