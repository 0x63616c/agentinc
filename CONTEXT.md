# AgentInc

A personal operating system where a human and agents work through the same constructs. Its first slice is a development environment where agents write code.

## Language

**AgentInc**:
The personal operating system where people and agents share the same constructs. Formerly named Agentinc OS.

**Owner**:
The human a Workspace belongs to; every Agent acts under the Owner's credentials and on the Owner's behalf.
_Avoid_: principal, user (when the Owner is meant), admin

**Workspace**:
The container that scopes every record: Tickets, Agents, Automations, Conversations and Connections. An Owner can have several.
_Avoid_: project, tenant, organization

**Agent**:
A named worker with a model and instructions, created at runtime, that holds Conversations and can be assigned Tickets. Evee is one.
_Avoid_: bot, worker, assistant (except as the Assistant page's name)

**Evee**:
The assistant the user talks to in AgentInc, and the single point of contact who turns requests into tracked work.
_Avoid_: main agent, supervisor, orchestrator, firstmate

**Conversation**:
An ongoing, resumable exchange of messages between the user and an agent such as Evee.
_Avoid_: chat, thread, session

**Turn**:
One exchange in a Conversation: a user message, the tool calls it leads to, and the Agent's reply.
_Avoid_: round, step, request

**Ticket**:
A unit of tracked work, shared by humans and agents.
_Avoid_: task, issue, job, backlog item

**Comment**:
A message posted on a Ticket by the user or an agent; comments are the Ticket's work log.
_Avoid_: note, reply, update

**Label**:
A short text tag on a Ticket, at most ten per Ticket, drawn in a stable colour per name.
_Avoid_: tag

**Work**:
One run of an Agent on an assigned Ticket, from assignment to its result Comment. Work has its own state (queued, running, completed, failed, cancelled), which is never a Ticket status.
_Avoid_: run, job, execution, ticket run

**Activity**:
One recorded change in a Ticket's history: created, renamed, status, assigned, labels, links and Work lifecycle.
_Avoid_: event, audit entry, history item

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
The short bar attached to the bottom inside the main content card.

## Reserved words

**Session**:
Reserved for the SDK (Turnkeel): a conversation with an agent that outlives one turn. In the product, say Conversation. The Mac app's persisted UI state (route history, panes, font) is `UiState`, not a session.

## Avoid everywhere

agentic (say what the agents do), principal (say Owner), session (outside the SDK), firstmate.
