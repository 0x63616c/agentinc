#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 2 ]; then
  echo "usage: dev/check-isolation.sh WORKTREE_A WORKTREE_B" >&2
  exit 2
fi

a=$(cd "$1" && pwd -P)
b=$(cd "$2" && pwd -P)
test "$a" != "$b"

for tree in "$a" "$b"; do
  (cd "$tree" && cargo xtask doctor >/dev/null)
done

field() {
  python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))[sys.argv[2]])' "$1/.local/dev/instance.json" "$2"
}

id_a=$(field "$a" id)
id_b=$(field "$b" id)
port_a=$(field "$a" tilt_port)
port_b=$(field "$b" tilt_port)
test "$id_a" != "$id_b"
test "$port_a" != "$port_b"

pid_a=
pid_b=
cleanup() {
  for pid in "$pid_a" "$pid_b"; do
    if [ -n "$pid" ]; then kill "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true; fi
  done
  (cd "$a" && TILT_DEV_DIR="$a/.local/dev/tilt" cargo xtask down >/dev/null) || true
  (cd "$b" && TILT_DEV_DIR="$b/.local/dev/tilt" cargo xtask down >/dev/null) || true
}
trap cleanup EXIT

start() {
  local tree=$1 port=$2
  (cd "$tree" && TILT_DEV_DIR="$tree/.local/dev/tilt" exec tilt up --file Tiltfile --host 127.0.0.1 --port "$port") >"$tree/.local/dev/tilt-isolation.log" 2>&1 &
  started_pid=$!
}

ready() {
  local tree=$1 port=$2
  local log="$tree/.local/dev/tilt-isolation.log"
  if ! rg -q 'Successfully loaded Tiltfile' "$log"; then
    tail -n +1 -F "$log" | while IFS= read -r line; do
      case "$line" in *'Successfully loaded Tiltfile'*) break;; esac
    done || true
  fi
  TILT_DEV_DIR="$tree/.local/dev/tilt" tilt wait --host 127.0.0.1 --port "$port" --timeout 5m --for=condition=Ready uiresource/aincd
  curl -fsS "$(cat "$tree/.local/dev/api-url")/health/ready" >/dev/null
}

db_url() {
  local tree=$1 id=$2 port
  port=$(cd "$tree" && docker compose --env-file .local/dev/compose.env -p "$id" -f dev/compose.yaml port postgres 5432)
  printf 'postgres://agentinc:agentinc@%s/agentinc_%s' "$port" "${id#agentinc-}"
}

start "$a" "$port_a"
pid_a=$started_pid
start "$b" "$port_b"
pid_b=$started_pid
ready "$a" "$port_a"
ready "$b" "$port_b"

pg_a=$(db_url "$a" "$id_a")
pg_b=$(db_url "$b" "$id_b")
test "$pg_a" != "$pg_b"
psql "$pg_a" -v ON_ERROR_STOP=1 -qAtc "CREATE TABLE IF NOT EXISTS isolation_probe (marker text PRIMARY KEY); INSERT INTO isolation_probe VALUES ('$id_a') ON CONFLICT DO NOTHING"
psql "$pg_b" -v ON_ERROR_STOP=1 -qAtc "CREATE TABLE IF NOT EXISTS isolation_probe (marker text PRIMARY KEY); INSERT INTO isolation_probe VALUES ('$id_b') ON CONFLICT DO NOTHING"
test "$(psql "$pg_a" -Atc 'SELECT marker FROM isolation_probe')" = "$id_a"
test "$(psql "$pg_b" -Atc 'SELECT marker FROM isolation_probe')" = "$id_b"

temporal_a=$(cd "$a" && docker compose --env-file .local/dev/compose.env -p "$id_a" -f dev/compose.yaml port temporal 7233)
temporal_b=$(cd "$b" && docker compose --env-file .local/dev/compose.env -p "$id_b" -f dev/compose.yaml port temporal 7233)
test "$temporal_a" != "$temporal_b"
temporal --address "$temporal_a" operator namespace describe --namespace "$id_a" >/dev/null
temporal --address "$temporal_b" operator namespace describe --namespace "$id_b" >/dev/null

kill "$pid_a"
wait "$pid_a" 2>/dev/null || true
pid_a=
(cd "$a" && TILT_DEV_DIR="$a/.local/dev/tilt" cargo xtask down >/dev/null)
ready "$b" "$port_b"
test "$(psql "$pg_b" -Atc 'SELECT marker FROM isolation_probe')" = "$id_b"
start "$a" "$port_a"
pid_a=$started_pid
ready "$a" "$port_a"
test "$(psql "$(db_url "$a" "$id_a")" -Atc 'SELECT marker FROM isolation_probe')" = "$id_a"
test "$(psql "$pg_b" -Atc 'SELECT marker FROM isolation_probe')" = "$id_b"
echo "isolation and one-stack restart passed: $id_a $id_b"
