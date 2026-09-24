#!/bin/sh
# Launch only an owned, fresh QA instance. Never reuse normal Agentinc state.
set -eu
cd "$(dirname "$0")/../../.."
state="${1:-$PWD/.local/pilot-$(date +%s)}"
case "$state" in /*) ;; *) echo 'Use an absolute new state directory' >&2; exit 2;; esac
mkdir -p .local
mkdir -m 700 "$state"
cargo build --locked -p agentinc-os -p gpui-pilot-cli --features agentinc-os/automation
printf 'Instance: %s/s/instance.json\n' "$state"
export AGENTINC_SESSION_PATH="$state/session.json"
export AINC_DISCOVERY_FILE="${AINC_DISCOVERY_FILE:-$PWD/.local/dev/api-url}"
test -s "$AINC_DISCOVERY_FILE" || { echo "Start cargo xtask dev first" >&2; exit 1; }
export AINC_LEGACY_DIR="$state/legacy"
export AGENTINC_CODEX_HOME="$state/codex"
export AGENTINC_WINDOW_TITLE='Agentinc Pilot QA'
exec target/debug/agentinc-os --gpui-pilot-session "$state/s"
