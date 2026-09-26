#!/usr/bin/env python3
"""Exercise a bundled macOS app and its self-started, isolated daemon."""
import argparse
import json
import os
from pathlib import Path
import select
import shutil
import subprocess
import tempfile
import time
import urllib.request
import uuid

ROOT = Path(__file__).resolve().parents[2]
ROUTES = ("tickets", "assistant", "agents", "automations", "terminal", "temporal", "settings")


def wait_file(path, process, seconds):
    """Wait for a real filesystem publication, rather than a startup delay."""
    deadline = time.monotonic() + seconds
    while not path.is_file():
        directory = path.parent if path.parent.is_dir() else path.parent.parent
        fd = os.open(directory, os.O_RDONLY)
        queue = select.kqueue()
        event = select.kevent(fd, filter=select.KQ_FILTER_VNODE,
                              flags=select.KQ_EV_ADD | select.KQ_EV_CLEAR,
                              fflags=select.KQ_NOTE_WRITE)
        try:
            if path.is_file():
                break
            if process.poll() is not None:
                raise RuntimeError(f'app exited {process.returncode} before {path}')
            remaining = deadline - time.monotonic()
            if remaining <= 0 or not queue.control([event], 1, remaining):
                raise TimeoutError(f'{path} was not published within {seconds}s')
        finally:
            queue.close()
            os.close(fd)


def request(discovery, path, token=None, data=None):
    url = discovery.read_text().strip() + path
    headers = {'Authorization': 'Bearer ' + token} if token else {}
    req = urllib.request.Request(url, headers=headers, data=data)
    with urllib.request.build_opener(urllib.request.ProxyHandler({})).open(req, timeout=10) as response:
        return response.read()


def wait_value(path, predicate, process, seconds):
    deadline = time.monotonic() + seconds
    fd = os.open(path.parent, os.O_RDONLY)
    queue = select.kqueue()
    event = select.kevent(fd, filter=select.KQ_FILTER_VNODE,
                          flags=select.KQ_EV_ADD | select.KQ_EV_CLEAR,
                          fflags=select.KQ_NOTE_WRITE)
    try:
        while not path.is_file() or not predicate(path.read_text()):
            if process.poll() is not None:
                raise RuntimeError(f'app exited {process.returncode} while waiting for {path}')
            remaining = deadline - time.monotonic()
            if remaining <= 0 or not queue.control([event], 1, remaining):
                raise TimeoutError(f'{path} did not reach expected state within {seconds}s')
    finally:
        queue.close()
        os.close(fd)


def native_window(process, output):
    check = ROOT / 'crates/ainc-mac/tests/pilot_os_acceptance.swift'
    native = subprocess.run(['swift', str(check), str(process.pid), 'AgentInc End-to-End CI'],
                            capture_output=True, text=True, timeout=50, check=True)
    (output / 'native-window.json').write_text(native.stdout)
    return json.loads(native.stdout)['window_number']


def native_input(process, operation):
    subprocess.run(['swift', str(ROOT / 'scripts/e2e/native_input.swift'), str(process.pid),
                    'AgentInc End-to-End CI', operation], check=True, timeout=50)


def native_capture(window, output, label):
    path = output / f'{label}.png'
    subprocess.run(['screencapture', '-x', '-l', str(window), str(path)],
                   check=True, timeout=15)
    assert path.stat().st_size > 4000, f'{label}: empty native capture'
    return path


def terminal_ui(process, output, layout):
    window = native_window(process, output)
    native_capture(window, output, 'terminal-before')
    native_input(process, 'command')
    deadline = time.monotonic() + 25
    while True:
        image = native_capture(window, output, 'terminal-command')
        ocr = subprocess.run(['swift', str(ROOT / 'scripts/e2e/read_screen.swift'), str(image)],
                             capture_output=True, text=True, check=True, timeout=50).stdout
        (output / 'terminal-command.txt').write_text(ocr)
        if 'AGENTINCUIOUTPUT' in ocr.replace(' ', ''):
            break
        if time.monotonic() >= deadline:
            raise AssertionError(f'terminal command output never appeared in native window: {ocr!r}')
    before = json.loads(layout.read_text())['tree']['ratio']
    native_input(process, 'drag')
    wait_value(layout, lambda data: abs(json.loads(data)['tree']['ratio'] - before) > 0.05,
               process, 10)
    native_capture(window, output, 'terminal-resized')
    print('PASS native terminal command output and split drag', flush=True)


