# Sparkle public headers

Unmodified public headers and license from **Sparkle 2.9.6**, the latest stable
2.9 release (verified against GitHub releases on 2026-10-03).

Source: <https://github.com/sparkle-project/Sparkle/releases/tag/2.9.6>

Archive: `Sparkle-2.9.6.tar.xz`

SHA-256: `52bf9e88cdd972fc0c81501377a880e90d47031bd8ca5462488f843e2609e192`

Headers are copied from `Sparkle.framework/Versions/B/Headers`. Normal builds
compile against these declarations without linking Sparkle. Production bundles
embed the matching signed framework in `Contents/Frameworks`; AgentInc loads it
with `NSBundle` and creates its own `SPUUserDriver` in `update_window.m`.

Do not hand-edit upstream declarations. Replace them from an upstream release
when updating the framework pin. See `LICENSE` for upstream and bundled licenses.
