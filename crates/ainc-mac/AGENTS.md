# Project agent memory

This file is the project's committed home for project-intrinsic agent knowledge: build, test, release, architecture, and sharp-edge notes that should travel with the code.

- Product ownership and daemon configuration: `../../docs/phase-2-ownership.md`. Native tests must use an isolated daemon discovery file; never import the regular Application Support profile.
- The native shell scope is in `docs/CONTROL_BUILD_HANDOFF.md`; Evee setup is in `docs/EVEE_ASSISTANT_HANDOFF.md`; current Tickets and Comments acceptance is in `docs/verification/PHASE3.md`. `.lavish/` remains the visual reference.
- From the workspace root, build the native bundle with `crates/ainc-mac/scripts/bundle.sh`; the pinned toolchain and lockfile are at the workspace root.
- The opt-in driver setup, protocol, upstream seam and acceptance commands are in `docs/GPUI_PILOT.md`; normal bundles must keep `automation` off.
- Keep GPUI `font-kit` enabled on macOS. Current upstream pin, native acceptance and the main-thread Metal regression command are documented in `docs/verification/GPUI_UPGRADE.md`; inspect saved full PNGs at logical resolution before diagnosing missing regions from inline previews.
- Workspace Linux CI runs fmt, clippy and default tests; use the macOS rendered and pilot commands in `docs/GPUI_PILOT.md` for native acceptance.
- Visual rule: keep control edge insets even, especially top/bottom/right around header icon buttons. No hover tooltips unless explicitly requested. Keep small interactions fluid with brief, restrained transitions. Approved live refinements are recorded in `docs/verification/STATUS.md`.

## Maintaining this file

Keep this file for knowledge useful to almost every future agent session in this project.
Do not repeat what the codebase already shows; point to the authoritative file or command instead.
Prefer rewriting or pruning existing entries over appending new ones.
When updating this file, preserve this bar for all agents and keep entries concise.
