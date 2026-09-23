# Agentinc OS: build the Control shell

## Assignment from Calum

Firstmate: take ownership of getting an implementation agent started on this project. Calum has approved the Control visual direction and now wants a real native Mac app. Write a short implementation plan, then execute it without stopping at the plan or waiting for another routine approval. Deliver a working first increment that we can build on.

Repository: `/Users/calum/code/github.com/0x63616c/agentinc-os`.

Confirmed stack: **Rust + GPUI**. Calum confirmed this interpretation of the dictated “gpio rust.” Do not substitute a webview, Electron, or SwiftUI implementation. Verify the current official GPUI API/examples and choose a pinned reproducible dependency. Report an actual framework blocker if one appears.

The first increment is deliberately the **application shell**, with real tabs and navigation and well-designed placeholder pages. Do not expand this into implementing every feature shown in the design study.

## Visual source of truth

Read and view these files before implementation:

- `../.lavish/agentinc-os-standalone.html`: complete runnable reference, defaults to Control. Open it in a browser and exercise its tabs, new-tab picker, search, sidebar, and Evee panel.
- `../.lavish/agentinc-os.html`, `../.lavish/agentinc-os.css`, `../.lavish/agentinc-os.js`: editable reference source; use the computed Control styles, including later overrides.
- `control-reference/control-desktop.png`: final Control desktop reference, including the revised header Search and adjacent plus.
- `control-reference/control-new-tab.png`: final New tab picker and connected tab contour.
- `../DESIGN.md` and `../.impeccable/design.json`: measured design language and component details.
- `../PRODUCT.md`: product context. The older design-only scope describes the previous phase; this assignment authorizes native shell implementation now.

These relative paths are from this document's `docs/` directory. All assets are also present in the shared local repository. At handoff, the repository has no commits and no configured remote; the prototype files are untracked. Preserve them. Establish a safe initial source checkpoint before using a separate worktree. A remote checkout alone will not contain this design yet.

Build **Control only**. Exclude the web design-study toolbar, concept selector, alternative-layout cards, review form, and reference footer from the native app. The screenshot's app window is the visual target. The screenshot's filled demo content establishes typography and spacing; destination pages can be placeholders in this milestone.

### Preserve precisely

- Black/charcoal solid surfaces, Apple/Vercel restraint, compact system-sans typography, consistent thin outline icons, subtle one-pixel borders. No gradients or ornamental prose.
- Charcoal titlebar/sidebar around the darker inset main panel. Rounded panel corners and fine tonal layering.
- Connected active-tab geometry: rounded upper corners and smooth inverse lower shoulders joining the main panel, with no horizontal seam beneath the active tab.
- Plus immediately beside the tabs. Compact right-aligned Search control with magnifier, label, and keyboard shortcut.
- Sidebar order: Today, Tasks, Agents, then Home, Calendar, Library, My apps; Evee and the notification affordance near the bottom.
- Evee as a separate rounded right-hand panel with a visible 10px gap. Closing it gives its entire width and gap back to the main panel; reopening restores it.
- Reference desktop proportions: 178px Control sidebar, 258px Evee panel, 48px titlebar, 50px inner toolbar, 14px main/Evee corners, 28px by 26px screen padding. Use the actual source/computed styles to resolve details and adapt appropriately when resizing a native window.
- Real native window controls and normal macOS window behavior, aligned with the design rather than drawing a second fake set of traffic lights.

## First milestone: real behavior

1. Launch a real native macOS GPUI app named **Agentinc OS**, opening Today in the Control shell.
2. Navigate every named sidebar destination. Each page gets its own clear title and concise intentional placeholder content using shared components. Do not simulate a connected agent, home device, calendar account, or AI service.
3. Implement selectable, closable workspace tabs. Opening a destination already open activates its existing tab. Keep Today available as in the prototype; closing an active non-Today tab selects a sensible remaining tab. Handle tab overflow without breaking the header.
4. **New tab opens a blank tab with a searchable space picker.** Calum explicitly selected this behavior. Focus the search input. Typing filters destinations; keyboard and pointer selection work. Choosing a destination replaces the blank tab or activates the existing destination and removes the blank. Closing the blank preserves other tabs.
5. Header Search and Cmd+K open a functioning space switcher; support query, keyboard selection, Escape, and pointer selection. Add Cmd+T for the new-tab action.
6. Sidebar and Evee visibility controls work. The central layout responds cleanly to them and to window resizing. Evee's initial panel is a styled placeholder for future assistant integration; do not add fake AI behavior.
7. Persist a small local shell session: open destinations, active destination, and panel visibility. Restore safely and recover from a missing or invalid saved state. Keep this lightweight; no server/database platform is needed.
8. All visible shell controls have deliberate behavior or a clear disabled/placeholder state. Provide proper hover/focus states and keyboard navigation.

## Plan, then build and ship

Create a short tracked plan with three concrete increments: GPUI app/bootstrap; shared Control shell and navigation model; fidelity/verification and runnable delivery. Resolve routine implementation decisions independently. Continue straight into implementation and work through compilation, packaging, and visual verification; a plan, scaffold that does not run, or compilation-only result is not completion.

Keep the Rust project small, with shared style tokens/components and a testable tab/navigation model. Use current official GPUI sources as API authority. Respect repository instructions and use the available Rust and validation workflows where applicable. Avoid speculative plugin systems, service infrastructure, agent runtimes, authentication, or integration adapters in this milestone.

## Completion evidence

- Reproducible build/run instructions and a locally runnable `.app` bundle or equivalent normal Mac launch artifact with its exact path.
- Appropriate formatting, lint/build checks, and meaningful tests of tab creation, selection, replacement, duplicate prevention, closing/fallback, and session restoration.
- **Launch and inspect the actual GPUI app.** Capture screenshots of Today/shell, New tab, another destination, and Evee closed. Compare the native shell against the supplied Control screenshots at matching logical dimensions; correct visible spacing, typography, tab contours, borders, and panel geometry.
- Exercise navigation, search, new/close tabs, sidebar/Evee toggles, resize, and relaunch persistence in the running app. Report what was actually verified and any remaining gaps.
- Finish with the app ready for Calum to open, a concise shipped-feature summary, and source checkpoint/PR/release links where the established Firstmate workflow supports them. There is no remote currently configured; resolve repository setup through the existing fleet conventions and do not invent a remote URL.

After this shell is delivered, tasks, agents, smart home, media, calendar, photo search, and live Evee can be added in separate increments. Do not claim those integrations shipped with this shell.

## Delivered follow-on increment

Evee chat, Keychain connection setup and SQLite-backed Tasks are described in [EVEE_ASSISTANT_HANDOFF.md](EVEE_ASSISTANT_HANDOFF.md), including local acceptance evidence and the live-credential verification gap. The shell-only scope above records the preceding milestone.
