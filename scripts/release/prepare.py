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
    parser.add_argument('--upload', action='store_true', help='Hand the artifact to Linux Distribution through a draft release')
    parser.add_argument('--profile', choices=['debug', 'release'], default='release')
    args = parser.parse_args()
    if platform.system() != 'Darwin':
        raise SystemExit('GPUI/Metal requires the macOS SDK: run cargo xtask release on a Mac.')
    os.chdir(ROOT)
    if run('git', 'status', '--porcelain', '--untracked-files=no'):
        raise SystemExit('Commit tracked changes before preparing a release handoff')
    metadata = json.loads(run('cargo', 'metadata', '--no-deps', '--format-version=1'))
    product = next(p for p in metadata['packages'] if p['name'] == 'ainc-release')
    version = product['version']
    commit = run('git', 'rev-parse', 'HEAD')
    build = run('git', 'rev-list', '--count', 'HEAD')
    env = dict(os.environ, AINC_BUILD_ID=build, AINC_CHANNEL='production', AINC_COMMIT=commit, CARGO_INCREMENTAL='0')
    command = ['cargo', 'build', '--locked', '-p', 'agentinc-os', '-p', 'ainc-daemon', '-p', 'ainc-release', '-p', 'ainc-cli', '--bins']
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
    for binary in ['agentinc-os', 'aincd', 'ainc-update', 'ainc']:
        shutil.copy2(target / binary, macos / ('AgentInc' if binary == 'agentinc-os' else binary))
    # The already-installed 0.1.0 updater launches this old path after replacement.
    (macos / 'agentinc-os').symlink_to('AgentInc')
    shutil.copy2(ROOT / 'crates/ainc-mac/assets/AppIcon.icns', resources / 'AppIcon.icns')
    runtime = resources / 'runtime'
    runtime.mkdir()
    # Portable by construction; never relocate a developer's Homebrew install.
    if platform.machine() != 'arm64':
        raise SystemExit('This release currently supports Apple Silicon only')
    pg_version = '16.15.0'
    pg_name = f'postgresql-{pg_version}-aarch64-apple-darwin'
    pg_sha256 = '46f6382024d9b633d1f4b4903ffef8c2404e00ae83098d628c00591048cb0512'
    cache = ROOT / '.local/release-inputs'
    cache.mkdir(parents=True, exist_ok=True)
    pg_archive = cache / (pg_name + '.tar.gz')
    if not pg_archive.exists():
        subprocess.run(['curl', '--fail', '--location', '--output', str(pg_archive),
            f'https://github.com/theseus-rs/postgresql-binaries/releases/download/{pg_version}/{pg_name}.tar.gz'], check=True)
    if hashlib.sha256(pg_archive.read_bytes()).hexdigest() != pg_sha256:
        raise SystemExit('portable Postgres checksum mismatch')
    import tempfile
    with tempfile.TemporaryDirectory(dir=cache) as unpack:
        # The archive is pinned by SHA-256 above. macOS 15's Python 3.9 does
        # not yet support tarfile's filter= argument.
        subprocess.run(['tar', '-xzf', str(pg_archive), '-C', unpack], check=True)
        pg = Path(unpack) / pg_name
        destination = runtime / 'postgres'
        (destination / 'bin').mkdir(parents=True)
        for binary in ['postgres', 'initdb']:
            shutil.copy2(pg / 'bin' / binary, destination / 'bin' / binary)
        shutil.copytree(pg / 'share', destination / 'share', symlinks=True)
        shutil.copytree(pg / 'lib', destination / 'lib', symlinks=True,
            ignore=shutil.ignore_patterns('pgxs', '*.a', '*.pc'))
        for license_file in ['LICENSE', 'COPYRIGHT', 'README.md']:
            shutil.copy2(pg / license_file, destination / license_file)
    for binary in ['temporal', 'codex']:
        source = shutil.which(binary)
        if not source:
            raise SystemExit(f'{binary} binary required for the self-contained bundle')
        shutil.copy2(Path(source).resolve(), runtime / binary)
    # Audit every Mach-O, including dylibs. A packaging regression fails before
    # the artifact reaches signing; no machine-local paths are tolerated.
    for binary in bundle.rglob('*'):
        if not binary.is_file() or not run('file', '-b', str(binary)).startswith('Mach-O'):
            continue
        for line in run('otool', '-L', str(binary)).splitlines()[1:]:
            dependency = line.strip().split(' (compatibility')[0]
            if not dependency.startswith(('/usr/lib/', '/System/', '@loader_path/', '@rpath/')):
                raise SystemExit(f'nonportable dependency {dependency} in {binary}')
            if dependency.startswith('@loader_path/'):
                dependency_path = (binary.parent / dependency.removeprefix('@loader_path/')).resolve()
                if not dependency_path.is_relative_to(bundle) or not dependency_path.is_file():
                    raise SystemExit(f'unresolved bundle dependency {dependency} in {binary}')
    identity = json.loads(run(str(target / 'ainc-release-manifest'), '--identity'))
    identity.update(commit=commit, architecture=platform.machine().replace('arm64','aarch64'))
    if identity['version'] != version or identity['build'] != build:
        raise SystemExit('compiled product identity differs from Cargo metadata/build input')
    (resources / 'release.json').write_text(json.dumps(identity, indent=2) + '\n')
    plist = dict(CFBundleName='AgentInc', CFBundleDisplayName='AgentInc', CFBundleIdentifier='co.worldwidewebb.agentinc', CFBundleExecutable='AgentInc', CFBundleIconFile='AppIcon', CFBundlePackageType='APPL', CFBundleShortVersionString=version, CFBundleVersion=build, NSHumanReadableCopyright='Copyright © 2026 Calum Webb', LSMinimumSystemVersion='15.0', NSHighResolutionCapable=True, NSPrincipalClass='NSApplication')
    (contents / 'Info.plist').write_bytes(plistlib.dumps(plist))
    # The runtime input inventory makes local build provenance reviewable.
    inventory = {str(p.relative_to(bundle)): hashlib.sha256(p.read_bytes()).hexdigest() for p in bundle.rglob('*') if p.is_file()}
    (out / 'handoff.json').write_text(json.dumps(dict(**identity, files=inventory), indent=2) + '\n')
    archive = out / 'unsigned.tar.gz'
    with tarfile.open(archive, 'w:gz') as tar:
        tar.add(bundle, arcname='AgentInc.app')
        tar.add(out / 'handoff.json', arcname='handoff.json')
    print(archive)
    if args.upload:
        tag = 'build-' + commit
        existing = subprocess.run(['gh-axi', 'release', 'view', tag], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        if existing.returncode:
            subprocess.run(['gh-axi', 'release', 'create', tag, '--draft', '--target', commit,
                '--title', f'AgentInc {version} build input', '--notes', 'Native signing input. Do not publish.'], check=True)
        subprocess.run(['gh-axi', 'release', 'upload', tag, str(archive), '--clobber'], check=True)
        testing = run('git', 'branch', '--show-current') != 'main'
        subprocess.run(['gh-axi', 'workflow', 'run', 'release.yml', '--ref', 'main',
            '-f', f'commit={commit}', '-f', 'test=' + str(testing).lower(),
            '-f', 'build=false'], check=True)

if __name__ == '__main__':
    main()
