# Agents run on the agentinc loop, not on Claude Code or Codex

Every agent, Evee included, runs on this repo's SDK loop with tools we write (files, shell, git), rather than driving the Claude Code or Codex CLIs as processes. That costs us building the coding tools ourselves, and buys one loop we own end to end: durable, testable, observable, and shown in our own UI. The first model provider reaches OpenAI's Codex backend through the user's ChatGPT sign-in, the way OpenCode does, so work runs on the subscription rather than API credit; Claude can follow later through an API key.
