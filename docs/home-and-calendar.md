# Dashboard, Smart Home and Calendar

AgentInc's first life-OS surfaces beyond development. The Dashboard is the home
screen; Smart Home and Calendar are full pages. All three read and write through
the daemon, so the CLI and Evee get the same commands as the app.

## What the control center offers, and what AgentInc exposes

World Wide Webb's control center (`0x63616c/world-wide-webb`) owns Home Assistant.
AgentInc calls its tRPC API and never talks to Home Assistant, so it needs no Home
Assistant token.

| Control center | AgentInc |
| --- | --- |
| `controls.toggle {key, on}` for groups `all`, `lamps`, `bedroomLamps`, `otherLamps` (living room), `lights` (both fixtures), `ceiling`, `cabinet` | Switches: All lights, Lamps, Bedroom lamps, Living room lamps, and each kitchen light: Ceiling and Under cabinet. |
| `controls.list` group state (`on`, `pending`) | Live switch state, polled while the page is open. |
| `climate.get` ambient temperature, action, mode, target or range | Indoor temperature and what the system is doing. |
| `climate.setMode`, `setTarget`, `setRange` (whole °F, 67–77, range at least 2° apart) | Mode (Off, Cool, Heat, Auto), target, and the Auto range. |
| Lamp brightness, white temperature, scenes, party mode, per-zone climate, weather, Sonos | Not exposed yet. |

The control center has no call for one individual lamp, so the kitchen lamp is
switched with the other living-room lamps. It has no calendar source; its old
hand-entered events table was dropped.

## Connecting

Settings → Smart Home stores the control center URL in Postgres and the optional
Cloudflare Access service token (client ID and secret) in the login Keychain,
under the product bundle ID. The public URL `https://app.worldwidewebb.co` sits
behind Cloudflare Access and needs a service token; an in-cluster or Tailscale URL
without Access needs none. Secrets never reach product records, logs, workflow
history or API responses.

## Calendar

Events live in `calendar_events`, scoped to workspace and user. People and Evee can
create, edit and delete AgentInc events. The Mac app reads upcoming events from the
macOS calendar store (EventKit, which includes synced iCloud and Google calendars)
after the user grants Full Access, and hands each snapshot to the daemon as an
import. Imported events are read-only mirrors updated by later imports; edit them
in Calendar.

## Durable actions

Smart Home commands and calendar imports are **durable actions**: the command
commits a `durable_actions` row and its receipt in one transaction, then the
daemon starts it as a Temporal workflow under the row's ID. The workflow retries
the effect; effects are idempotent (a switch is set to a state, an import upserts
by external ID), so a retry after a lost acknowledgement cannot double-apply. Each
action appears in Temporal's run history as `home-<id>` or `calendar-import-<id>`.
