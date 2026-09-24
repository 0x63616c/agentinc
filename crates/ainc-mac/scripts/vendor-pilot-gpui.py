#!/usr/bin/env python3
"""Reproduce the pinned GPUI source plus the opt-in pilot seam (no cache edits)."""
import json
import pathlib
import shutil
import subprocess
import sys
import tomllib

REV = '4c902c9db22a82f5f3a14c02442e7f60ec40d9c8'
source = pathlib.Path(sys.argv[1]).resolve()
assert subprocess.check_output(['git', '-C', str(source), 'rev-parse', 'HEAD'], text=True).strip() == REV
root = pathlib.Path(__file__).resolve().parent.parent
dest = root / 'vendor/gpui'
workspace = tomllib.loads((source / 'Cargo.toml').read_text())
crate = source / 'crates/gpui'
manifest = tomllib.loads((crate / 'Cargo.toml').read_text())
manifest['package']['edition'] = workspace['workspace']['package']['edition']
manifest['package'].update(publish=False, autoexamples=False, autotests=False, autobenches=False)
manifest['lints'] = {'rust': {'unexpected_cfgs': {'level': 'warn', 'check-cfg': ['cfg(rust_analyzer)']}}}
manifest.pop('example', None)
manifest['features']['pilot'] = []

def resolve(table):
    for key, value in list(table.items()):
        if key.endswith('dependencies'):
            for name, dep in list(value.items()):
                if isinstance(dep, dict) and dep.get('workspace'):
                    base = workspace['workspace']['dependencies'][name]
                    base = {'version': base} if isinstance(base, str) else base.copy()
                    overrides = {k: v for k, v in dep.items() if k != 'workspace'}
                    if 'features' in overrides:
                        overrides['features'] = list(dict.fromkeys(base.get('features', []) + overrides['features']))
                    base.update(overrides)
                    if 'path' in base:
                        base.pop('path')
                        base.update(git='https://github.com/zed-industries/zed', rev=REV)
                    value[name] = base
        elif isinstance(value, dict):
            resolve(value)
resolve(manifest)
# Dependency tests/examples are not part of the application build.
manifest.pop('dev-dependencies', None)
for table in manifest.get('target', {}).values():
    table.pop('dev-dependencies', None)

def literal(value):
    if isinstance(value, dict):
        return '{ ' + ', '.join(json.dumps(k) + ' = ' + literal(v) for k, v in value.items()) + ' }'
    return json.dumps(value)

def emit(table, prefix=()):
    lines = []
    scalars = {k: v for k, v in table.items() if not isinstance(v, dict)}
    if prefix:
        lines += ['[' + '.'.join(json.dumps(p) for p in prefix) + ']']
    lines += [json.dumps(k) + ' = ' + literal(v) for k, v in scalars.items()]
    for key, value in table.items():
        if isinstance(value, dict):
            lines += emit(value, (*prefix, key))
    return lines + ['']

dest.mkdir(parents=True, exist_ok=True)
for name in ['src', 'resources']:
    shutil.copytree(crate / name, dest / name, dirs_exist_ok=True)
for name in ['build.rs', 'README.md']:
    shutil.copy2(crate / name, dest / name)
shutil.copy2(source / 'LICENSE-APACHE', dest / 'LICENSE-APACHE')
(dest / 'Cargo.toml').write_text('\n'.join(emit(manifest)))
patch = root / 'vendor/gpui-pilot.patch'
if patch.exists():
    subprocess.run(['git', 'apply', '--directory=vendor/gpui', str(patch)], cwd=root, check=True)
