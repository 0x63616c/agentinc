# App identity and sidebar verification

These are GPUI Metal captures at 2× backing scale. The `main` captures use
commit `0299c1d52af12cddc51e4eb853d4baf43d014806`; the development
captures use this branch's compiled development identity and the same fixture.

| Logical window | Original sidebar | Sidebar with version |
| --- | --- | --- |
| 1360 × 828 | [main-1360.png](main-1360.png) | [development-1360.png](development-1360.png) |
| 1160 × 728 | [main-1160.png](main-1160.png) | [development-1160.png](development-1160.png) |

The 24 px profile photo, name, and muted `0.1.0-dev` are stacked on the
same left edge. The profile still opens Settings. The rendered runner checks
separate photo, name, and version regions across 41 frames. The Pilot initial
snapshot exposes `sidebar.version` as a label named `0.1.0-dev`. The broader
Pilot acceptance run reaches that snapshot but later fails on a Ticket
acknowledgement timeout; its hidden-window test reports a frontmost-app change.
The [development tooltip capture](version-tooltip.png) shows the full commit
above the profile without covering its text.

The native development bundle displayed an `AgentInc` app menu with
`About AgentInc` first. AppKit's About panel showed
`AgentInc Version 0.1.0-dev (55c8e8f602c3)` and the copyright. During the
side-by-side check, the installed 0.1.0 production app and development bundle
ran concurrently with different bundle IDs, discovery locks, and loopback API
ports. To protect existing production data during the check, production was
launched with data paths overridden into this worktree; neither the installed
app nor its existing data was changed. The default production and development
paths are separately covered by the channel identity tests.
