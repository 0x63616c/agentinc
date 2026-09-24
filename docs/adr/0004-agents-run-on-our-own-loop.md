# Agents run on our own loop

Every agent, Evee included, runs on this repo's SDK loop with tools we write (files, shell, git), rather than delegating its turn to the Claude Code or Codex CLI. The first provider uses Codex's official ChatGPT OAuth sign-in and the Codex backend Responses endpoint, as OpenCode does, for personal subscription use. That backend may change; pin the integration and test it against live sign-in. An API-key model loop is an option, not a prerequisite. The SDK crate name is pending a separate naming decision.
