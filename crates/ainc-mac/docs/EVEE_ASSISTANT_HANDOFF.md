# Evee assistant and Tasks increment

This increment follows the native Control shell. Evee now has a real OpenAI-backed conversation, and Tasks supports creating, completing, reopening and deleting local to-dos. Other integration destinations remain placeholders. The shell's design reference, tabs, panel geometry, resizing and session file remain in place.

## Run and connect

Build with `scripts/bundle.sh` and open `dist/Agentinc OS.app`. In Evee, choose **Setup**, enter your OpenAI API key, leave `gpt-5-mini` or choose a Responses-compatible model available to your account, then **Save connection**. Settings → OpenAI connection opens the same setup form. “Key configured” means a credential is present; the first sent message validates service access. The key field is masked, does not copy its contents, and clears after saving.

The app stores the key in macOS Keychain through GPUI's credential API, under the app-specific server `com.agentinc.os.openai`; its account field stores the model name. A blank replacement key retains the existing key when changing models. **Remove saved key** removes only this app's credential. Keychain failures remain visible and leave the prior in-memory key intact.

Alternatively launch the executable with `OPENAI_API_KEY` already set in its environment. That takes precedence over Keychain and makes credential setup read-only for the process. `OPENAI_MODEL` overrides the model at startup. Finder-launched apps do not normally inherit a terminal's environment; use in-app setup for normal Finder launches. An OpenAI API key/account is required; this does not consume Codex CLI or ChatGPT subscription credentials.

Return or **Send** sends a message. The composer stays responsive while the request runs. One request can run at a time. Replies appear together after completion with a brief eased entrance, and can be copied. History scrolls independently of the anchored composer. Sending without a key opens setup and keeps the draft. Failures retain the user message with **Retry reply**, without inserting a duplicate user turn. An interrupted request is marked retryable on the next launch; it is never automatically resubmitted. A response that fails to save remains visible, with **Retry saving**, and blocks another send until saved.

## Data and boundaries

- SQLite: `~/Library/Application Support/Agentinc OS/assistant.sqlite3`, including ordinary SQLite WAL/SHM sidecars. New database files have owner-only permissions. No credential is stored in SQLite or session JSON. SQLite content is local plain text, not encrypted by this app.
- Shell state remains `~/Library/Application Support/Agentinc OS/session.json`.
- `AGENTINC_DATABASE_PATH`, `AGENTINC_SESSION_PATH`, and `AGENTINC_WINDOW_TITLE` isolate QA from normal application state. Do not run two app instances against the same QA database; startup recovery assumes the prior process has exited.
- Schema version 1 lives in `src/storage.rs`; newer schema versions are rejected rather than downgraded or erased. Read/write errors are surfaced, without falling back to an empty replacement database.
- The UI restores the complete local conversation. Each request sends the latest 20 successfully completed earlier turns plus the current prompt. Failed turns and turns later than a retried prompt are excluded. There is no conversation summarization or multi-conversation management yet.
- Messages are limited to 8,000 characters and task names to 500. The native composer is a single-line field with horizontal caret scrolling; pasted line breaks become spaces. Replies retain paragraphs as plain text.
- Requests go only to `https://api.openai.com/v1/responses`, with TLS verification, redirects disabled, `store: false`, a 90-second timeout and a 4,096-output-token budget. Raw service errors, request headers, keys and message contents are not logged. Response bodies are bounded to 1 MiB.
- This is a GPT chat integration, not a Codex agent runtime. Evee has no tools, shell access, task mutation, file access or device control. Conversational task creation was optional and is deferred. Tasks are managed directly in their existing sidebar destination.
- New messages/task insertion use restrained 220ms easing; new controls use interruptible 140ms hover fades. The existing panel animations remain. macOS Reduce Motion is read once at startup and disables these animations. No hover tooltips are added.

`src/evee.rs` owns chat/setup UI and request state. `src/assistant.rs` owns the HTTP transport and prompt contract. `src/tasks.rs` owns task UI. Both use `src/storage.rs`. `src/input.rs` extends the existing native input with submit events, secret masking and caret scrolling.

The API shape and default model were checked against official [OpenAI text generation guidance](https://developers.openai.com/api/docs/guides/text) and [GPT-5 Mini documentation](https://developers.openai.com/api/docs/models/gpt-5-mini). Responses was chosen because the official guidance recommends it for new text integrations.

## Acceptance, 23 September 2026

Local commands, from this worktree:

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
scripts/bundle.sh
codesign --verify --deep --strict --verbose=2 'dist/Agentinc OS.app'
```

The test suite covers real local HTTP sockets (authorization header, request roles, multi-part text, safe 401/429/5xx/redirect errors, malformed/incomplete/empty replies, refusal, timeout and bounded history), SQLite task CRUD/reopen, interrupted-turn recovery and same-turn retry through HTTP followed by database reopen. Tests send the test-only credential exclusively to a loopback fixture. Schema-forward compatibility and owner-only new database permissions are checked too. Existing shell/navigation tests remain.

Native acceptance uses the bundled binary in an ignored, separately identified `Assistant Acceptance.app`, with explicit QA database/session paths. The Tasks input created three records, a checkbox completed one, and Delete removed the disposable third task. After quitting and relaunching, the remaining open and completed tasks were present in both the native view and SQLite. Sending a draft without credentials retained it, opened setup and did not create a saved turn. The regular app's data was not used for these checks.

- [Tasks and Evee chat](verification/assistant/chat-and-tasks.png)
- [Setup and Tasks after relaunch](verification/assistant/setup-and-restored-tasks.png)

**No live OpenAI round-trip was performed:** neither the worker environment nor this app's Keychain entry supplied a valid API credential. The screenshot's saved conversation is explicitly labeled “QA fixture — not a live AI reply”; it verifies rendering/restoration, not model output. No production fake-response mode or bundled sample conversation exists. Keychain writes with a real credential and actual OpenAI account/model access remain to be exercised by the owner. The integration was verified against deterministic HTTP fixtures instead.

Screenshots are exact native-window captures at 1360×829 logical pixels. Firstmate authorized macOS `screencapture` after CUA's captures proved stale/ambiguous across repeated QA processes; a unique QA bundle and exact window title were used for final captures. This validates local native behavior and layout, not hosted CI, VoiceOver, all IME languages, small-window acceptance, notarization or release signing. No hosted CI is configured.
