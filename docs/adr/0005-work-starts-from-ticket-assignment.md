# Autonomous effects start from a Ticket

Autonomous agent work with effects requires an assigned Ticket; a Conversation reply or inline inference does not. Evee creates and assigns Tickets through the same authorized commands as a human, and each Automation occurrence creates and assigns its own Ticket. This keeps effectful work tracked and inspectable without making every reply a Ticket.

**Amended 2026-09-26 (1.0 board):** a Ticket has one of six statuses: Backlog, To do, In progress, Blocked, Done, Cancelled. Blocked and Cancelled are states a person sets on the board; they are not run failures, which stay run state. Only To do and In progress dispatch an agent assignee, so moving work into any other column stops it.
