#!/usr/bin/env python3
"""Check the signed bundle's real terminal attach process through a PTY."""
import argparse
import fcntl
import os
from pathlib import Path
import pty
import select
import struct
import subprocess
import termios
import time
import uuid


def read_until(master, process, predicate, description):
    output = bytearray()
    deadline = time.monotonic() + 20
    while not predicate(output):
        if process.poll() is not None:
            raise RuntimeError(f'{description}: attach exited {process.returncode}: {output[-500:]!r}')
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise TimeoutError(f'{description}: {output[-500:]!r}')
        if select.select([master], [], [], remaining)[0]:
            output.extend(os.read(master, 8192))
    return bytes(output)


def check_pane(app, discovery, existing):
    session_id = str(uuid.uuid4())
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 42, 80, 0, 0))
    command = [str(app / 'Contents/MacOS/aincd'), '--terminal-attach', session_id]
    if existing:
        command.append('--existing')
    process = subprocess.Popen(
        command, stdin=slave, stdout=slave, stderr=slave,
        env=dict(os.environ, AINC_DISCOVERY_FILE=str(discovery)), start_new_session=True)
    os.close(slave)
    try:
        # The shell's first prompt has no newline. This catches a buffered
        # stdout writer that leaves a pane showing "Connecting" indefinitely.
        startup = read_until(master, process,
                             lambda data: b'\x1bc' in data and bool(data.split(b'\x1bc', 1)[1]),
                             'initial terminal output')
        marker = f'__AGENTINC_TERMINAL_{session_id.replace("-", "")}__'.encode()
        os.write(master, b"printf '" + marker + b"\\n'\n")
        result = startup + read_until(master, process,
                                      lambda data: data.count(marker) >= 2,
                                      'terminal command output')
        if result.count(marker) < 2:
            raise RuntimeError('terminal command did not run')
        print(f'PASS terminal {"restored" if existing else "new"}: {session_id}', flush=True)
    finally:
        process.terminate()
        process.wait(timeout=10)
        os.close(master)


def check(app, profile):
    discovery = (profile / 'daemon/api-url').resolve()
    if not discovery.is_file():
        raise RuntimeError(f'terminal daemon discovery missing: {discovery}')
    check_pane(app, discovery, existing=False)
    # A saved pane from the old daemon has an ID but no live PTY after update.
    check_pane(app, discovery, existing=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--app', type=Path, required=True)
    parser.add_argument('--profile', type=Path, required=True)
    args = parser.parse_args()
    check(args.app.resolve(), args.profile.resolve())
