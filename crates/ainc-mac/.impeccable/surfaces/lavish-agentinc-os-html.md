---
version: 1
slug: "lavish-agentinc-os-html"
primary_target: ".lavish/agentinc-os.html"
related_targets: []
---

# Agentinc OS visual exploration
Mode: Operate. Scope: three browser concepts for user review, not production implementation.
## Direction contract
THESIS: A personal Mac workspace with daily life and agents close at hand. Compare Quiet, Control, and Focus layouts, all opening Today, before selecting a permanent identity.
OWN-WORLD: User-pinned minimal shadcn-like design, solid surfaces, consistent spacing, system sans, fine separators, restrained semantic sage and amber. Black dark mode only per user correction; Vercel and Apple references. No gradients.
STORY: Switch concepts, navigate tasks/agents/home/Evee, try local interactions, and submit a preferred direction with notes.
FIRST VIEWPORT: Compact review toolbar above a complete Mac window. Quiet opens a 210px sidebar and generous Today canvas with tasks, schedule, agent review, home and music. Control opens a denser Today with Evee in a context rail. Focus moves spaces into horizontal navigation above Today. Signature interaction: concept switching changes structure and density, retaining the same sample world.
FORM: Seven grounded candidates: daily planner, project outliner, inbox triage, app launcher, conversation notebook, desktop workspace, room controller. Assigned index 6, seed 3fb190d0: desktop workspace. User-pinned shadcn/minimal constraints override foreign material styles. Prototype all three openings within this workspace for concrete review; none is accepted yet.
Challenger judgment: HyperCard is competitive in personal extensibility but loses contemporary familiarity; retain navigable app spaces. Oscilloscope declined for both audience fit and task clarity; adopt explicit state labels. Streaming catalog declined as whole-app navigation; reserve visual browsing for the photo library. Bitmap specimen declined on daily readability; keep disciplined type scale. Industrial quotations declined on visual restraint; keep direct action labels. Cyclorama declined against no-gradient brief and task clarity; keep named home scenes, no theatrical materials.
FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance

## Review decision
User selected Control: "really clean", "really simple", "a little bit more design characteristic ... more unique". Preserve layout, strengthen only the identity details. Control is the initial view. Added a custom open monogram, sample day timeline, and restrained Evee/scene state transitions. No comp image was commissioned; user selected from rendered code prototypes.

## Latest review supersedes the timeline
User feedback IDs 3, 5, 6, 9, and freeform Multica screenshots: remove timeline; concise Evee; week/month calendar; modal create/edit; connected top tabs and rounded inset workspace with bottom-left notifications. Control remains the base. Signature details now live in the connected tab/content geometry and Evee mark. Calendar events are shared between Today, Week and Month and editable in one modal.

## Applied feedback receipt
- 3: removed day timeline from Today.
- 5: Evee rail intro reduced to one status line.
- 6: Calendar has week time grid and month grid with shared events and period navigation.
- 9: create/edit use a native dialog with date, start/end, location, cancel, and explicit validation.
- Multica freeform: connected tabs, rounded inset workspace, neutral sidebar/background layering, bottom-left notification, based on supplied screenshots.
- 57, 66, 65: removed the three requested footer labels; prototype disclosure remains outside the app frame.
- Latest 1 and 3: shortened task empty states and removed redundant page subtitles across the app.
- Latest freeform: Evee is a separate rounded card with a 10px gap; closing it gives the whole width to the main panel. Active tab shoulders use smooth inverse corners and overlap the panel border.

- Header review 1/3 and freeform: wider tabs; plus adjacent to open tabs; compact right-side Search with shortcut; New tab opens a searchable space chooser. Selecting a space replaces the blank tab or activates an existing space. User confirmed the core visual direction while leaving further details open.

## Pass close
User confirmed blank tab with space picker and requested wrapping up this pass. Scoped finish review: ship. Browser verification: 26 interactions and 21 page/direction combinations passed at both 1440px and 390px; targeted new-tab checks passed. Standalone export refreshed with zero unresolved local assets. Product implementation remains outside this design pass.
