# Evee, Codex connection and conversations

Evee uses the installed Codex CLI with a ChatGPT subscription. Settings owns the connection, Assistant owns the conversation library, and the Evee panel opens a conversation for continued chat. Tasks remains a local SQLite to-do list. Other integrations remain placeholders.

## Supported subscription interface: research, 23 September 2026

OpenAI documents [Codex App Server](https://developers.openai.com/codex/app-server) as the embedding interface for products needing authentication, conversation history and agent events. Its stdio transport is newline-delimited JSON-RPC: initialize, send initialized, then call account/thread/turn methods. This is a better fit than wrapping interactive terminal output or invoking a separate `codex exec` for authentication and model discovery.

[Codex authentication](https://developers.openai.com/codex/auth) distinguishes **Sign in with ChatGPT**, which uses subscription access, from API-key billing. The app-server's `account/login/start` with `type: "chatgpt"` returns the official browser URL; `account/login/completed` reports completion. `account/read`, `account/login/cancel`, `account/logout` and `model/list` provide the Settings controls. This is the same managed sign-in mechanism exposed by `codex login`, without copying credentials into the host app. No unofficial endpoints, token scraping, API-key fallback or external-token authentication is used.

The supported [`CODEX_HOME` configuration](https://developers.openai.com/codex/config-advanced) allows a separate profile. Agentinc OS sets it to `~/Library/Application Support/Agentinc OS/codex`. Codex owns all credential persistence and refresh there, including whichever credential store Codex selects. The app never reads an auth file or accesses Keychain. Its Sign out affects this profile rather than the user's ordinary CLI profile. An existing CLI sign-in is therefore not automatically imported: connect once in Settings.

Validated against **codex-cli 0.155.1** and its generated JSON schema. The CLI still labels `app-server` experimental; OpenAI documents the public embedding interface and distinguishes separately gated experimental APIs. This client does not opt into experimental API capabilities or WebSocket transport. Compatibility with other CLI versions is not claimed. In particular, the tested `thread/start` sandbox wire value is `"read-only"`, as specified by the installed schema, not the camel-case value shown in one documentation example. The opt-in installed-CLI contract test catches this before a billable turn.

## Run and connect

Install the official [Codex CLI](https://developers.openai.com/codex/cli). Build with `scripts/bundle.sh` and open `dist/Agentinc OS.app`. In **Settings → Accounts & connections**, choose **Sign in with ChatGPT** and complete the browser flow. The card shows the account/plan reported by Codex, Refresh, Sign out, and models returned by Codex. “Codex default” leaves model selection to Codex. The selected model is saved locally; model availability remains account-dependent.

Finder launches search `~/.local/bin/codex`, `/opt/homebrew/bin/codex`, `/usr/local/bin/codex`, then PATH. A custom installation can use `AGENTINC_CODEX_PATH`. Missing CLI and protocol errors appear in Settings. Sign-in can be cancelled and times out after five minutes. A callback racing cancellation is reconciled by re-reading Codex's account state.

The previous API-key form, Keychain access and HTTP Responses transport are removed. `OPENAI_API_KEY` and `CODEX_API_KEY` are removed from the Codex child environment, and the child forces ChatGPT login and OpenAI's model provider. Old app-owned Keychain entries, if any, are left unused; this increment does not read, migrate or delete them.

## Conversation and composer behavior

**Assistant** is a sidebar destination (Cmd+8). It lists all local conversations with title, last message snippet and local time, most recently active first. Open a row to continue in Evee; **New conversation** or the panel's plus starts another. A row's ellipsis offers Rename and Delete. Delete requires confirmation naming the conversation and removes its messages. Existing single-chat history migrates to “Previous conversation”. New titles come from the first prompt and remain editable.

The composer has an arrow button inside its lower right corner. Return sends; Shift+Return inserts a newline. The native field preserves pasted newlines, supports vertical cursor movement and line-aware selection, and shows up to five lines with caret scrolling. Long individual lines scroll horizontally rather than soft-wrapping. Sending while disconnected opens Settings and preserves the draft without saving a turn.

One reply runs at a time. Replies appear together on completion and can be copied. Conversation switching/deletion is disabled while replying or while a received reply still needs saving. Failed requests keep the user turn and offer **Retry reply**; retry does not duplicate it. Interrupted requests become retryable at next launch and are never resubmitted automatically. Failed database saves retain the response on screen with **Retry saving**. Drafts are in-memory and clear when opening another conversation.

## Data and runtime boundaries

- SQLite: `~/Library/Application Support/Agentinc OS/assistant.sqlite3`, with WAL/SHM sidecars. New database files are owner-only. Conversations, messages, model preference and tasks are local plain text; credentials are not stored there or in session JSON.
- Schema 2 in `src/storage.rs` transactionally migrates legacy messages, adds conversations/settings and a cascading foreign key. Newer schemas are rejected. Storage errors are surfaced without an empty replacement database.
- Each reply starts an ephemeral Codex thread, supplies the selected conversation's latest 20 successful earlier turns plus the current prompt as role-labeled dialogue, and collects agent-message events. Failed/future turns are excluded when retrying. SQLite, not Codex thread history, is the conversation source of truth.
- Threads use read-only sandboxing, never grant approvals, disable the shell tool and web search, and instruct Evee to answer conversationally without tools. The client declines server-initiated approval/credential requests. There are no app/task/device integration tools. This is not a general-purpose coding-agent UI.
- Protocol calls time out after 30 seconds; replies after 180 seconds. Individual incoming frames are bounded to 2 MiB and accumulated reply text to 1 MiB. Dropping the client kills/reaps its child. Raw backend errors, auth URLs, credentials and message content are not logged by the app. Codex manages its own profile state/logs.
- Messages are limited to 8,000 characters, conversation titles to 120 and task names to 500. Existing restrained animations and Reduce Motion behavior remain; no hover tooltips were added.
- QA isolation uses `AGENTINC_DATABASE_PATH`, `AGENTINC_SESSION_PATH`, `AGENTINC_CODEX_HOME` and `AGENTINC_WINDOW_TITLE`. Do not run two instances against the same QA database: startup recovery assumes the prior process exited.

`src/assistant.rs` owns Codex process/protocol and dialogue context; `src/evee.rs` owns connection, conversation and panel UI; `src/storage.rs` owns persistence; `src/input.rs` owns native editing; `src/tasks.rs` retains Tasks UI. Settings embeds the connection in Accounts & connections, below the persisted Appearance font choice.

## Acceptance, 23 September 2026

Passed locally:

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
scripts/bundle.sh
codesign --verify --deep --strict --verbose=2 'dist/Agentinc OS.app'
AGENTINC_CODEX_HOME="$PWD/.local/contract-codex" \
  cargo test --locked installed_codex_accepts_thread_contract -- --ignored
```

The ordinary suite passed 11 tests after rebasing onto the single-tab shell, with the installed-CLI test ignored by default. The explicit installed-CLI test also passed. Tests cover stdio initialization/events, account parsing, safe protocol errors, reply collection, retry context filtering, schema migration, conversation isolation/CRUD/cascade, persistence/reopen, interrupted-turn recovery, native input line boundaries, existing navigation and Tasks behavior.

Native verification used an isolated `Codex Assistant QA.app` copy of the debug bundle, separate SQLite/session files and a separate Codex profile. CUA captures are exact native-window images at 1360×829 logical pixels:

- [Settings connection and multiline composer](verification/codex-assistant/settings-and-composer.png): signed-out status, Settings routing, Shift+Return; sending disconnected retained the draft and added no turn.
- [Conversation library](verification/codex-assistant/conversations.png): migrated fixture, new conversation, renamed “Planning the week”; native delete confirmation and deletion of a disposable third conversation were exercised.
- [Delete confirmation](verification/codex-assistant/delete-confirmation.png): the final confirmation names the target conversation; Cancel leaves it intact.
- [Official sign-in pending](verification/codex-assistant/sign-in-pending.png): the real browser flow was launched, including cancellation UI (capture before the Settings/single-tab rebase).
- [Live conversation restored after relaunch](verification/codex-assistant/live-conversation.png): real model replies and conversation list after rebuilding/relaunching.

**Live ChatGPT subscription verification succeeded.** After the browser flow, Codex's real `account/read` reported a ChatGPT Pro account and `model/list` returned its models. The initial live request exposed the sandbox-enum mismatch described above; after fixing it, Retry reply returned **“Evee is connected.”** on the existing saved turn. A second prompt, **“What exact sentence did you just say?”**, returned the same sentence, confirming supplied conversation context. Both responses were confirmed in SQLite and restored in the native app after relaunch. These were real subscription requests using Codex's default model; no fake signed-in state or synthetic reply path exists. The separately labeled legacy “QA fixture” checks migration/rendering only.

Sign out was then live-verified in the isolated QA profile: Settings changed from the real connected account to “Not connected” and Evee showed its Settings prompt, while saved messages remained. Final Settings, library, confirmation and restored-reply captures were refreshed after rebasing onto the UI-polish changes.

Not live-verified: every model choice, token-expiry refresh, subscription quota exhaustion, older CLI versions, all IME languages or VoiceOver. No account-email screenshot or credential material is committed. This is local debug-bundle acceptance, not hosted CI, notarization or release signing; no hosted CI is configured.
