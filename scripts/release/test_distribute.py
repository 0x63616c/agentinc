import hashlib
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import threading
import unittest
from unittest.mock import patch
import distribute


class DistributionTest(unittest.TestCase):
    def test_four_bundles_and_manifest_build_overlap_without_timers(self):
        ready = threading.Barrier(5)
        completed = []

        def task(name):
            ready.wait(timeout=10)
            completed.append(name)

        distribute.sign_all([lambda n=n: task(n) for n in range(4)], lambda: task('manifest'))
        self.assertCountEqual(completed, [0, 1, 2, 3, 'manifest'])

    def test_every_fixture_and_manifest_failure_propagates(self):
        def fail():
            raise RuntimeError('failure')

        for tasks, build in (([lambda: None, fail], lambda: None), ([lambda: None], fail)):
            with self.assertRaisesRegex(RuntimeError, 'failure'):
                distribute.sign_all(tasks, build)

    def test_notary_wait_and_staple_are_still_required(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            bundle = root / 'AgentInc.app'
            bundle.mkdir()
            with patch.dict(os.environ, APPLE_TEAM_ID='team'), patch(
                    'distribute.subprocess.run') as command:
                distribute.sign_bundle(bundle, root / 'signed.tar.gz', root)
            self.assertEqual(command.call_args_list[1].args[0][:2], ['rcodesign', 'notary-submit'])
            self.assertIn('--wait', command.call_args_list[1].args[0])
            self.assertIn('--staple', command.call_args_list[1].args[0])
            self.assertTrue((root / 'signed.tar.gz').is_file())
            with patch.dict(os.environ, APPLE_TEAM_ID='team'), patch(
                    'distribute.subprocess.run', side_effect=[None, subprocess.CalledProcessError(1, 'notary')]):
                with self.assertRaises(subprocess.CalledProcessError):
                    distribute.sign_bundle(bundle, root / 'failed.tar.gz', root)
            self.assertFalse((root / 'failed.tar.gz').exists())

    def test_fixture_identity_and_inventory_are_checked_before_signing(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            app = root / 'AgentInc.app'
            app.mkdir()
            (app / 'binary').write_bytes(b'fixture')
            valid = dict(commit='a' * 40, version='0.3.5', upgrade_test=True,
                         files={'binary': hashlib.sha256(b'fixture').hexdigest()})
            for override in ({}, {'commit': 'b' * 40}, {'version': '0.3.3'},
                             {'upgrade_test': False}, {'files': {}}):
                identity = dict(valid, **override)
                (root / 'handoff.json').write_text(json.dumps(identity))
                source = root / 'fixture.tar.gz'
                with tarfile.open(source, 'w:gz') as archive:
                    archive.add(app, arcname='AgentInc.app')
                    archive.add(root / 'handoff.json', arcname='handoff.json')
                with patch('distribute.sign_bundle') as sign:
                    if override:
                        with self.assertRaises(SystemExit):
                            distribute.sign_upgrade_fixture(source, root / 'signed.tar.gz', root,
                                                            valid['commit'], valid['version'])
                        sign.assert_not_called()
                    else:
                        distribute.sign_upgrade_fixture(source, root / 'signed.tar.gz', root,
                                                        valid['commit'], valid['version'])
                        sign.assert_called_once()

    def test_ci_failure_prevents_assets_upload_and_publication(self):
        # Exercise main, not merely the gate helper: speculative signing can
        # finish, but no feed/archive becomes available before complete CI.
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            commit = 'a' * 40
            app = root / 'AgentInc.app'
            app.mkdir()
            identity = dict(commit=commit, version='0.3.5', upgrade_test=False, files={})
            (root / 'handoff.json').write_text(json.dumps(identity))
            source = root / 'source.tar.gz'
            with tarfile.open(source, 'w:gz') as archive:
                archive.add(app, arcname='AgentInc.app')
                archive.add(root / 'handoff.json', arcname='handoff.json')

            def output(*args, **kwargs):
                if args[:2] == ('git', 'rev-parse'):
                    return commit
                if args[:2] == ('cargo', 'metadata'):
                    return json.dumps(dict(packages=[dict(name='ainc-release', version='0.3.5')],
                                           target_directory=str(root / 'target')))
                if args[:2] == ('gh', 'api'):
                    if 'generate-notes' in args[2]:
                        return json.dumps(dict(body='Release notes'))
                    return '[]'
                return ''

            environment = dict(GITHUB_REF='refs/heads/main', GITHUB_REPOSITORY='owner/repo',
                               UPDATE_SIGNING_KEY_ED25519_PEM='private', APPLE_DEVELOPER_ID_P12_BASE64='YQ==',
                               APPLE_DEVELOPER_ID_P12_PASSWORD='password', NOTARY_KEY_P8='private',
                               NOTARY_ISSUER_ID='issuer', NOTARY_KEY_ID='key')
            original = Path.cwd()
            try:
                os.chdir(root)
                with patch.dict(os.environ, environment), patch('sys.argv', [
                        'distribute.py', '--commit', commit, '--archive', str(source)]), patch(
                        'distribute.run', side_effect=output), patch(
                        'distribute.subprocess.run', return_value=subprocess.CompletedProcess([], 1)) as command, patch(
                        'distribute.sign_all') as signing, patch(
                        'distribute.wait_for_ci', side_effect=RuntimeError('CI failed')):
                    with self.assertRaisesRegex(RuntimeError, 'CI failed'):
                        distribute.main()
                    signing.assert_called_once()
                commands = [call.args[0] for call in command.call_args_list]
                self.assertFalse(any(cmd[:3] == ['gh', 'release', 'upload'] for cmd in commands))
                self.assertFalse(any('--draft=false' in cmd for cmd in commands))
            finally:
                os.chdir(original)


if __name__ == '__main__':
    unittest.main()
