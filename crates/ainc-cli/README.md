# `ainc` CLI

`ainc` is the command line client for the running AgentInc daemon. The released app bundles it at `AgentInc.app/Contents/MacOS/ainc`. Run that binary's `install-cli` command to link it into `~/.local/bin` without administrator access; add that directory to your `PATH` if needed.

```sh
ainc --help
ainc status
ainc tickets list
ainc tickets create --title 'Review launch checklist'
ainc automations list
ainc --json tickets list
ainc completions zsh > ~/.zfunc/_ainc
```

Commands come from the checked-in [OpenAPI contract](../../api/openapi-3.0.json) through `cargo xtask generate`. Resource commands expose the generated operations; JSON command endpoints also have schema-derived flags for each command variant. Use `--json-body FILE` on a command variant or its `command` operation for the full request body. `--json` prints the complete response as JSON.

The CLI reads the installed app's daemon discovery and owner credential files. For source development it falls back to `.local/dev/api-url` and `.local/dev/owner-token`. `AINC_DISCOVERY_FILE` selects a discovery file, `AINC_API_URL` overrides the URL, and `AINC_TOKEN_FILE` overrides the credential file. The CLI does not start or stop the daemon; open AgentInc or start `cargo xtask dev` first.
