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

# Run every check CI runs. Starts a throwaway Postgres in Docker unless DATABASE_URL is set.
test: check
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -z "${DATABASE_URL:-}" ]; then
        container=$(docker run -d --rm -e POSTGRES_PASSWORD=test -p 127.0.0.1::5432 postgres:16-alpine)
        trap 'docker stop "$container" >/dev/null' EXIT
        until docker exec "$container" pg_isready -U postgres >/dev/null 2>&1; do sleep 1; done
        port=$(docker port "$container" 5432/tcp | head -1 | sed 's/.*://')
        export DATABASE_URL="postgres://postgres:test@127.0.0.1:$port/postgres"
    fi
    # The xtask workspace test checks generation using the same compiled dependency graph.
    cargo test --locked --workspace

# Bump to the next version (patch, minor, major, or an explicit one of those) and commit; pushing to main ships it.
release bump: _hooks
    cargo xtask bump {{bump}}
