#!/usr/bin/env python3
"""Seed an isolated AgentInc stack and capture one real app frame.

Run after `cargo xtask dev` and `crates/ainc-mac/scripts/bundle.sh automation`.
GPUI Pilot reads the running app's retina Metal frame. No desktop input is
synthesized and the app is closed immediately afterward.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
APP = ROOT / "crates/ainc-mac/dist/AgentInc Dev.app/Contents/MacOS/AgentInc"
PILOT = ROOT / "target/debug/gpui-pilot"
DISCOVERY = ROOT / ".local/dev/api-url"
TITLE = "AgentInc README Capture"
TICKETS = (
    ("Explore offline onboarding", "backlog"),
    ("Map connector permissions", "backlog"),
    ("Draft first-run checklist", "to_do"),
    ("Review sync error copy", "to_do"),
    ("Stabilize ticket import", "in_progress"),
    ("Polish keyboard navigation", "in_progress"),
    ("Ship updater smoke test", "done"),
)


def pilot(manifest, *args, typed=None):
    result = subprocess.run(
        [str(PILOT), "--instance", str(manifest), "--json", *args],
        input=typed, text=True, capture_output=True,
    )
    reply = json.loads(result.stdout) if result.stdout else {}
    if result.returncode or reply.get("status") != "ok":
        raise RuntimeError(f"Pilot {args}: {reply or result.stderr}")
    return reply["output"]


def snapshot(manifest):
    return pilot(manifest, "snapshot")["snapshot"]


def node(manifest, author_id):
    return next(n for n in snapshot(manifest)["nodes"] if n.get("author_id") == author_id)


def click(manifest, author_id):
    for _ in range(20):
        try:
            return pilot(manifest, "click", node(manifest, author_id)["reference"])
        except RuntimeError as error:
            if "stale_ref" not in str(error):
                raise
    raise RuntimeError(f"Pilot ref stayed stale: {author_id}")


def type_into(manifest, author_id, value):
    click(manifest, author_id)
    for _ in range(20):
        try:
            return pilot(manifest, "type", node(manifest, author_id)["reference"], "--stdin", typed=value)
        except RuntimeError as error:
            if "stale_ref" not in str(error):
                raise
    raise RuntimeError(f"Pilot input stayed stale: {author_id}")


def wait(manifest, kind, author_id):
    if kind == "absent":
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            if not any(n.get("author_id") == author_id for n in snapshot(manifest)["nodes"]):
                return
        raise TimeoutError(f"Pilot node remained present: {author_id}")
    return pilot(manifest, "wait", json.dumps({"kind": kind, "author_id": author_id}), "10000")


def wait_status(manifest, status):
    author_id = f"tickets.status.{status}"
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        state = snapshot(manifest)
        status_node = next(n for n in state["nodes"] if n.get("author_id") == author_id)
        back = next(n for n in state["nodes"] if n.get("author_id") == "tickets.back")
        if not status_node["enabled"] and back["enabled"]:
            return
    raise TimeoutError(f"Ticket status did not settle: {status}")


def create_ticket(manifest, title, status):
    def matches():
        return [n for n in snapshot(manifest)["nodes"]
                if n.get("name") == title and (n.get("author_id") or "").startswith("ticket.")]

    if not matches():
        click(manifest, "tickets.create")
        type_into(manifest, "tickets.title", title)
        pilot(manifest, "wait", json.dumps({
            "kind": "value", "author_id": "tickets.title", "equals": title,
        }), "10000")
        click(manifest, "tickets.submit")
        wait(manifest, "absent", "tickets.title")
    deadline = time.monotonic() + 10
    while not matches() and time.monotonic() < deadline:
        pass
    found = matches()
    if len(found) != 1:
        raise RuntimeError(f"Expected one visible Ticket: {title}")
    if status == "to_do":
        return
    click(manifest, found[0]["author_id"])
    wait(manifest, "present", "tickets.back")
    if node(manifest, f"tickets.status.{status}")["enabled"]:
        click(manifest, f"tickets.status.{status}")
        wait_status(manifest, status)
    click(manifest, "tickets.back")
    wait(manifest, "present", "tickets.create")


def wait_for_manifest(app, manifest, log_path):
    deadline = time.monotonic() + 20
    while not manifest.is_file():
        if app.poll() is not None or time.monotonic() >= deadline:
            raise RuntimeError(log_path.read_text())
        time.sleep(0.05)  # Wait only for process startup; UI uses Pilot gates.
    pilot(manifest, "hello")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path, help="raw PNG path under ignored .local/")
    args = parser.parse_args()
    if not APP.is_file() or not PILOT.is_file() or not DISCOVERY.is_file():
        parser.error("start the isolated stack and build the automation bundle first")
    output = args.output.resolve()
    if not output.is_relative_to(ROOT / ".local"):
        parser.error("raw capture must stay under this worktree's ignored .local/")
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="readme-", dir=ROOT / ".local") as directory:
        state = Path(directory)
        env = os.environ.copy()
        env.update({
            "AGENTINC_SESSION_PATH": str(state / "session.json"),
            "AINC_DISCOVERY_FILE": str(DISCOVERY),
            "AINC_LEGACY_DIR": str(state / "legacy"),
            "AGENTINC_CODEX_HOME": str(state / "codex"),
            "AGENTINC_WINDOW_TITLE": TITLE,
            "AGENTINC_CAPTURE_WORKSPACE": "Acme Inc",
            "AGENTINC_CAPTURE_PROFILE": "Alex",
        })
        with (state / "app.log").open("w") as log:
            app = subprocess.Popen(
                [str(APP), "--gpui-pilot-session", str(state / "p")],
                env=env, stdout=log, stderr=log,
            )
            try:
                manifest = state / "p/instance.json"
                wait_for_manifest(app, manifest, state / "app.log")
                version = node(manifest, "sidebar.version").get("name")
                if not version or version.endswith("-dev"):
                    raise RuntimeError(f"README capture needs a production-channel build: {version}")
                click(manifest, "nav.tickets")
                wait(manifest, "present", "tickets.create")
                for title, status in TICKETS:
                    create_ticket(manifest, title, status)
                app.terminate()
                app.wait(timeout=5)
                app = subprocess.Popen(
                    [str(APP), "--gpui-pilot-session", str(state / "q")],
                    env=env, stdout=log, stderr=log,
                )
                manifest = state / "q/instance.json"
                wait_for_manifest(app, manifest, state / "app.log")
                click(manifest, "nav.tickets")
                deadline = time.monotonic() + 10
                while time.monotonic() < deadline:
                    if any(n.get("name") == "Ship updater smoke test"
                           for n in snapshot(manifest)["nodes"]):
                        break
                else:
                    raise TimeoutError("Seeded Tickets did not load")
                visible = [n.get("name") for n in snapshot(manifest)["nodes"]
                           if (n.get("author_id") or "").startswith("ticket.")]
                if sorted(visible) != sorted(title for title, _ in TICKETS):
                    raise RuntimeError(f"Capture stack contains unexpected Tickets: {visible}")
                if any(n.get("author_id") == "close-evee" for n in snapshot(manifest)["nodes"]):
                    click(manifest, "close-evee")
                    wait(manifest, "absent", "close-evee")
                capture = pilot(manifest, "screenshot")
                if (capture["width"], capture["height"]) != (2720, 1656):
                    raise RuntimeError(f"Unexpected retina frame: {capture}")
                shutil.copyfile(capture["path"], output)
                print(f"Captured real app frame {capture['frame']}: {output}")
            finally:
                app.terminate()
                try:
                    app.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    app.kill()
                    app.wait()


if __name__ == "__main__":
    main()
