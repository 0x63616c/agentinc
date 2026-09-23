# Agentinc OS

A design prototype for Calum’s personal Mac app. Evee is the assistant.

**Current direction:** Control, a black, minimal workspace with Today at its center and Evee alongside. Selected during visual review. Quiet and Focus remain available as comparison layouts. Product implementation and integration choices are still open.

## Open the prototype

Open `.lavish/agentinc-os.html` in a browser. All assets are local; no install or build is needed. `.lavish/agentinc-os-standalone.html` is an exported single-file copy for portability; regenerate it with `lavish-axi export .lavish/agentinc-os.html --out .lavish/agentinc-os-standalone.html` after editing the source.

For the collaborative review surface:

```sh
lavish-axi .lavish/agentinc-os.html
```

Or serve the files locally:

```sh
python3 -m http.server 8080 --directory .lavish
```

Visit `http://localhost:8080/agentinc-os.html`.

## What you can try

- Switch Quiet / Control / Focus without losing sample state.
- Add and complete tasks, filter completed tasks, and find them again after reload.
- Inspect sample coding/life agents, mark a review complete, pause a run, and create draft agent instructions.
- Change home scenes, room switches and brightness; preview music playback/track controls.
- Open Evee and try scripted conversations, with links back into the workspace.
- Browse the calendar by week or month; add, edit, or delete sample events in a modal. Changes also appear in Today.
- Open a new tab with the searchable space picker; open and close spaces as workspace tabs; toggle the sidebar and preview a notification from the bell.
- Search the illustrative photo-library tiles.
- Press Cmd+K or Ctrl+K to jump between spaces.
- Queue a direction and notes through Lavish, or copy the prepared feedback when running standalone.

This is not a connected product: all names, times, tasks, runs, checks, and device states are illustrative. No devices are actuated, no music is played, and no AI or integration service is called. State is stored only in this browser’s local storage; the reset control restores the sample data. Library tiles explore layout, not actual photo retrieval.

## Files

- `PRODUCT.md`: confirmed product brief and review decisions.
- `DESIGN.md`: implemented design language, recorded after review.
- `.lavish/agentinc-os.{html,css,js}`: portable prototype.
- `.impeccable/surfaces/lavish-agentinc-os-html.md`: direction and refinement record.
- `tests/preview-smoke.js`: browser interaction checks.

## Verify

```sh
node --check .lavish/agentinc-os.js
chrome-devtools-axi open http://localhost:8080/agentinc-os.html
chrome-devtools-axi emulate --viewport '1440x1100x1'
chrome-devtools-axi run < tests/preview-smoke.js
chrome-devtools-axi emulate --viewport '390x844x1,mobile,touch'
chrome-devtools-axi run < tests/preview-smoke.js
```

The smoke check resets sample data, exercises 26 task/agent/home/event/search/chat/tab checks (including invalid event times, editing, and agent reset), verifies rendered navigation and document overflow across all 21 page/direction pairs, then restores the sample data. Run it in a separate preview browser, not while someone is reviewing unsaved sample edits.

References: [Vercel/Geist](https://vercel.com/geist/introduction), [Apple/macOS](https://developer.apple.com/design/human-interface-guidelines/designing-for-macos), [shadcn/ui](https://ui.shadcn.com/blocks), [Linear navigation](https://linear.app/now/how-we-redesigned-the-linear-ui), [Impeccable](https://github.com/pbakaus/impeccable).
