#!/usr/bin/env python3
"""Run signed, notarized upgrade fixtures through the real native app and helper."""
import argparse
import functools
import http.server
import json
import os
from pathlib import Path
import select
import shutil
import subprocess
import tempfile
import threading
import time


KNOWN_BROKEN = {
    '0.2.0': 'update window crashes when Check for Updates closes it',
    '0.3.1': 'update window crashes on Check for Updates and Update ready',
}


def run(*args, **kwargs):
    subprocess.run(args, check=True, **kwargs)


def unpack(archive, destination):
    destination.mkdir(parents=True)
    run('tar', '-xzf', str(archive), '-C', str(destination))
    app = destination / 'AgentInc.app'
    if not app.is_dir():
        raise RuntimeError(f'{archive} has no AgentInc.app')
    requirement = '=anchor apple generic and certificate leaf[subject.OU] = "X9E4HG27NK" and identifier "co.worldwidewebb.agentinc"'
    run('/usr/bin/codesign', '--verify', '--deep', '--strict', '-R', requirement, str(app))
    run('/usr/sbin/spctl', '--assess', '--type', 'execute', str(app))
    return app


def wait_for(directory, ready, seconds, description):
    fd = os.open(directory, os.O_RDONLY)
    queue = select.kqueue()
    event = select.kevent(fd, filter=select.KQ_FILTER_VNODE,
                          flags=select.KQ_EV_ADD | select.KQ_EV_CLEAR,
                          fflags=select.KQ_NOTE_WRITE)
    deadline = time.monotonic() + seconds
    try:
        while not ready():
            remaining = deadline - time.monotonic()
            if remaining <= 0 or not queue.control([event], 1, remaining):
                raise TimeoutError(description)
    finally:
        queue.close()
        os.close(fd)


