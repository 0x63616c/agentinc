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
    python3 -m unittest discover -s scripts/release -p 'test_*.py'
    # The xtask workspace test checks generation using the same compiled dependency graph.
    cargo test --locked --workspace

# Bump to the next version (patch, minor, major, or an explicit one of those) and commit; pushing to main ships it.
release bump: _hooks
    #!/usr/bin/env python3
    import re, subprocess, sys
    run = lambda *cmd: subprocess.run(cmd, check=True)
    toml = open("Cargo.toml").read()
    current = tuple(int(n) for n in re.search(r'^version = "(\d+)\.(\d+)\.(\d+)"', toml, re.M).groups())
    major, minor, patch = current
    allowed = {"patch": (major, minor, patch + 1), "minor": (major, minor + 1, 0), "major": (major + 1, 0, 0)}
    show = lambda v: ".".join(map(str, v))
    bump = "{{bump}}"
    if bump in allowed:
        new = allowed[bump]
    else:
        try:
            new = tuple(int(n) for n in bump.split("."))
        except ValueError:
            new = None
        if new not in allowed.values():
            options = ", ".join(f"{show(v)} ({k})" for k, v in allowed.items())
            sys.exit(f"Current version is {show(current)}. Next must be one of: {options}.")
    if subprocess.run(["git", "status", "--porcelain"], capture_output=True, text=True, check=True).stdout.strip():
        sys.exit("Working tree is not clean: commit or stash first, so the release commit holds only the version bump.")
    open("Cargo.toml", "w").write(re.sub(r'^version = ".*?"', f'version = "{show(new)}"', toml, count=1, flags=re.M))
    run("cargo", "update", "--workspace")
    run("cargo", "xtask", "generate")
    run("git", "add", "-A")
    run("git", "commit", "-m", f"Release {show(new)}")
    print(f"Committed Release {show(new)}. Push to main to ship it.")
