#!/usr/bin/env python3
"""Build the macOS-only GPUI application, then stage portable signing input.
No credentials needed. The Linux release job signs every nested Mach-O.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import plistlib
import shutil
import subprocess
import tarfile

ROOT = Path(__file__).resolve().parents[2]
def run(*args, **kwargs):
    return subprocess.check_output(args, text=True, **kwargs).strip()

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--profile', choices=['debug', 'release'], default='release')
    args = parser.parse_args()
    if platform.system() != 'Darwin':
        raise SystemExit('GPUI/Metal requires the macOS SDK: run cargo xtask release on a Mac. CI remains Linux-only.')
    os.chdir(ROOT)
    metadata = json.loads(run('cargo', 'metadata', '--no-deps', '--format-version=1'))
    product = next(p for p in metadata['packages'] if p['name'] == 'ainc-release')
    version = product['version']
    commit = run('git', 'rev-parse', 'HEAD')
    build = run('git', 'rev-list', '--count', 'HEAD')
    env = dict(os.environ, AINC_BUILD_ID=build, CARGO_INCREMENTAL='0')
    command = ['cargo', 'build', '--locked', '-p', 'agentinc-os', '-p', 'ainc-daemon', '-p', 'ainc-release', '--bins']
    if args.profile == 'release':
        command.append('--release')
    subprocess.run(command, check=True, env=env)
    out = ROOT / '.local/release' / commit
    out.mkdir(parents=True, exist_ok=True)
    bundle = out / 'AgentInc.app'
    if bundle.exists():
        shutil.rmtree(bundle)
    contents = bundle / 'Contents'
    macos = contents / 'MacOS'
    resources = contents / 'Resources'
    macos.mkdir(parents=True)
    resources.mkdir()
    target = Path(metadata['target_directory']) / args.profile
    for binary in ['agentinc-os', 'aincd', 'ainc-update']:
        shutil.copy2(target / binary, macos / binary)
    shutil.copy2(ROOT / 'crates/ainc-mac/assets/AppIcon.icns', resources / 'AppIcon.icns')
    runtime = resources / 'runtime'
    runtime.mkdir()
    # Versioned runtime inputs are pinned by their hashes in the handoff record.
    pg = Path(run('brew', '--prefix', 'postgresql@16'))
    (runtime / 'postgres/bin').mkdir(parents=True)
    for binary in ['postgres', 'initdb']:
        shutil.copy2(pg / 'bin' / binary, runtime / 'postgres/bin' / binary)
    shutil.copytree(pg / 'share/postgresql@16', runtime / 'postgres/share', symlinks=False)
    shutil.copytree(pg / 'lib/postgresql', runtime / 'postgres/lib/postgresql', symlinks=False)
    for binary in ['temporal', 'codex']:
        source = shutil.which(binary)
        if not source:
            raise SystemExit(f'{binary} binary required for the self-contained bundle')
        shutil.copy2(Path(source).resolve(), runtime / binary)
    # Rewrite every non-system dylib dependency into this bundle. This includes
    # transitive Homebrew dependencies; fresh Macs never resolve /opt/homebrew.
    framework = contents / 'Frameworks'
    framework.mkdir()
    queue = [p for p in bundle.rglob('*') if p.is_file() and run('file', '-b', str(p)).startswith('Mach-O')]
    copied = {}
    checked = set()
    while queue:
        binary = queue.pop()
        if binary in checked:
            continue
        checked.add(binary)
        deps = run('otool', '-L', str(binary)).splitlines()[1:]
        for line in deps:
            dependency = line.strip().split(' (compatibility')[0]
            if dependency.startswith(('/usr/lib/', '/System/Library/', '@loader_path/')):
                continue
            if dependency.startswith('@'):
                raise SystemExit(f'unresolved dynamic library {dependency} in {binary}')
            source = Path(dependency).resolve()
            if source == binary.resolve():
                continue
            if not source.is_file():
                raise SystemExit(f'missing dynamic library {source}')
            if source not in copied:
                name = hashlib.sha256(str(source).encode()).hexdigest()[:8] + '-' + source.name
                destination = framework / name
                shutil.copy2(source, destination)
                copied[source] = destination
                subprocess.run(['install_name_tool', '-id', '@loader_path/' + name, str(destination)], check=True)
                queue.append(destination)
            destination = copied[source]
            relative = os.path.relpath(destination, binary.parent)
            subprocess.run(['install_name_tool', '-change', dependency, '@loader_path/' + relative, str(binary)], check=True)
    identity = dict(version=version, build=build, commit=commit, architecture=platform.machine().replace('arm64','aarch64'), api=1, minimum_client='0.1.0', schema=1)
    (resources / 'release.json').write_text(json.dumps(identity, indent=2) + '\n')
    plist = dict(CFBundleName='AgentInc', CFBundleDisplayName='AgentInc', CFBundleIdentifier='co.worldwidewebb.agentinc', CFBundleExecutable='agentinc-os', CFBundleIconFile='AppIcon', CFBundlePackageType='APPL', CFBundleShortVersionString=version, CFBundleVersion=build, LSMinimumSystemVersion='12.0', NSHighResolutionCapable=True, NSPrincipalClass='NSApplication')
    (contents / 'Info.plist').write_bytes(plistlib.dumps(plist))
    # The runtime input inventory makes local build provenance reviewable.
    inventory = {str(p.relative_to(bundle)): hashlib.sha256(p.read_bytes()).hexdigest() for p in bundle.rglob('*') if p.is_file()}
    (out / 'handoff.json').write_text(json.dumps(dict(**identity, files=inventory), indent=2) + '\n')
    archive = out / 'unsigned.tar.gz'
    with tarfile.open(archive, 'w:gz') as tar:
        tar.add(bundle, arcname='AgentInc.app')
        tar.add(out / 'handoff.json', arcname='handoff.json')
    print(archive)

if __name__ == '__main__':
    main()
