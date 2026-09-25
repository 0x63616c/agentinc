# AgentInc

A personal operating system where a human and agents work through the same constructs. Its first slice is a development environment for agentic coding.

## Language

**AgentInc**:
The personal operating system where people and agents share the same constructs. Formerly named Agentinc OS.

**Evee**:
The assistant the user talks to in AgentInc, and the single point of contact who turns requests into tracked work.
_Avoid_: main agent, supervisor, orchestrator, firstmate

**Conversation**:
An ongoing, resumable exchange of messages between the user and an agent such as Evee.
_Avoid_: chat, thread

**Ticket**:
A unit of tracked work, shared by humans and agents.
_Avoid_: task, issue, job, backlog item

**Comment**:
A message posted on a Ticket by the user or an agent; comments are the Ticket's work log.
_Avoid_: note, reply, update

**Environment**:
Reserved for a place other than this Mac where an agent's work runs, such as a container or a remote machine. Nothing uses it yet.
_Avoid_: box, sandbox, machine

**Automation**:
A saved rule that initiates agent work when its trigger fires.
_Avoid_: cron job, background task

**Occurrence**:
One firing of an Automation, linked to the Ticket it creates.
_Avoid_: run, job

**Connection**:
A user's link to an external account or provider that makes its capabilities available in AgentInc.
_Avoid_: integration, account

**Route**:
A named destination in the AgentInc app's navigation.
_Avoid_: tab index, screen ID

**Page**:
The view shown for a Route.
_Avoid_: route, tab

**Status bar**:
The thin strip beneath the main pane at the bottom of the AgentInc window.
