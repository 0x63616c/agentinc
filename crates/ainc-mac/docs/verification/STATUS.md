# Native Control shell — acceptance report

Current GPUI dependency and regression acceptance: [GPUI_UPGRADE.md](GPUI_UPGRADE.md). The report below is the original 0.2.2 shell milestone and retains its historical commands and scope.

Verified locally on macOS 27.0 (26A428), 23 September 2026. This is a working Rust + GPUI application shell. All destination content is explicitly placeholder content; no tasks, agents, home, media, calendar, library search or live assistant integration is claimed.

## Reproduce

From the repository root, run `scripts/bundle.sh`, then `open 'dist/Agentinc OS.app'`. The verified artifact is:

`/Users/calum/.treehouse/agentinc-os-8663d6/1/agentinc-os/dist/Agentinc OS.app`

Rust 1.94.0, GPUI exactly 0.2.2 and Cargo.lock are pinned. The final artifact uses the official dependency, with `font-kit` and `runtime_shaders`; there is no local GPUI patch. The app is ad hoc signed for local use, not notarized for distribution. Finder launch and the rounded-square Evee icon were inspected in the actual Finder window. The final bundle was opened, navigated through its header Search, and its notification placeholder was exercised ([capture](acceptance-shipped.png)).

## Checks completed

| Check | Result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo test --locked` | 12 passed: tab selection/replacement, duplicate picker behavior, close/fallback/last-tab guard, history, persistence/recovery, filtering and Unicode word boundaries |
| `cargo clippy --locked --all-targets -- -D warnings` | Passed |
| `scripts/bundle.sh` | Built native `.app` |
| `codesign --verify --deep --strict --verbose=2 'dist/Agentinc OS.app'` | Passed |

No hosted CI is configured. These are local checks, not a claim of hosted CI or release signing.

## Actual native interaction acceptance

An isolated `Agentinc QA` bundle used the same application binary with a separate bundle identifier, explicit window title and `AGENTINC_SESSION_PATH`. Inputs were delivered to the actual macOS window through CUA. Native window captures were matched by owner and exact title, then reduced from Retina pixels to logical dimensions. Session JSON was inspected to corroborate the visible result.

- Navigated all seven sidebar destinations with Cmd+1…7, and clicked sidebar Tasks. Opened Evee and Settings through search; each has distinct, clearly planned content.
- Created tabs with Cmd+N/Cmd+T and selected destinations through text/Return and pointer selection. Choosing an already-open Home removed the blank and selected Home. Closed tabs through the close button and Cmd+W, checked the neighboring fallback, and confirmed repeated close leaves the last tab open.
- Opened nine destination tabs and switched adjacent tabs. Overflow scrolls the active tab into view while keeping plus and Search available ([overflow](acceptance-tab-overflow.png)).
- Navigated within a tab, went Back with Cmd+Option+Left and Forward with the header button. Closing another tab preserved the remaining tab's history.
- Opened command search with Cmd+K and the header Search button, filtered destinations, selected them and dismissed with Escape. Checked no-results behavior. Exercised Cmd+Backspace, Option+Backspace, line selection, cut/paste and undo/redo against real input.
- Verified bounded Tab/Shift+Tab focus in command search and the blank-tab picker, including empty results. Forward traversal wrapped to the palette close button; reverse traversal selected Settings; no-results traversal retained usable input. Background controls were excluded ([palette](acceptance-palette-focus.png), [no results](acceptance-no-results.png)).
- Toggled the sidebar with Cmd+B and Evee with Cmd+Shift+E. Content reclaimed the closed panels' space ([Evee closed](acceptance-evee-closed.png), [both closed](acceptance-panels-hidden.png)).
- Dragged Evee from 258 to 353 logical pixels, then back to 258. Verified the persisted width and visible result ([resized Evee](acceptance-resized-evee.png)).
- Used native window zoom to resize the app to 1728×998 logical pixels and inspected the layout ([window resized](acceptance-window-zoom.png)).
- Quit, waited for process exit and relaunched the isolated app. Today/Calendar tabs, active Calendar, hidden Evee, sidebar visibility, history and 353px Evee width restored. The saved file compared byte-for-byte equal before/after relaunch ([restored Calendar](acceptance-persistence.png)). Missing/invalid state recovery is covered by the model tests; initial no-file launch also opened Today.

## Visual comparison

Opened and exercised the standalone browser reference: new-tab picker, query/selection, command search, sidebar and Evee toggles. Read the editable Control style overrides and compared both committed reference PNGs with native screenshots at the same logical scale.

| View | Native capture | Reference app crop |
| --- | --- | --- |
| Today, 1360×829 | [Today](acceptance-today.png) | [Control desktop](reference-control-desktop-shell.png) |
| New tab, common 1360×808 bounds | [New tab crop](acceptance-new-tab-comparison.png) | [Control new tab](reference-control-new-tab-shell.png) |

Reference crops start at (40,126) in the supplied PNGs and exclude design-study chrome. The desktop app is 1360×829; the reference new-tab app is 1360×808. The new-tab comparison crops the native image's bottom 21px to compare identical bounds without stretching; the [full native capture](acceptance-new-tab.png) remains available. Earlier destination/panel captures are 1360×809 from the preceding default height; behavior and styling are unchanged.

The native shell retains the 178px sidebar, 258px default Evee width, 10px gap, 48px titlebar, 50px inner toolbar, 14px panel corners and 26px horizontal/28px vertical content padding. The 420px picker aligns with the reference. Panel borders, active-tab join and tab shoulders were inspected at magnification, including hovering inactive tabs on each side of the active tab ([left](tab-contour-left-detail.png), [right](tab-contour-detail.png)). The selected contour paints above neighboring hover shoulders and has no horizontal seam. The purple upper-left badge in automated captures is macOS screen-sharing UI, not app chrome.

Approved live refinements intentionally differ from the original screenshots: darker surfaces; Back/Forward header controls; wider Search and rounded shortcut badges; notifications beside Search; World Wide Webb workspace identity; locally read profile/photo opening Settings; actual Evee artwork and padded rounded-square app icon; 10px even Evee logo insets; animated sidebar/Evee visibility; resizable Evee; explicit planned placeholder sections. Sidebar navigation replaces the current tab, explicit new-tab actions create blanks, and Today may close when another tab remains. Hover tooltips were removed. Command-held navigation badges are implemented; shortcut actions were exercised, but sustained modifier-badge visibility was not separately captured.

## Resolved diagnostics and limits

The original invisible-text problem was fixed by enabling GPUI's `font-kit` feature. It was not a reason to replace GPUI or upgrade its pinned version.

Earlier entries in this report incorrectly interpreted large screenshot previews as partial native redraws. The **same saved PNG files** show complete text and panels when reduced to logical dimensions before viewing. A temporary, ignored diagnostic copy of GPUI also recorded complete scene counts and successful Metal rendering/readback. This resolves the capture blocker; it does not establish a native renderer defect. All diagnostic dependency changes were removed, Cargo.lock restored, and the final official build verified. Older tracked diagnostic images are historical evidence, not the final acceptance set linked above.

This acceptance covers pointer/keyboard operation and observed layout/transitions. It does not establish a frame-time benchmark, cross-version macOS compatibility, VoiceOver support for custom GPUI elements, full IME-language coverage or notarized distribution. CUA exposes native window/menu accessibility, but not a semantic tree for the custom shell controls. No external integration functionality is present.
