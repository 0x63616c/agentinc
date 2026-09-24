# Agentinc OS

<!-- impeccable:product-schema 1 -->

## Platform
web

The current deliverable is a browser-viewable design prototype for a future Mac app.

## Stack
Delegated by the brief: no stack preference for this prototype. Static HTML, CSS, and JavaScript keep the visual exploration portable.

## Users
Calum, managing personal work, home, coding agents, and everyday life.

## Product Purpose
One personal app for tasks, coding agents, life agents, smart home, and future personal apps. The assistant is named Evee.

## Capabilities and Constraints
Explore tasks, agent management, Hue/home controls, Sonos/Spotify, events, and image search. Integrations and version scope are undecided. Prototype actions use illustrative local state and never operate devices or services.

## Brand Commitments
Product name: Agentinc OS. Assistant: Evee. Minimal, polished, consistent spacing; shadcn is a named reference. Black/dark mode only, inspired by Vercel and Apple. Small distinctive details matter, with consistency across the product. No gradients. Use Impeccable guidance. Show visual alternatives before product implementation.

## Evidence on Hand
User objective and attached pasted-text-1.txt. Repository initially empty. An earlier external mockup exists but has no confirmed design approval in this thread. All tasks, events, agent runs, and device states in this prototype are synthetic examples.

## Confirmed Opening Screen
Today: tasks, agents, and home at a glance. Evee remains easy to reach.

## Confirmed Visual Direction
Control selected in the visual review: dense but clean, with Evee alongside. Preserve the clean layout and add restrained distinctive details. All three studies remain available for comparison.

## Open Decisions
Final refinements, native technology, integration credentials, and first release scope. Initial concepts are proposals, not accepted decisions.

## Review refinements
Remove the decorative day timeline. Reduce Evee sidebar prose. Calendar uses familiar week/month views and modal event creation/editing. Follow the user-supplied Multica screenshots for connected top tabs, inset rounded workspace, sidebar/background layering, and compact notification styling; place notifications at the bottom left per the user’s wording. These replace the earlier timeline proposal.

Remove ornamental footer copy: Local preview, Your space in sync / Demo, and Made for a life beyond the tabs. Keep prototype disclosure outside the app frame.

Evee sits in a separate rounded panel with a visible gap. Closing it expands the main panel to occupy the space. Active tabs join the main panel with smooth inward corners. Keep operational copy concise: omit redundant page subtitles and use short empty states.

The user confirmed the current layout as the core visual direction. Header refinement: a compact Search control at the right, plus beside the tabs, and a functional New tab space picker. Other product details can remain undecided.

Confirmed new-tab behavior: blank tab with a space picker. This prototype refinement pass is wrapped up at the user’s request.

## Native shell implementation authorized
Calum has asked Firstmate to plan and execute the first real macOS app increment in Rust + GPUI, matching Control. Initial delivery is the shell, working tabs/new-tab space picker/search/navigation, collapsible sidebar and separate Evee panel, with placeholder pages. The full assignment and completion checks are in `docs/CONTROL_BUILD_HANDOFF.md`. The existing web prototype remains the visual source of truth; feature integrations follow later.
