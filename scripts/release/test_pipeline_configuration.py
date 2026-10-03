"""Regressions for the required-status and rolling-cache contracts."""
import os
from pathlib import Path
import re
import subprocess
import unittest


ROOT = Path(__file__).resolve().parents[2]


class PipelineConfigurationTest(unittest.TestCase):
    def test_required_rust_aggregate_only_accepts_all_success(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        aggregate = workflow.split('\n  rust:\n', 1)[1]
        self.assertIn('needs: [checks, workspace]', aggregate)
        self.assertIn('if: always()', aggregate)
        command = re.search(r'^        run: (.+)$', aggregate, re.MULTILINE).group(1)
        for checks in ('success', 'failure', 'cancelled', 'skipped'):
            for workspace in ('success', 'failure', 'cancelled', 'skipped'):
                result = subprocess.run(['bash', '-c', command],
                                        env=dict(os.environ, CHECKS=checks, WORKSPACE=workspace))
                self.assertEqual(result.returncode == 0,
                                 checks == workspace == 'success', (checks, workspace))

    def test_cache_primary_keys_roll_but_restore_prefixes_do_not(self):
        for name in ('ci', 'release'):
            workflow = (ROOT / f'.github/workflows/{name}.yml').read_text()
            restore = workflow.split('uses: actions/cache/restore@v4', 1)[1]
            primary = re.search(r'^          key: (.+)$', restore, re.MULTILINE).group(1)
            self.assertIn('github.sha', primary)
            prefixes = re.search(r'          restore-keys: \|\n((?:            .+\n)+)', restore).group(1)
            self.assertNotIn('github.sha', prefixes)
            self.assertNotIn('inputs.commit', prefixes)
            save = workflow.split('uses: actions/cache/save@v4', 1)[1]
            self.assertIn("if: success() && github.ref == 'refs/heads/main'", save)
            self.assertIn('steps.cache.outputs.cache-primary-key', save)


if __name__ == '__main__':
    unittest.main()
