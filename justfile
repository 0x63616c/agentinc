# The things you do here. Every recipe first points git at the tracked hooks in .githooks.

_default:
    @just --list

# Run everything: Postgres, Temporal, the daemon and the Mac app, all rebuilding on save. Needs Docker running.
dev: _hooks
    cargo xtask dev

# Point git at the tracked hooks. Idempotent, so every recipe can depend on it.
_hooks:
    @git rev-parse --git-dir >/dev/null 2>&1 && [ "$(git config core.hooksPath)" = .githooks ] || git config core.hooksPath .githooks 2>/dev/null || true

# The fast static gate, no database and no tests: fmt, clippy and the native UI checks. Runs on every commit.
check: _hooks
    cargo xtask check

# Apply the fixes clippy and rustfmt can make on their own.
fix: _hooks
    cargo clippy --workspace --all-targets --fix --allow-dirty --allow-staged
    cargo fmt --all

# Run every check CI runs: `check`, then nextest and the doctests. Starts a throwaway Postgres in Docker unless DATABASE_URL is set.
test: check
    cargo xtask test

# Remove the incremental compilation caches under target/ (they grow without bound; dependencies stay built).
clean-incremental:
    cargo xtask clean-incremental

# Bump to the next version (patch, minor, major, or an explicit one of those) and commit; pushing to main ships it.
release bump: _hooks
    cargo xtask bump {{bump}}
