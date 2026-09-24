#!/usr/bin/env python3
"""Capture the native pages through the opt-in GPUI Pilot driver."""

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time


ROOT = Path(__file__).resolve().parents[3]
APP = ROOT / "target/debug/agentinc-os"
PILOT = ROOT / "target/debug/gpui-pilot"
PAGES = (
    ("today", "cmd-1"),
    ("tickets", "cmd-2"),
    ("agents", "cmd-3"),
    ("home", "cmd-4"),
    ("calendar", "cmd-5"),
    ("library", "cmd-6"),
    ("my-apps", "cmd-7"),
    ("assistant", "cmd-8"),
    ("automations", "cmd-9"),
)


def run_pilot(manifest, *args):
    result = subprocess.run(
        [str(PILOT), "--instance", str(manifest), "--json", *args],
        text=True,
        capture_output=True,
    )
    if result.returncode:
        raise RuntimeError(result.stdout + result.stderr)
    return json.loads(result.stdout)["output"]


def capture(manifest, output, name):
    # The native hover and page transitions finish in at most 220 ms.
    time.sleep(0.8 if name == "search" else 0.3)
    result = run_pilot(manifest, "screenshot")
    source = Path(result["path"])
    assert (result["width"], result["height"]) == (2720, 1656)
    destination = output / f"{name}.png"
    subprocess.run(["sips", "-Z", "1360", str(source), "--out", str(destination)],
                   check=True, capture_output=True)
    print(f"{name}: frame {result['frame']} -> {destination}")


def main():
    if len(sys.argv) != 2:
        raise SystemExit("usage: capture-color-pages.py OUTPUT_DIRECTORY")
    output = Path(sys.argv[1]).resolve()
    output.mkdir(parents=True, exist_ok=True)
    assert APP.is_file() and PILOT.is_file(), "build the automation app and pilot first"
    scratch = ROOT / ".local"
    scratch.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="color-pages-", dir=scratch) as temporary:
        state = Path(temporary)
        token = state / "owner-token"
        token.write_text("isolated-unavailable-fixture")
        env = os.environ.copy()
        env.update({
            "USER": "agentinc-evidence",
            "AGENTINC_SESSION_PATH": str(state / "session.json"),
            "AINC_DISCOVERY_FILE": str(state / "api-url"),
            "AINC_TOKEN_FILE": str(token),
            "AINC_DAEMON_URL": "http://127.0.0.1:1",
            "AINC_LEGACY_DIR": str(state / "legacy"),
            "AGENTINC_CODEX_HOME": str(state / "codex"),
            "AGENTINC_WINDOW_TITLE": "AgentInc Color Evidence",
        })
        log = (state / "app.log").open("w")
        app = subprocess.Popen([str(APP), "--gpui-pilot-session", str(state / "pilot")],
                               env=env, stdout=log, stderr=log)
        try:
            manifest = state / "pilot/instance.json"
            deadline = time.monotonic() + 20
            while not manifest.is_file():
                if app.poll() is not None:
                    raise RuntimeError((state / "app.log").read_text())
                if time.monotonic() >= deadline:
                    raise TimeoutError((state / "app.log").read_text())
                time.sleep(0.05)
            run_pilot(manifest, "hello")
            for name, key in PAGES:
                run_pilot(manifest, "press", key)
                capture(manifest, output, name)
            # Offline page refreshes can commit a frame between snapshot and click.
            # A stale ref dispatches no input, so retry only that response.
            for attempt in range(50):
                snapshot = run_pilot(manifest, "snapshot")["snapshot"]
                profile = next(node for node in snapshot["nodes"]
                               if node.get("author_id") == "profile")
                try:
                    run_pilot(manifest, "click", profile["reference"])
                    break
                except RuntimeError as error:
                    if "stale_ref" not in str(error) or attempt == 49:
                        raise
            capture(manifest, output, "settings")
            run_pilot(manifest, "press", "cmd-k")
            capture(manifest, output, "search")
        finally:
            app.terminate()
            try:
                app.wait(timeout=5)
            except subprocess.TimeoutExpired:
                app.kill()
                app.wait()
            log.close()


if __name__ == "__main__":
    main()
