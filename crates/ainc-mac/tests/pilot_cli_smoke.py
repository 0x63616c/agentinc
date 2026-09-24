#!/usr/bin/env python3
"""macOS CLI/OS smoke. Run after cargo build --workspace --features automation."""
import json
import os
import pathlib
import subprocess
import tempfile
import time

root = pathlib.Path.cwd()
output = root / "target/pilot-acceptance"
output.mkdir(parents=True, exist_ok=True)
(root / ".local").mkdir(exist_ok=True)
with tempfile.TemporaryDirectory(prefix="c", dir=root / ".local") as temp:
    state = pathlib.Path(temp)
    env = os.environ.copy()
    env.update(
        AGENTINC_SESSION_PATH=str(state / "session.json"),
        AGENTINC_DATABASE_PATH=str(state / "db.sqlite3"),
        AGENTINC_CODEX_HOME=str(state / "codex"),
        AGENTINC_WINDOW_TITLE="Agentinc Pilot CLI QA",
    )
    with (output / "cli-app.log").open("w") as log:
        app = subprocess.Popen(
            [root / "target/debug/agentinc-os", "--gpui-pilot-session", state / "s"],
            env=env, stdout=log, stderr=log,
        )
        try:
            manifest = state / "s/instance.json"
            deadline = time.monotonic() + 20
            while not manifest.exists():
                assert app.poll() is None, "app exited"
                assert time.monotonic() < deadline, "startup timeout"
                time.sleep(0.02)  # Process readiness only; UI waits use the protocol.
            base = [str(root / "target/debug/gpui-pilot"), "--instance", str(manifest), "--json"]

            def cli(*args, text=None):
                result = subprocess.run(base + list(args), input=text, text=True, capture_output=True)
                if result.returncode:
                    raise RuntimeError(result.stderr)
                return json.loads(result.stdout)["output"]

            assert cli("hello")["title"] == "Agentinc Pilot CLI QA"
            snap = cli("snapshot")["snapshot"]
            assert any(node["author_id"] == "shell.search" for node in snap["nodes"])
            native = subprocess.run(
                ["swift", "tests/pilot_os_acceptance.swift", str(app.pid), "Agentinc Pilot CLI QA"],
                capture_output=True, text=True, check=True,
            )
            (output / "os-window.json").write_text(native.stdout)
            cli("press", "cmd-k")
            cli("wait", '{"kind":"present","author_id":"search.input"}', "3000")
            for _ in range(100):
                snap = cli("snapshot")["snapshot"]
                ref = next(node["reference"] for node in snap["nodes"] if node["author_id"] == "search.input")
                try:
                    cli("type", ref, "--stdin", text="Tasks")
                    break
                except RuntimeError as error:
                    if "stale_ref" not in str(error):
                        raise
            else:
                raise AssertionError("unable to get fresh ref")
            cli("wait", '{"kind":"value","author_id":"search.input","equals":"Tasks"}', "3000")
            cli("press", "enter")
            cli("wait", '{"kind":"present","author_id":"tasks.create"}', "3000")
            capture = cli("screenshot")
            assert pathlib.Path(capture["path"]).is_file()
            print("CLI hello/snapshot/press/type/wait/screenshot passed; native PID/title/dimensions passed.")
            print(native.stdout)
            # Graceful quit via the same driver; a closed transport is expected.
            subprocess.run(base + ["press", "cmd-q"], capture_output=True)
            app.wait(timeout=5)
            assert not manifest.exists(), "owned endpoints were not removed on normal quit"
            print("Normal shutdown removed instance manifest/token/socket.")
        finally:
            if app.poll() is None:
                app.terminate()
                app.wait(timeout=5)
