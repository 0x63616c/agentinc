# The three things you do here.

_default:
    @just --list

# Run everything: Postgres, Temporal, the daemon and the Mac app, all rebuilding on save. Needs Docker running.
dev:
    cargo xtask dev

# Run every check CI runs.
test:
    cargo fmt --all -- --check
    python3 crates/ainc-mac/scripts/check-colors.py
    python3 crates/ainc-mac/scripts/check-ui-spacing.py
    python3 crates/ainc-mac/scripts/check-ui-core.py
    python3 -m unittest discover -s scripts/release -p 'test_*.py'
    cargo xtask generate --check
    cargo clippy --locked --workspace --all-targets -- -D warnings
    cargo test --locked --workspace

# Bump to the next version and commit it. Pushing that commit to main ships the release.
# Takes patch, minor or major, or an explicit version that must be one of those three next versions.
release bump:
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
