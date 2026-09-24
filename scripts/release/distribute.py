#!/usr/bin/env python3
"""Linux signing/notarization and idempotent GitHub distribution.
Only main may publish. Test jobs leave a draft; credentials never enter arguments.
"""
import argparse
import base64
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import time


def run(*args, **kwargs):
    return subprocess.check_output(args, text=True, **kwargs).strip()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--commit', required=True)
    parser.add_argument('--test', action='store_true')
    parser.add_argument('--archive', type=Path, help='Unsigned artifact from this workflow run')
    args = parser.parse_args()
    if not args.test and not os.environ.get('UPDATE_SIGNING_KEY_ED25519_PEM', '').strip():
        raise SystemExit('publish refused: UPDATE_SIGNING_KEY_ED25519_PEM is missing')
    if not args.test and os.environ.get('GITHUB_REF') != 'refs/heads/main':
        raise SystemExit('publish refused: only main may publish')
    commit = run('git', 'rev-parse', args.commit + '^{commit}')
    if commit != args.commit or run('git', 'rev-parse', 'HEAD') != commit:
        raise SystemExit('release checkout does not match requested commit')
    repo = os.environ['GITHUB_REPOSITORY']
    while True:
        checks = json.loads(run('gh', 'api', f'repos/{repo}/commits/{commit}/check-runs'))['check_runs']
        rust = [check for check in checks if check['name'] == 'rust']
        if any(check['conclusion'] == 'success' for check in rust):
            break
        if rust and all(check['status'] == 'completed' for check in rust):
            raise SystemExit('release refused: workspace CI failed for this exact commit')
        print('Waiting for workspace CI on the release commit', flush=True)
        time.sleep(15)
    if not args.test:
        subprocess.run(['git', 'merge-base', '--is-ancestor', commit, 'origin/main'], check=True)
    handoff_tag = 'build-' + commit
    out = Path('.local/distribution').resolve()
    out.mkdir(parents=True, exist_ok=True)
    if args.archive:
        if args.archive.resolve() != out / 'unsigned.tar.gz':
            shutil.copy2(args.archive, out / 'unsigned.tar.gz')
    else:
        subprocess.run(['gh', 'release', 'download', handoff_tag, '--pattern', 'unsigned.tar.gz', '--dir', str(out), '--clobber'], check=True)
    with tarfile.open(out / 'unsigned.tar.gz') as tar:
        tar.extractall(out, filter='data')
    identity = json.loads((out / 'handoff.json').read_text())
    if identity['commit'] != commit:
        raise SystemExit('handoff commit mismatch')
    metadata = json.loads(run('cargo', 'metadata', '--no-deps', '--format-version=1'))
    version = next(p['version'] for p in metadata['packages'] if p['name'] == 'ainc-release')
    if identity['version'] != version:
        raise SystemExit('handoff version mismatch')
    import hashlib
    bundle = out / 'AgentInc.app'
    files = {str(p.relative_to(bundle)): hashlib.sha256(p.read_bytes()).hexdigest() for p in bundle.rglob('*') if p.is_file()}
    if files != identity['files']:
        raise SystemExit('handoff inventory mismatch')
    tag = 'phase5-test-' + commit[:12] if args.test else 'v' + version
    existing = subprocess.run(['gh', 'release', 'view', tag, '--json', 'targetCommitish,isDraft,body'], capture_output=True, text=True)
    if existing.returncode == 0:
        release = json.loads(existing.stdout)
        if release['targetCommitish'] != commit:
            raise SystemExit('release version already belongs to another commit')
        if not release['isDraft']:
            print('Release already published for this commit')
            return
        notes = release['body']
    else:
        notes = json.loads(run('gh', 'api', f'repos/{repo}/releases/generate-notes', '-f', f'tag_name={tag}', '-f', f'target_commitish={commit}'))['body']
        notes_path = out / 'notes.md'
        notes_path.write_text(notes)
        subprocess.run(['gh', 'release', 'create', tag, '--draft', '--target', commit, '--title', f'AgentInc {version}', '--notes-file', str(notes_path)], check=True)
    # Notes are generated once, saved in the draft, then reused on every retry.
    (out / 'notes.md').write_text(notes)
    releases = json.loads(run('gh', 'api', f'repos/{repo}/releases?per_page=100'))
    history = '\n\n'.join(r['body'] or '' for r in releases if not r['draft'])
    (out / 'changelog.md').write_text(notes + '\n\n' + history)
    with tempfile.TemporaryDirectory() as private:
        private = Path(private)
        os.chmod(private, 0o700)
        (private / 'identity.p12').write_bytes(base64.b64decode(os.environ['APPLE_DEVELOPER_ID_P12_BASE64']))
        (private / 'password').write_text(os.environ['APPLE_DEVELOPER_ID_P12_PASSWORD'])
        (private / 'notary.p8').write_text(os.environ['NOTARY_KEY_P8'])
        # rcodesign uses the modern Notary API and runs on Linux.
        run('rcodesign', 'encode-app-store-connect-api-key', os.environ['NOTARY_ISSUER_ID'], os.environ['NOTARY_KEY_ID'], str(private / 'notary.p8'), '--output-path', str(private / 'notary.json'))
        subprocess.run(['rcodesign', 'sign', '--p12-file', str(private / 'identity.p12'), '--p12-password-file', str(private / 'password'), '--team-name', os.environ['APPLE_TEAM_ID'], '--for-notarization', str(bundle)], check=True)
        subprocess.run(['rcodesign', 'notary-submit', '--api-key-file', str(private / 'notary.json'), '--wait', '--staple', str(bundle)], check=True)
        archive = out / 'AgentInc.tar.gz'
        with tarfile.open(archive, 'w:gz') as tar:
            tar.add(bundle, arcname='AgentInc.app')
        env = dict(os.environ)
        env.pop('AINC_RELEASE_TEST_KEY', None)
        if args.test:
            env['AINC_RELEASE_TEST_KEY'] = '1'
            # Throwaway key, never used by the production channel.
            run('openssl', 'genpkey', '-algorithm', 'ED25519', '-out', str(private / 'update.pem'))
            env['UPDATE_SIGNING_KEY_ED25519_PEM'] = (private / 'update.pem').read_text()
            public = subprocess.check_output(['openssl', 'pkey', '-in', str(private / 'update.pem'), '-pubout', '-outform', 'DER'])[-32:]
            (out / 'test-public-key.txt').write_text(base64.b64encode(public).decode() + '\n')
        url = f'https://github.com/{repo}/releases/download/{tag}/AgentInc.tar.gz'
        subprocess.run(['cargo', 'run', '--locked', '-p', 'ainc-release', '--bin', 'ainc-release-manifest', '--', str(bundle / 'Contents/Resources/release.json'), str(archive), str(out / 'notes.md'), str(out / 'changelog.md'), url, str(out / 'feed.json')], check=True, env=env)
    assets = [str(out / p) for p in ['AgentInc.tar.gz', 'feed.json', 'notes.md', 'changelog.md']]
    if args.test:
        assets.append(str(out / 'test-public-key.txt'))
    subprocess.run(['gh', 'release', 'upload', tag, *assets, '--clobber'], check=True)
    if not args.test:
        subprocess.run(['gh', 'release', 'edit', tag, '--draft=false', '--latest'], check=True)
    print('Draft validated' if args.test else 'Published', tag)

if __name__ == '__main__':
    main()
