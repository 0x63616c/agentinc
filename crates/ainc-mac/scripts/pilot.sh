#!/bin/sh
# Launch only an owned, fresh QA instance. Never reuse normal Agentinc state.
set -eu
cd "$(dirname "$0")/.."
state="${1:-$PWD/.local/pilot-$(date +%s)}"
case "$state" in /*) ;; *) echo 'Use an absolute new state directory' >&2; exit 2;; esac
mkdir -p .local
mkdir -m 700 "$state"
cargo build --locked --workspace --features automation
printf 'Instance: %s/s/instance.json\n' "$state"
export AGENTINC_SESSION_PATH="$state/session.json"
export AGENTINC_DATABASE_PATH="$state/assistant.sqlite3"
export AGENTINC_CODEX_HOME="$state/codex"
export AGENTINC_WINDOW_TITLE='Agentinc Pilot QA'
exec target/debug/agentinc-os --gpui-pilot-session "$state/s"
