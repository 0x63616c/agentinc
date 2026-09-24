#!/usr/bin/env python3
"""Only a product version change on main starts a release."""
import subprocess
import sys
import tomllib
from pathlib import Path
path = 'crates/ainc-release/Cargo.toml'
current = tomllib.loads(Path(path).read_text())['package']['version']
previous = subprocess.run(['git', 'show', sys.argv[1] + ':' + path], capture_output=True, text=True)
old = tomllib.loads(previous.stdout)['package']['version'] if previous.returncode == 0 else None
print('true' if old != current else 'false')