def exercise(mode, candidate, newer, key, manifest_tool):
    with tempfile.TemporaryDirectory(prefix=f'agentinc-upgrade-{mode}-') as temporary:
        root = Path(temporary)
        app = unpack(candidate, root / 'install')
        with tempfile.TemporaryDirectory(dir=root) as serving:
            serving = Path(serving)
            shutil.copy2(newer, serving / 'AgentInc.tar.gz')
            handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=str(serving))
            server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), handler)
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            port = server.server_port
            next_app = unpack(newer, root / 'newer')
            identity = next_app / 'Contents/Resources/release.json'
            old_version = json.loads((app / 'Contents/Resources/release.json').read_text())['version']
            next_version = json.loads(identity.read_text())['version']
            if old_version == next_version:
                raise RuntimeError('fixture must be newer than candidate')
            (serving / 'notes.md').write_text('Upgrade gate fixture\n')
            (serving / 'changelog.md').write_text('Upgrade gate fixture\n')
            env = dict(os.environ, AINC_RELEASE_TEST_KEY='1',
                       UPDATE_SIGNING_KEY_ED25519_PEM=key.read_text())
            run(str(manifest_tool), str(identity), str(serving / 'AgentInc.tar.gz'),
                str(serving / 'notes.md'), str(serving / 'changelog.md'),
                f'http://127.0.0.1:{port}/AgentInc.tar.gz', str(serving / 'feed.json'), env=env)
            profile = root / 'profile'
            profile.mkdir()
            marker = root / 'relaunch.txt'
            app_env = dict(os.environ,
                           AINC_UPGRADE_TEST_FEED_URL=f'http://127.0.0.1:{port}/feed.json',
                           AINC_UPGRADE_TEST_MODE=mode,
                           AINC_UPGRADE_TEST_FROM=old_version,
                           AINC_UPGRADE_TEST_SUCCESS_FILE=str(marker),
                           AGENTINC_SESSION_PATH=str(profile / 'sessions.json'),
                           AINC_DISCOVERY_FILE=str(profile / 'daemon' / 'api-url'),
                           AINC_LEGACY_DIR=str(profile / 'legacy'),
                           AGENTINC_CODEX_HOME=str(profile / 'codex'))
            process = subprocess.Popen([str(app / 'Contents/MacOS/AgentInc')], env=app_env,
                                       stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
            try:
                _, errors = process.communicate(timeout=240)
                if process.returncode:
                    raise RuntimeError(f'{mode} candidate exited {process.returncode}: {errors.decode(errors="replace")[-4000:]}')
                wait_for(root, marker.is_file, 240, f'updated app did not relaunch: {marker}')
                version, pid_text = marker.read_text().split()
                if version != next_version:
                    raise RuntimeError(f'{mode} relaunched {version}, expected {next_version}')
                installed = json.loads((app / 'Contents/Resources/release.json').read_text())
                if installed['version'] != next_version:
                    raise RuntimeError(f'{mode} install did not replace bundle')
                backup = app.with_suffix('.previous.app')
                wait_for(app.parent, lambda: not backup.exists(), 120,
                         f'{mode} installer did not finish; see {profile / "updates" / "install.log"}')
                os.kill(int(pid_text), 0)
                print(f'PASS {mode}: {old_version} -> {next_version}, relaunch PID {pid_text}', flush=True)
            except Exception:
                log = profile / 'updates' / 'install.log'
                if log.is_file():
                    print(log.read_text(errors='replace')[-4000:], flush=True)
                raise
            finally:
                if process.poll() is None:
                    process.terminate()
                    process.wait(timeout=10)
                if marker.is_file():
                    os.kill(int(marker.read_text().split()[1]), 15)
                server.shutdown()
                thread.join()


def published_predecessors(candidate_version, root):
    # Shipped clients have the GitHub feed URL and production key compiled in.
    # Their candidate upgrade cannot be staged with a test key before publication.
    repo = os.environ['GITHUB_REPOSITORY']
    releases = []
    page = 1
    while True:
        batch = json.loads(subprocess.check_output(
            ['gh', 'api', f'repos/{repo}/releases?per_page=100&page={page}'], text=True))
        releases.extend(batch)
        if len(batch) < 100:
            break
        page += 1
    published = {}
    for release in releases:
        if release['draft'] or not release['tag_name'].startswith('v'):
            continue
        version = release['tag_name'][1:]
        if tuple(map(int, version.split('.'))) >= tuple(map(int, candidate_version.split('.'))):
            continue
        asset = next((a for a in release['assets'] if a['name'] == 'AgentInc.tar.gz'), None)
        if asset is None:
            raise RuntimeError(f'{version} has no published app archive')
        directory = root / version
        directory.mkdir()
        run('gh', 'release', 'download', release['tag_name'], '--pattern', 'AgentInc.tar.gz',
            '--dir', str(directory))
        app = unpack(directory / 'AgentInc.tar.gz', directory / 'installed')
        identity = json.loads((app / 'Contents/Resources/release.json').read_text())
        if identity['version'] != version:
            raise RuntimeError(f'published {version} contains {identity["version"]}')
        published[version] = identity['commit']
        reason = KNOWN_BROKEN.get(version)
        print(f'KNOWN BROKEN {version}: {reason}' if reason else
              f'SHIPPED FEED LOCKED {version}: signed install verified; test-key rebuild required for candidate upgrade',
              flush=True)
    return published


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--newer', type=Path, required=True)
    parser.add_argument('--key', type=Path, required=True)
    parser.add_argument('--manifest-tool', type=Path, required=True)
    parser.add_argument('--prior', type=Path, action='append', default=[])
    args = parser.parse_args()
    candidate = args.candidate.resolve()
    newer = args.newer.resolve()
    key = args.key.resolve()
    manifest_tool = args.manifest_tool.resolve()
    with tempfile.TemporaryDirectory(prefix='agentinc-published-') as temporary:
        with tempfile.TemporaryDirectory(prefix='agentinc-candidate-') as install:
            candidate_version = json.loads((unpack(candidate, Path(install)) /
                                            'Contents/Resources/release.json').read_text())['version']
        published = published_predecessors(candidate_version, Path(temporary))
    for prior in args.prior:
        prior = prior.resolve()
        with tempfile.TemporaryDirectory(prefix='agentinc-prior-') as install:
            identity = json.loads((unpack(prior, Path(install)) /
                                   'Contents/Resources/release.json').read_text())
        if published.get(identity['version']) != identity['commit']:
            raise RuntimeError(f'prior test build does not match published {identity["version"]}')
        for mode in ('manual', 'automatic'):
            exercise(mode, prior, candidate, key, manifest_tool)
    for mode in ('manual', 'automatic'):
        exercise(mode, candidate, newer, key, manifest_tool)


if __name__ == '__main__':
    main()
