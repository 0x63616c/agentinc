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

Completed `response.output_item.done` events supply the output items; the terminal
`response.completed` event can have an empty or absent output array. Its snapshot
is used only when no completed items were streamed, avoiding duplicate tool calls.
No partial item is accepted without successful response completion.

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
redirect refusal and malformed/incomplete streams. The streaming regression covers
empty/missing terminal output, snapshot deduplication and a two-turn SDK session
that retains tool results and reasoning context. Run
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
