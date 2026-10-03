"""Fail closed on the latest exact-commit CI workflow, not a similarly named check."""
import json
import os
import subprocess
import time


def ci_state(runs, commit):
    eligible = [run for run in runs if run['head_sha'] == commit
                and run['event'] in ('push', 'pull_request')]
    if not eligible:
        return False
    latest = max(eligible, key=lambda run: (run['run_number'], run['run_attempt']))
    if latest['status'] != 'completed':
        return False
    if latest['conclusion'] != 'success':
        raise RuntimeError('release refused: latest workspace CI failed for this exact commit')
    return True


def wait_for_ci(commit):
    repo = os.environ['GITHUB_REPOSITORY']
    while True:
        runs = json.loads(subprocess.check_output(
            ['gh', 'api', f'repos/{repo}/actions/workflows/ci.yml/runs?head_sha={commit}&per_page=100'],
            text=True))['workflow_runs']
        if ci_state(runs, commit):
            return
        print('Waiting for workspace CI on the release commit', flush=True)
        time.sleep(15)


if __name__ == '__main__':
    import sys
    wait_for_ci(sys.argv[1])
