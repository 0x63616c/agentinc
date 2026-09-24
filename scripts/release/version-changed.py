#!/usr/bin/env python3
"""Only a product version change on main starts a release."""
import subprocess
import sys
import tomllib
from pathlib import Path

workspace_path = 'Cargo.toml'
legacy_path = 'crates/ainc-release/Cargo.toml'
current = tomllib.loads(Path(workspace_path).read_text())['workspace']['package']['version']


def previous_version(path, keys):
    previous = subprocess.run(['git', 'show', sys.argv[1] + ':' + path], capture_output=True, text=True)
    if previous.returncode != 0:
        return None
    data = tomllib.loads(previous.stdout)
    for key in keys:
        data = data.get(key, {})
    return data if isinstance(data, str) else None


old = previous_version(workspace_path, ('workspace', 'package', 'version'))
if old is None:
    old = previous_version(legacy_path, ('package', 'version'))
print('true' if old != current else 'false')
