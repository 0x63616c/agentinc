# AgentInc

A personal life OS: one native app for the Overview, the Smart Home, Todos and the Calendar, with a local daemon that owns the data. Agents will join later on the Turnkeel SDK and will use the same constructs a person does.

## Language

**AgentInc**:
The personal operating system. Formerly named Agentinc OS.

**Owner**:
The human a Workspace belongs to.
_Avoid_: principal, user (when the Owner is meant), admin

**Workspace**:
The container that scopes every record: Todos and Events. An Owner can have several and switches between them.
_Avoid_: project, tenant, organization

**Todo**:
One item on the Owner's checklist in a Workspace: a title, done or not.
_Avoid_: task, ticket, item

**Calendar**:
The Owner's Events in a Workspace, shown by day.
_Avoid_: schedule, agenda

**Event**:
One entry on the Calendar: a title with a start and an end.
_Avoid_: appointment, meeting, booking

**Overview**:
The first page: a greeting, the weather, the open Todos, the upcoming Events and the state of the house.
_Avoid_: dashboard, home (that is the Smart Home)

**Smart Home**:
The page that controls the house: lights, fan, climate, Sonos rooms, alarms and Wi-Fi. Today it works on local state only.
_Avoid_: home automation, devices page

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
Reserved for the SDK (Turnkeel): a conversation with an agent that outlives one turn. The Mac app's persisted UI state (route history, panes, font) is `UiState`, not a session.

**Agent**, **Conversation**, **Ticket**, **Automation**, **Connection**:
Reserved for the agent features that will come back on the SDK. Nothing in the product uses them today.

## Avoid everywhere

agentic (say what the agents do), principal (say Owner), session (outside the SDK), firstmate.
