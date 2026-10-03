"""Regressions for the required-status and rolling-cache contracts."""
import os
import json
from pathlib import Path
import re
import subprocess
import unittest


ROOT = Path(__file__).resolve().parents[2]


def workflow(name):
    # Psych ships with Ruby on macOS and the hosted Ubuntu runner. Parse the
    # declarative contract, so comments/formatting cannot satisfy these checks.
    return json.loads(subprocess.check_output(
        ['ruby', '-ryaml', '-rjson', '-e',
         'puts JSON.generate(YAML.safe_load(File.read(ARGV[0])))',
         str(ROOT / f'.github/workflows/{name}.yml')], text=True))


def expression(value, context):
    if '||' in value:
        return next((expression(part.strip(), context) for part in value.split('||')
                     if expression(part.strip(), context)), '')
    if value.startswith('hashFiles('):
        return 'unchanged-manifest-hash'
    if value.startswith("'") and value.endswith("'"):
        return value[1:-1]
    return context[value]


def interpolate(value, context):
    return re.sub(r'\$\{\{\s*(.*?)\s*\}\}',
                  lambda match: expression(match.group(1), context), value)


def condition(value, context):
    terms = value.split('&&')
    for term in terms:
        term = term.strip()
        if term == 'success()':
            if not context['success']:
                return False
            continue
        left, operator, right = re.fullmatch(r'(.+?)\s*(==|!=)\s*(.+)', term).groups()
        equal = expression(left.strip(), context) == expression(right.strip(), context)
        if equal != (operator == '=='):
            return False
    return True


class PipelineConfigurationTest(unittest.TestCase):
    def test_historical_publication_uses_the_workflow_revision_gate(self):
        publish = workflow('release')['jobs']['publish']
        checkout, = (step for step in publish['steps']
                     if step.get('uses') == 'actions/checkout@v4')
        self.assertEqual(checkout['with']['ref'], '${{ github.workflow_sha }}')
        gate, = (step for step in publish['steps']
                 if step.get('name') == 'Recheck exact-commit CI before publication')
        self.assertEqual(gate['env']['COMMIT'], '${{ inputs.commit || github.sha }}')

    def test_required_rust_aggregate_only_accepts_all_success(self):
        aggregate = workflow('ci')['jobs']['rust']
        self.assertEqual(set(aggregate['needs']), {'checks', 'workspace'})
        self.assertEqual(aggregate['if'], 'always()')
        step, = aggregate['steps']
        self.assertEqual(step['env'], {'CHECKS': '${{ needs.checks.result }}',
                                      'WORKSPACE': '${{ needs.workspace.result }}'})
        command = step['run']
        for checks in ('success', 'failure', 'cancelled', 'skipped'):
            for workspace in ('success', 'failure', 'cancelled', 'skipped'):
                result = subprocess.run(['bash', '-c', command],
                                        env=dict(os.environ, CHECKS=checks, WORKSPACE=workspace))
                self.assertEqual(result.returncode == 0,
                                 checks == workspace == 'success', (checks, workspace))

    def test_cache_primary_keys_roll_but_restore_prefixes_do_not(self):
        for name in ('ci', 'release'):
            jobs = workflow(name)['jobs']
            job = jobs['workspace' if name == 'ci' else 'distribute']
            restore, = (step for step in job['steps']
                        if step.get('uses') == 'actions/cache/restore@v4')
            save, = (step for step in job['steps']
                     if step.get('uses') == 'actions/cache/save@v4')
            context = {'runner.os': 'Linux', 'runner.arch': 'X64', 'matrix.check': 'test',
                       'inputs.commit': '', 'github.ref': 'refs/heads/main',
                       'github.event_name': 'push', 'success': True,
                       'steps.cache.outputs.cache-hit': 'false'}
            keys, prefixes = [], []
            for commit in ('a' * 40, 'b' * 40):
                context['github.sha'] = commit
                keys.append(interpolate(restore['with']['key'], context))
                prefixes.append(interpolate(restore['with']['restore-keys'], context))
                context['steps.cache.outputs.cache-primary-key'] = keys[-1]
                self.assertEqual(interpolate(save['with']['key'], context), keys[-1])
            self.assertNotEqual(*keys)
            self.assertEqual(*prefixes)
            self.assertTrue(condition(save['if'], context))
            for override in ({'github.ref': 'refs/pull/1/merge'}, {'success': False},
                             {'steps.cache.outputs.cache-hit': 'true'}):
                self.assertFalse(condition(save['if'], context | override))
            if name == 'release':
                self.assertFalse(condition(save['if'], context | {'github.event_name': 'workflow_dispatch'}))


if __name__ == '__main__':
    unittest.main()
