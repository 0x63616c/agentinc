# Project agent memory

This file is the project's committed home for project-intrinsic agent knowledge: build, test, release, architecture, and sharp-edge notes that should travel with the code.

- The native scope and acceptance gates are in `docs/CONTROL_BUILD_HANDOFF.md`; `.lavish/` remains the visual reference.
- Build the native bundle with `scripts/bundle.sh`; the pinned toolchain and dependencies are in `rust-toolchain.toml`, `Cargo.toml`, and `Cargo.lock`.
- GPUI requires the `font-kit` feature on macOS when default features are disabled; `docs/verification/BLOCKER.md` records the resolved diagnosis.
- Visual rule: keep control edge insets even, especially top/bottom/right around header icon buttons. No hover tooltips unless explicitly requested. Keep small interactions fluid with brief, restrained transitions. Approved live refinements are recorded in `docs/verification/STATUS.md`.

## Maintaining this file

Keep this file for knowledge useful to almost every future agent session in this project.
Do not repeat what the codebase already shows; point to the authoritative file or command instead.
Prefer rewriting or pruning existing entries over appending new ones.
When updating this file, preserve this bar for all agents and keep entries concise.
