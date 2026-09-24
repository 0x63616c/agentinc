# Control icon library

- Drawing, naming, integration, and extension rules: `STYLE.md`.
- SVGs are the source artwork. Run `python3 assets/icons/build-sheet.py` from the repository root to validate coverage and regenerate the HTML sheet; capture the PNG with the commands in `STYLE.md`.
- Keep this library independent of shell wiring; the native app assets outside this directory belong to the separate shell implementation.

## Maintaining this file

Keep this file for knowledge useful to almost every future agent session in this project.
Do not repeat what the codebase already shows; point to the authoritative file or command instead.
Prefer rewriting or pruning existing entries over appending new ones.
When updating this file, preserve this bar for all agents and keep entries concise.
