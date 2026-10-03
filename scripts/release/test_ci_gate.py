import unittest
from unittest.mock import patch

from ci_gate import ci_state, wait_for_ci


COMMIT = 'a' * 40


def ci_run(**fields):
    return dict(head_sha=COMMIT, event='push', run_number=1, run_attempt=1,
                status='completed', conclusion='success') | fields


class CiGateTest(unittest.TestCase):
    def test_success_must_be_for_exact_commit(self):
        self.assertTrue(ci_state([ci_run()], COMMIT))
        run = ci_run()
        run['head_sha'] = 'b' * 40
        self.assertFalse(ci_state([run], COMMIT))
        self.assertFalse(ci_state([], COMMIT))

    def test_pending_latest_run_does_not_accept_old_success(self):
        latest = ci_run()
        latest.update(run_number=2, status='in_progress', conclusion=None)
        self.assertFalse(ci_state([latest, ci_run()], COMMIT))

    def test_latest_failed_or_cancelled_attempt_blocks(self):
        for conclusion in ('failure', 'cancelled', 'skipped', 'timed_out', None):
            latest = ci_run()
            latest.update(run_attempt=2, conclusion=conclusion)
            with self.subTest(conclusion=conclusion), self.assertRaises(RuntimeError):
                ci_state([ci_run(), latest], COMMIT)

    def test_unrequested_workflow_event_cannot_satisfy_gate(self):
        run = ci_run()
        run.update(event='workflow_dispatch')
        self.assertFalse(ci_state([run], COMMIT))

    def test_queries_the_ci_workflow_not_a_check_name(self):
        import json
        with patch.dict('os.environ', GITHUB_REPOSITORY='owner/repo'), patch(
                'ci_gate.subprocess.check_output', return_value=json.dumps(
                    dict(workflow_runs=[ci_run()]))) as command, patch('ci_gate.time.sleep') as sleep:
            wait_for_ci(COMMIT)
        self.assertIn(f'/actions/workflows/ci.yml/runs?head_sha={COMMIT}', command.call_args.args[0][-1])
        sleep.assert_not_called()


if __name__ == '__main__':
    unittest.main()
