# Agentinc OS

A personal operating system where a human and agents work through the same constructs. Its first slice is a development environment for agentic coding.

## Language

**Evee**:
The assistant the user talks to in Agentinc OS, and the single point of contact who turns requests into tracked work.
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
