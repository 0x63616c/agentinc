# Phase 2 ownership acceptance

Verified on this Mac in the isolated `agentinc-phase2-ownership` worktree. All Postgres databases, legacy fixtures, Codex fixtures, app preferences and pilot endpoints were disposable. The regular Application Support directory and subscription profile were not read or changed.

- Workspace formatting, Clippy (`--workspace --all-targets -- -D warnings`) and tests passed, including the real Postgres import/command tests and the real daemon process test with a gated Codex fixture.
- `cargo xtask generate --check` verified the checked-in OpenAPI/client/CLI output.
- `cargo test --locked -p agentinc-os --features rendered-tests --test rendered_shell`: all 38 real Metal frames and region-removal negative controls passed.
- `AINC_DISCOVERY_FILE="$PWD/.local/dev/api-url" cargo test --locked -p agentinc-os --features automation --test pilot_acceptance -- --nocapture`: Search → Tasks → Unicode task creation, overlay occlusion, stale refs, keyboard selection/undo, concurrent condition wait, screenshot regions and normal quit passed against `cargo xtask dev`.
- Built and launched `crates/ainc-mac/dist/AgentInc.app` with `scripts/bundle.sh automation`. `codesign --verify --deep --strict --verbose=2` passed for the app and nested daemon.

## Bundled companion click-through

The bundle was launched with a fresh discovery path, an empty disposable database on the worktree's Compose Postgres, `AINC_DATABASE_URL`, a copied synthetic schema-2 SQLite database, and an isolated protocol-fixture Codex executable. No daemon was prestarted for that database: the app launched the signed companion itself.

1. Pilot opened Tasks and observed imported task ID 7. The imported Conversation ID 3 and its completed legacy reply appeared in Evee.
2. Pilot opened Settings and selected the fixture model, then sent “Finish this after I quit the app.” through the native composer. The daemon acknowledged turn ID 2 as running while the provider waited on an explicit FIFO gate. [Acknowledged frame](ownership/acknowledged.png).
3. Pilot pressed Cmd+Q. After the app exited, the gate was released. The daemon persisted the completed reply with no app/HTTP request waiting for it. [Stored result](ownership/after-close.json).
4. Relaunching the bundle reconnected to the companion and displayed both the imported legacy reply and the completed reply. [Reopened native frame](ownership/reopened-reply.png).
5. Pilot renamed the Conversation, created/selected another Conversation and completed the imported Task. The daemon snapshot contains the selected Conversation, selected model, renamed history and completed task. [Command results](ownership/click-through.json).

[Task capture](ownership/tasks.png) and [pilot timing samples](ownership/pilot-latency.json) are included. Snapshot median/p95 were approximately 2.5/4.2 ms; key dispatch plus committed frame approximately 15.2/17.4 ms. These are local small-sample observations, not performance guarantees.

One initial pilot run timed out after a committed task during ongoing edits/rebuilds; its precise timing cause was not isolated. Relaunch showed the saved task. The client now retains a command's operation ID across uncertain acknowledgements or failed snapshot refresh, and the acceptance rerun passed with the daemon stable. The suite no longer assumes that the first task in a persistent dev database has ID 1.

The fixtures validate the extracted official Codex stdio contract and lifecycle; they do not assert a new live subscription sign-in or paid model compatibility check. The provider implementation is the imported adapter, relocated into the daemon. Daemon-crash execution recovery beyond visible unknown outcomes remains phase 3.
