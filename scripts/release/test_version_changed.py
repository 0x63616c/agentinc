"""Release trigger checks for the product version, including the old manifest layout."""
import subprocess
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name('version-changed.py')


class VersionChangedTest(unittest.TestCase):
    def test_only_product_version_starts_release(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)

            def git(*args):
                return subprocess.check_output(['git', *args], cwd=root, text=True).strip()

            def commit():
                git('add', '.')
                git('commit', '-qm', 'snapshot')
                return git('rev-parse', 'HEAD')

            def changed(before):
                return subprocess.check_output(
                    ['python3', str(SCRIPT), before], cwd=root, text=True
                ).strip()

            git('init', '-q')
            git('config', 'user.name', 'Release Test')
            git('config', 'user.email', 'release-test@example.com')
            root.joinpath('Cargo.toml').write_text('[workspace.package]\nversion = "0.2.0"\n')
            old = commit()

            # SDK or other changes must not start Distribution.
            root.joinpath('Cargo.toml').write_text(
                '[workspace.package]\nversion = "0.2.0"\n[workspace.dependencies]\nturnkeel = "0.3.0"\n'
            )
            self.assertEqual(changed(old), 'false')
            unchanged = commit()

            root.joinpath('Cargo.toml').write_text('[workspace.package]\nversion = "0.3.0"\n')
            self.assertEqual(changed(unchanged), 'true')

    def test_layout_migration_does_not_start_release(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)

            def git(*args):
                return subprocess.check_output(['git', *args], cwd=root, text=True).strip()

            git('init', '-q')
            git('config', 'user.name', 'Release Test')
            git('config', 'user.email', 'release-test@example.com')
            root.joinpath('Cargo.toml').write_text('[workspace.package]\nedition = "2024"\n')
            legacy = root.joinpath('crates/ainc-release/Cargo.toml')
            legacy.parent.mkdir(parents=True)
            legacy.write_text('[package]\nversion = "0.2.0"\n')
            git('add', '.')
            git('commit', '-qm', 'legacy')
            before = git('rev-parse', 'HEAD')

            root.joinpath('Cargo.toml').write_text('[workspace.package]\nversion = "0.2.0"\n')
            legacy.write_text('[package]\nversion.workspace = true\n')
            result = subprocess.check_output(
                ['python3', str(SCRIPT), before], cwd=root, text=True
            ).strip()
            self.assertEqual(result, 'false')


if __name__ == '__main__':
    unittest.main()
