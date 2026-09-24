# App identity and profile card verification

These are real GPUI Metal captures from the rendered fixture at 2× backing scale.
The `main` captures used the sidebar and rendered runner from commit
`0299c1d52af12cddc51e4eb853d4baf43d014806`; the `development` captures use
the new profile card. Both were rendered in this worktree with the same fixture.

| Logical window | Original profile card | Profile card with version |
| --- | --- | --- |
| 1360 × 828 | [main-1360.png](main-1360.png) | [development-1360.png](development-1360.png) |
| 1160 × 728 | [main-1160.png](main-1160.png) | [development-1160.png](development-1160.png) |

At both widths the avatar remains left of the text within the original 50 px
sidebar row and 40 px button. The username and muted `0.1.0-dev` share a left
edge inside that button. The rendered runner checks separate avatar, username,
and version regions in all 38 frames, and Pilot exposes `sidebar.version` as a
label named `0.1.0-dev`.

The native development bundle also displayed an `AgentInc` app menu with
`About AgentInc` first. AppKit's About panel showed
`AgentInc Version 0.1.0-dev (55c8e8f602c3)` and the copyright. During the
side-by-side check, the installed 0.1.0 production app and development bundle
ran concurrently with different bundle IDs, discovery locks and loopback API
ports. Production was launched with data-path overrides into this worktree;
the installed app and its existing data were not changed.
