#!/usr/bin/env python3
"""Run a native build and record sampled CPU/RSS for its process tree."""
import os
import subprocess
import sys
import time


def usage(pid):
    rows = {}
    for line in subprocess.check_output(
        ['ps', '-A', '-o', 'pid=,ppid=,rss=,%cpu='], text=True
    ).splitlines():
        fields = line.split()
        if len(fields) == 4:
            child, parent, rss, cpu = fields
            rows[int(child)] = (int(parent), int(rss), float(cpu))
    family = {pid}
    while True:
        children = {child for child, (parent, _, _) in rows.items() if parent in family}
        if children <= family:
            break
        family |= children
    return (
        sum(rows[child][1] for child in family if child in rows),
        sum(rows[child][2] for child in family if child in rows),
    )


def main():
    if len(sys.argv) < 2:
        raise SystemExit('usage: measure-build.py COMMAND [ARG ...]')
    started = time.monotonic()
    process = subprocess.Popen(sys.argv[1:])
    peak_rss = peak_cpu = 0
    while process.poll() is None:
        rss, cpu = usage(process.pid)
        peak_rss = max(peak_rss, rss)
        peak_cpu = max(peak_cpu, cpu)
        try:
            process.wait(timeout=2)
        except subprocess.TimeoutExpired:
            pass
    summary = (
        f'Native build on {os.uname().nodename}: {time.monotonic() - started:.0f}s; '
        f'peak sampled process-tree RSS {peak_rss / 1024:.0f} MiB; '
        f'peak sampled CPU {peak_cpu:.0f}% (100% = one core).'
    )
    print(summary)
    if os.environ.get('GITHUB_STEP_SUMMARY'):
        with open(os.environ['GITHUB_STEP_SUMMARY'], 'a') as output:
            output.write(summary + '\n')
    raise SystemExit(process.returncode)


if __name__ == '__main__':
    main()