def cli(binary, manifest, *args, text=None):
    command = [str(binary), '--instance', str(manifest), '--json', *args]
    result = subprocess.run(command, input=text, text=True, capture_output=True, timeout=20)
    if result.returncode:
        raise RuntimeError(f'{command[-2:]}: {result.stderr.strip()}')
    return json.loads(result.stdout)['output']


def snapshot(binary, manifest):
    return cli(binary, manifest, 'snapshot')['snapshot']


def click(binary, manifest, author_id=None, name=None):
    for _ in range(8):
        nodes = snapshot(binary, manifest)['nodes']
        matches = [node for node in nodes if (author_id and node['author_id'] == author_id)
                   or (name and node['name'] == name and node['clickable'])]
        if len(matches) != 1:
            raise AssertionError(f'expected one clickable {author_id or name}; got {len(matches)}')
        try:
            return cli(binary, manifest, 'click', matches[0]['reference'])['snapshot']
        except RuntimeError as error:
            if 'stale_ref' not in str(error):
                raise
    raise AssertionError(f'{author_id or name}: refs stayed stale')


def capture(binary, manifest, output, label):
    shot = cli(binary, manifest, 'screenshot')
    source = Path(shot['path'])
    assert source.stat().st_size > 4000, f'{label}: empty Metal capture'
    shutil.copy2(source, output / f'{label}.png')
    snap = snapshot(binary, manifest)
    (output / f'{label}.json').write_text(json.dumps(snap, indent=2))
    bad = [node['author_id'] for node in snap['nodes'] if node['visible'] and
           node['author_id'] in ('temporal.error', 'terminal.unavailable')]
    assert not bad, f'{label}: rendered error states: {bad}'
    return snap


def pilot_flows(binary, manifest, output, session, process, layout):
    assert cli(binary, manifest, 'hello')['title'] == 'AgentInc End-to-End CI'
    capture(binary, manifest, output, 'launch')
    for route in ROUTES:
        click(binary, manifest, author_id=f'nav.{route}')
        state = json.loads(session.read_text())
        assert state['router']['current'] == ('evee' if route == 'assistant' else route), route
        capture(binary, manifest, output, route)
        if route == 'terminal':
            terminal_ui(process, output, layout)
        print(f'PASS rendered route {route}', flush=True)
    # Components is intentionally in Search rather than the main sidebar.
    cli(binary, manifest, 'press', 'cmd-k')
    cli(binary, manifest, 'wait', '{"kind":"present","author_id":"search.input"}', '5000')
    click(binary, manifest, author_id='search.input')
    for _ in range(8):
        node = next(n for n in snapshot(binary, manifest)['nodes'] if n['author_id'] == 'search.input')
        try:
            cli(binary, manifest, 'type', node['reference'], '--stdin', text='Components')
            break
        except RuntimeError as error:
            if 'stale_ref' not in str(error):
                raise
    else:
        raise AssertionError('search input refs stayed stale')
    cli(binary, manifest, 'wait', '{"kind":"present","author_id":"search.result.components"}', '5000')
    click(binary, manifest, author_id='search.result.components')
    capture(binary, manifest, output, 'components')
    assert json.loads(session.read_text())['router']['current'] == 'components'
    click(binary, manifest, author_id='nav.settings')
    click(binary, manifest, name='Larger')
    assert json.loads(session.read_text())['font_size'] == 'larger'
    capture(binary, manifest, output, 'settings-saved')
    print('PASS Settings persisted font size', flush=True)


