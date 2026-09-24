#!/usr/bin/env python3
"""Capture synthetic AgentInc pages through the real GPUI Pilot app.

Run from an isolated, empty worktree after `cargo xtask dev` and:
`cargo build --locked -p agentinc-os -p gpui-pilot-cli --features agentinc-os/automation`.
Only the composited, privacy-redacted output should be committed.
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
APP = ROOT / "target/debug/agentinc-os"
PILOT = ROOT / "target/debug/gpui-pilot"
DISCOVERY = ROOT / ".local/dev/api-url"


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
        ref = node(manifest, author_id)["reference"]
        try:
            return pilot(manifest, "click", ref)
        except RuntimeError as error:
            if "stale_ref" not in str(error):
                raise
    raise RuntimeError(f"Pilot ref stayed stale: {author_id}")


def type_into(manifest, author_id, value):
    click(manifest, author_id)
    for _ in range(20):
        ref = node(manifest, author_id)["reference"]
        try:
            return pilot(manifest, "type", ref, "--stdin", typed=value)
        except RuntimeError as error:
            if "stale_ref" not in str(error):
                raise
    raise RuntimeError(f"Pilot input stayed stale: {author_id}")


def replace_into(manifest, author_id, value):
    click(manifest, author_id)
    pilot(manifest, "press", "cmd-a")
    for _ in range(20):
        ref = node(manifest, author_id)["reference"]
        try:
            return pilot(manifest, "type", ref, "--stdin", typed=value)
        except RuntimeError as error:
            if "stale_ref" not in str(error):
                raise
    raise RuntimeError(f"Pilot input stayed stale: {author_id}")


def wait(manifest, kind, author_id, equals=None):
    condition = {"kind": kind, "author_id": author_id}
    if equals is not None:
        condition["equals"] = equals
    return pilot(manifest, "wait", json.dumps(condition), "10000")


def capture(manifest, output, name):
    result = pilot(manifest, "screenshot")
    if (result["width"], result["height"]) != (2720, 1656):
        raise RuntimeError(f"Unexpected retina capture size: {result}")
    shutil.copyfile(result["path"], output / f"{name}.png")
    print(f"{name}: frame {result['frame']}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path, help="raw PNG directory (keep untracked)")
    args = parser.parse_args()
    if not APP.is_file() or not PILOT.is_file():
        parser.error("build the automation app and GPUI Pilot first")
    if not DISCOVERY.is_file():
        parser.error("start this worktree's isolated stack with cargo xtask dev")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    scratch = ROOT / ".local"
    scratch.mkdir(exist_ok=True)
    # Unix domain sockets have a short path limit on macOS.
    with tempfile.TemporaryDirectory(prefix="r", dir=scratch) as directory:
        state = Path(directory)
        env = os.environ.copy()
        env.update({
            "AGENTINC_SESSION_PATH": str(state / "session.json"),
            "AINC_DISCOVERY_FILE": str(DISCOVERY),
            "AINC_LEGACY_DIR": str(state / "legacy"),
            "AGENTINC_CODEX_HOME": str(state / "codex"),
            "AGENTINC_WINDOW_TITLE": "AgentInc Demo",
        })
        log = (state / "app.log").open("w")
        app = subprocess.Popen(
            [str(APP), "--gpui-pilot-session", str(state / "pilot")],
            env=env, stdout=log, stderr=log,
        )
        try:
            manifest = state / "pilot/instance.json"
            deadline = time.monotonic() + 20
            while not manifest.is_file():
                if app.poll() is not None:
                    raise RuntimeError((state / "app.log").read_text())
                if time.monotonic() >= deadline:
                    raise TimeoutError((state / "app.log").read_text())
                time.sleep(0.05)  # Process startup; UI readiness uses Pilot.
            pilot(manifest, "hello")
            if node(manifest, "today.tickets.summary")["name"] != "All caught up":
                raise RuntimeError("Demo worktree must start with an empty Ticket list")

            click(manifest, "nav.agents")
            wait(manifest, "present", "agents.create")
            if any((n.get("author_id") or "").startswith("agent.") for n in snapshot(manifest)["nodes"]):
                raise RuntimeError("Demo worktree must start with no registered agents")
            click(manifest, "agents.create")
            type_into(manifest, "agents.name", "Demo reviewer")
            type_into(manifest, "agents.instructions", "Review example project work.")
            click(manifest, "tickets.submit")
            wait(manifest, "absent", "agents.name")
            agents = [n for n in snapshot(manifest)["nodes"]
                      if n.get("name") == "Demo reviewer" and
                      (n.get("author_id") or "").startswith("agent.")]
            if len(agents) != 1:
                raise RuntimeError("Demo agent not visible")
            agent_id = agents[0]["author_id"].removeprefix("agent.")

            click(manifest, "nav.tickets")
            wait(manifest, "present", "tickets.create")
            for title in ("Review onboarding flow", "Write API examples", "Plan release checklist"):
                click(manifest, "tickets.create")
                type_into(manifest, "tickets.title", title)
                click(manifest, "tickets.submit")
                wait(manifest, "absent", "tickets.title")
            capture(manifest, output, "hero")
            tickets = [n for n in snapshot(manifest)["nodes"]
                       if n.get("name") == "Review onboarding flow" and
                       (n.get("author_id") or "").startswith("ticket.") and
                       not n["author_id"].endswith(".complete")]
            if len(tickets) != 1:
                raise RuntimeError("Demo Ticket not visible")
            click(manifest, tickets[0]["author_id"])
            wait(manifest, "present", "tickets.comment")
            type_into(manifest, "tickets.comment", "Check the welcome screen and setup steps.")
            click(manifest, "tickets.post")
            wait(manifest, "value", "tickets.comment", "")
            capture(manifest, output, "tickets")

            click(manifest, "nav.automations")
            wait(manifest, "present", "automations.create")
            if any((n.get("author_id") or "").startswith("automations.rule.")
                   for n in snapshot(manifest)["nodes"]):
                raise RuntimeError("Demo worktree must start with no Automations")
            click(manifest, "automations.create")
            type_into(manifest, "automations.name", "Weekly repository review")
            type_into(manifest, "automations.prompt", "Review the example repository and summarize changes.")
            replace_into(manifest, "automations.minutes", "10080")
            click(manifest, f"automations.agent.{agent_id}")
            click(manifest, "automations.save")
            wait(manifest, "present", "automations.pause")
            click(manifest, "automations.refresh")
            capture(manifest, output, "automations")
            click(manifest, "automations.pause")
            pilot(manifest, "wait", json.dumps({
                "kind": "name", "author_id": "automations.pause", "equals": "Resume"
            }), "10000")

            click(manifest, "nav.today")
            wait(manifest, "present", "panel-new")
            capture(manifest, output, "evee")
            click(manifest, "profile")
            wait(manifest, "present", "profile")
            capture(manifest, output, "settings")
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
