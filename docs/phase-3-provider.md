# Phase 3: subscription model steps

`ainc_daemon::inference::CodexModel` implements Turnkeel's `Model` interface. It
sends our model-step request directly to the Codex backend Responses endpoint;
Turnkeel executes every returned tool call. It never asks Codex to run a turn.
The existing app's official Codex sign-in client still owns OAuth and refresh.
Only the explicitly selected profile is read; no default Codex home or API key
fallback is used by the model adapter.

This adapter requires that profile's Codex `auth.json` file credential store.
A missing/unsupported store gives an actionable Connection error. The transport
refuses redirects, redacts provider error bodies, bounds the response size, maps
rate limits/server errors to retryable model failures, and rejects malformed or
incomplete output. Tokens are never serialized into agent definitions, workflow
history, product records, events or errors. Sign-in remains separate from app
identity. The adapter is personal-use scope, not pooled subscription access.

Responses function calls/results become SDK content blocks. Encrypted reasoning
is retained in `Content::ModelContext` and returned only to the same provider.
It is not display text or a Ticket Comment. `Run::events()` yields typed messages
and tool results while work runs, catches up new subscribers, and ends when the
run closes. It does not expose raw provider events or token deltas.

Pinned protocol references inspected at OpenAI Codex commit
`e0ef5a1a0f6421601baaa679fb37eddaa4e9c8c1`:

- [backend base URL](https://github.com/openai/codex/blob/e0ef5a1a0f6421601baaa679fb37eddaa4e9c8c1/codex-rs/model-provider-info/src/lib.rs)
- [Responses transport](https://github.com/openai/codex/blob/e0ef5a1a0f6421601baaa679fb37eddaa4e9c8c1/codex-rs/codex-api/src/endpoint/responses.rs)
- [credential store](https://github.com/openai/codex/blob/e0ef5a1a0f6421601baaa679fb37eddaa4e9c8c1/codex-rs/login/src/auth/storage.rs)
- [official authentication documentation](https://developers.openai.com/codex/auth)

The backend is subject to change; a source pin and fixtures do not prove live
compatibility. No live provider smoke was run: this worktree has no isolated
`auth.json`. Automated tests use a loopback Responses fixture and fake credentials,
including a real Turnkeel tool round-trip, context retention, error redaction,
redirect refusal and malformed/incomplete streams. Run
`cargo test --locked -p ainc-daemon --lib`.

Activities now heartbeat while model/tool I/O is pending, so lost workers can be
retried and cancellation drops live I/O futures. Ten seconds without a heartbeat
marks an activity lost; idempotency still belongs to the effect boundary. Tools
must stop owned subprocesses when dropped. A real-process fixture kills the worker
after a durable effect but before acknowledgement, then verifies a replacement
uses the same receipt/key, produces exactly one effect and passes history replay.
Non-idempotent tools retain their existing one-attempt policy.

Daemon integration and its process recovery evidence are documented in
[phase-3-execution.md](phase-3-execution.md).

## Providers (1.0)

`ainc_daemon::providers` is the single model catalog. Canonical model IDs are
`provider:model`, for example `claude:claude-sonnet-5`, `codex:gpt-5-codex` and
`openrouter:typesafe/jev-router`; a bare ID from earlier releases still means the
ChatGPT Connection. `Providers` implements `execution::ModelCatalog`, and
`resolve_streaming` hands the transport a sink so reply text can stream.

- **Claude** (`providers/claude.rs`) runs one `claude -p` invocation of the official
  Claude Code CLI per model step under the user's existing Claude sign-in, with
  built-in tools disabled, a replaced system prompt and a JSON schema
  (`reply` plus `tool_calls`). Turnkeel still executes every tool call. Status comes
  from `claude auth status`; sign-in spawns `claude auth login`, opens the browser
  link and forwards the pasted code. The daemon never reads or copies Anthropic
  credentials.
- **ChatGPT** (`inference.rs`, `codex.rs`) is the existing Codex Responses adapter,
  now with `response.output_text.delta` streaming.
- **OpenRouter** (`providers/openrouter.rs`) speaks OpenAI-style chat completions with
  streaming tool calls. The API key lives only in the login Keychain
  (`providers/secrets.rs`, service `<bundle id>.providers`); Jev
  (`typesafe/jev-router`, `typesafe/jev-latest`) is featured for cheap real runs.

`GET /v1/providers` reports every provider (connection, account, models, sign-in
progress) plus the workspace's default model; `POST /v1/providers/{id}/connect`,
`cancel`, `disconnect` and `test` drive them. `test` makes one small real exchange.
The legacy `/v1/connection` routes delegate to the same ChatGPT state.

Every model step of a Conversation turn is stored in `turn_steps` (`text`,
`tool_use`, `tool_result`), and streaming text accumulates in `turns.draft` until its
step lands. Evee's tools are the Ticket and Automation commands, `list_runs` over the
durable runtime and `http_request` under the workspace `http_policy` setting
(`agent_tools.rs`): public hosts only, no redirects, 20 s timeout, 64 KB body.