def run(args):
    app = args.app.resolve()
    executable = app / 'Contents/MacOS/AgentInc'
    daemon = app / 'Contents/MacOS/aincd'
    assert executable.is_file() and daemon.is_file(), f'incomplete bundle: {app}'
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    subprocess.run(['codesign', '--verify', '--deep', '--strict', str(app)], check=True, timeout=30)
    with tempfile.TemporaryDirectory(prefix='agentinc-e2e-') as temporary:
        profile = Path(temporary)
        discovery = profile / 'daemon/api-url'
        session = profile / 'session.json'
        # A saved split exercises both restoration and native pane creation.
        ids = [str(uuid.uuid4()), str(uuid.uuid4())]
        layout = profile / 'terminal-layout.json'
        layout.write_text(json.dumps({'tree': {'vertical': False, 'ratio': 0.5,
                                                'first': {'id': ids[0]}, 'second': {'id': ids[1]}},
                                      'zoomed': None}))
        pilot_parent = Path(tempfile.mkdtemp(prefix='ae', dir='/tmp')) if args.pilot_cli else None
        pilot_dir = pilot_parent / 's' if pilot_parent else None
        env = os.environ.copy()
        for name in ('DATABASE_URL', 'AINC_DATABASE_URL', 'AINC_DAEMON_URL', 'AINC_RUNTIME_CONFIG'):
            env.pop(name, None)
        env.update(AGENTINC_SESSION_PATH=str(session), AINC_DISCOVERY_FILE=str(discovery),
                   AINC_LEGACY_DIR=str(profile / 'legacy'),
                   AGENTINC_CODEX_HOME=str(profile / 'codex'),
                   AGENTINC_WINDOW_TITLE='AgentInc End-to-End CI')
        command = [str(executable)]
        if pilot_dir:
            command += ['--gpui-pilot-session', str(pilot_dir), '--gpui-pilot-visible']
        app_log = (output / 'app.log').open('w')
        process = subprocess.Popen(command, env=env, stdout=app_log, stderr=subprocess.STDOUT)
        try:
            wait_file(discovery, process, 120)
            assert request(discovery, '/health/ready') is not None
            assert (discovery.parent / 'owner-token').is_file()
            print('PASS bundled app and daemon readiness', flush=True)
            subprocess.run(['python3', str(ROOT / 'scripts/release/terminal_smoke.py'),
                            '--app', str(app), '--profile', str(profile)],
                           check=True, timeout=75, env=env)
            if pilot_dir:
                manifest = pilot_dir / 'instance.json'
                wait_file(manifest, process, 40)
                pilot_flows(args.pilot_cli.resolve(), manifest, output, session, process, layout)
                # Relaunch into the same isolated profile to prove saved UI state.
                process.terminate()
                process.wait(timeout=15)
                shutil.rmtree(pilot_parent)
                pilot_parent = Path(tempfile.mkdtemp(prefix='ae', dir='/tmp'))
                pilot_dir = pilot_parent / 's'
                process = subprocess.Popen([str(executable), '--gpui-pilot-session', str(pilot_dir),
                                            '--gpui-pilot-visible'], env=env, stdout=app_log,
                                           stderr=subprocess.STDOUT)
                manifest = pilot_dir / 'instance.json'
                wait_file(manifest, process, 40)
                snap = capture(args.pilot_cli.resolve(), manifest, output, 'relaunch')
                assert json.loads(session.read_text())['font_size'] == 'larger'
                assert any(n['name'] == 'Larger' and (n['selected'] or n['checked'])
                           for n in snap['nodes']), 'saved font size was not selected on relaunch'
                subprocess.run([str(executable), '--update-ui-smoke',
                                str(ROOT / 'crates/ainc-mac/tests/fixtures/update-manifest.json'),
                                str(output / 'update-ui')], check=True, timeout=45, env=env)
                print('PASS update offer and progress UI', flush=True)
            else:
                # Production has no Pilot endpoint, so use the owned native
                # window and keyboard shortcuts for the shipping executable.
                window = native_window(process, output)
                native_capture(window, output, 'signed-launch')
                for index, route in enumerate(ROUTES[:6], 1):
                    native_input(process, f'nav-{index}')
                    expected = 'evee' if route == 'assistant' else route
                    wait_value(session, lambda data: json.loads(data)['router']['current'] == expected,
                               process, 10)
                    native_capture(window, output, f'signed-{route}')
                    if route == 'terminal':
                        terminal_ui(process, output, layout)
                    print(f'PASS signed route {route}', flush=True)
                native_input(process, 'settings')
                wait_value(session, lambda data: json.loads(data)['router']['current'] == 'settings',
                           process, 10)
                native_capture(window, output, 'signed-settings')
        finally:
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=15)
            app_log.close()
            for log in profile.rglob('*.log'):
                shutil.copy2(log, output / log.name)
            token = discovery.parent / 'owner-token'
            if discovery.is_file() and token.is_file():
                try:
                    request(discovery, '/internal/drain', token.read_text().strip(), b'')
                except OSError:
                    pass
            if pilot_parent:
                shutil.rmtree(pilot_parent, ignore_errors=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--app', type=Path, required=True)
    parser.add_argument('--pilot-cli', type=Path)
    parser.add_argument('--output', type=Path, default=Path('target/app-e2e'))
    run(parser.parse_args())
