---
name: Agentinc OS
description: A quiet dark workspace for personal work, agents, and everyday life.
colors:
  canvas: '#080808'
  surface: '#101010'
  raised: '#171717'
  hover: '#1d1d1d'
  border: '#262626'
  text: '#ededed'
  muted: '#a0a0a0'
  dim: '#888'
  green: '#88b69b'
  amber: '#d6b77b'
  primary: '#e8e8e8'
  primary-ink: '#141414'
  nav-selected: '#252525'
  evee-mark: '#d5ded8'
  focus-ring: '#b5cabe'
  shell: '#181818'
  workspace-border: '#333'
  calendar-work-bg: '#242b31'
  calendar-work-border: '#48545e'
  calendar-work-text: '#d7e0e7'
  calendar-work-detail: '#bcc7d0'
  calendar-work-dot: '#9caebb'
  calendar-focus-bg: '#282828'
  calendar-focus-border: '#555'
  calendar-focus-text: '#e2e2e2'
  calendar-focus-detail: '#bdbdbd'
  calendar-life-bg: '#302c24'
  calendar-life-border: '#5b503c'
  calendar-life-text: '#e7dfd0'
  calendar-life-detail: '#d0c4ac'
  field-error: '#e6acac'
  delete-text: '#daa7a7'
typography:
  headline:
    fontFamily: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif
    fontSize: 26px
    fontWeight: 500
    lineHeight: 1.15
    letterSpacing: -.035em
  title:
    fontFamily: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif
    fontSize: 13px
    fontWeight: 550
    lineHeight: 1.5
    letterSpacing: -.01em
  body:
    fontFamily: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif
    fontSize: 13px
    fontWeight: 400
    lineHeight: 1.5
  row:
    fontFamily: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif
    fontSize: 12px
    fontWeight: 400
    lineHeight: 1.5
  label:
    fontFamily: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif
    fontSize: 11px
    fontWeight: 400
    lineHeight: 1.5
  mono:
    fontFamily: ui-monospace, SFMono-Regular, monospace
    fontSize: 11px
    fontWeight: 400
  calendar-period:
    fontFamily: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif
    fontSize: 17px
    fontWeight: 500
    lineHeight: 1.5
    letterSpacing: -.02em
  dialog-title:
    fontFamily: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif
    fontSize: 20px
    fontWeight: 500
    lineHeight: 1.5
    letterSpacing: -.025em
rounded:
  checkbox: 4px
  icon: 5px
  control: 6px
  field: 7px
  tile: 8px
  panel: 10px
  dialog: 12px
  window: 15px
  workspace: 14px
spacing:
  unit: 8px
  compact: 12px
  standard: 16px
  panel: 20px
  section: 24px
  wide: 28px
  panel-gap: 10px
components:
  button-primary:
    backgroundColor: '{colors.primary}'
    textColor: '{colors.primary-ink}'
    typography: '{typography.label}'
    rounded: '{rounded.control}'
    padding: 7px 13px
  button-primary-hover:
    backgroundColor: '#fff'
  button-secondary:
    textColor: '#c6c6c6'
    typography: '{typography.label}'
    rounded: '{rounded.control}'
    padding: 7px 13px
  button-text:
    textColor: '{colors.muted}'
    typography: '{typography.label}'
    padding: '0'
  input:
    backgroundColor: '{colors.surface}'
    textColor: '{colors.text}'
    rounded: '{rounded.field}'
    padding: 9px
  nav-item:
    textColor: '{colors.muted}'
    typography: '{typography.row}'
    rounded: '{rounded.control}'
    padding: 7px 10px
  nav-item-selected:
    backgroundColor: '{colors.nav-selected}'
    textColor: '#fff'
  scene-chip:
    textColor: '{colors.muted}'
    typography: '{typography.label}'
    rounded: '{rounded.control}'
    padding: 7px 11px
  scene-chip-selected:
    backgroundColor: '#e0e0e0'
    textColor: '#111'
  room-card:
    rounded: '{rounded.panel}'
    padding: 20px
  status-badge:
    textColor: '{colors.muted}'
    typography: '{typography.label}'
  evee-composer:
    backgroundColor: '#181818'
    rounded: '{rounded.panel}'
    padding: 12px 14px
  workspace-panel:
    backgroundColor: '{colors.surface}'
    rounded: '{rounded.workspace}'
  workspace-tab:
    textColor: '{colors.muted}'
    typography: '{typography.row}'
    height: 38px
  workspace-tab-selected:
    backgroundColor: '{colors.surface}'
    textColor: '{colors.text}'
    height: 40px
  event-dialog:
    backgroundColor: '{colors.raised}'
    rounded: '{rounded.workspace}'
    width: min(440px,calc(100vw - 32px))
  event-field:
    backgroundColor: '#111'
    textColor: '{colors.text}'
    typography: '{typography.body}'
    rounded: '{rounded.field}'
    padding: 9px 11px
  notification-card:
    backgroundColor: '#141414'
    rounded: '{rounded.dialog}'
    padding: 16px
    width: 330px
  calendar-event-work:
    backgroundColor: '{colors.calendar-work-bg}'
    textColor: '{colors.calendar-work-text}'
    typography: '{typography.label}'
    rounded: '{rounded.icon}'
    padding: 4px 6px
  calendar-event-focus:
    backgroundColor: '{colors.calendar-focus-bg}'
    textColor: '{colors.calendar-focus-text}'
    typography: '{typography.label}'
    rounded: '{rounded.icon}'
    padding: 4px 6px
  calendar-event-life:
    backgroundColor: '{colors.calendar-life-bg}'
    textColor: '{colors.calendar-life-text}'
    typography: '{typography.label}'
    rounded: '{rounded.icon}'
    padding: 4px 6px
  evee-panel:
    backgroundColor: '{colors.surface}'
    rounded: '{rounded.workspace}'
    padding: 24px 20px
    width: 258px
  header-search:
    backgroundColor: '#202020'
    textColor: '#aaa'
    typography: '{typography.row}'
    rounded: '{rounded.control}'
    padding: 0 9px
    width: 188px
    height: 30px
  header-search-hover:
    backgroundColor: '#282828'
  space-picker-row:
    textColor: '{colors.text}'
    typography: '{typography.body}'
    rounded: '{rounded.control}'
    padding: 12px 10px
---

# Design System: Agentinc OS

## Current native navigation glossary

- **Route:** a place the user can navigate to. `src/model.rs` defines its stable identity and the `PAGES` catalogue.
- **Page:** the native view that renders a route, such as `TasksPage` or `AssistantPage`.
- **Router:** the private single-tab current route and back/forward history. Legacy session files retain their saved route IDs.
- **Overlay:** the one active dialog, menu, search, or popover per window; `src/overlay.rs` owns dismissal and return focus.

The native sidebar is one uninterrupted list with no group heading or replacement concept word. The navigation control says “Go to…”, its result action says “Open”, and empty results say “No matches.” Keep feature-specific empty text. Dialogs share a centred 440px shell with a visible field label, inline error, and right-aligned actions; destructive actions stay in compact menus with confirmation. The browser prototype notes below are historical where they describe multiple tabs or older notification placement.

## Overview

**Creative North Star: "Control"**

Control is the selected expression: a compact, black personal workspace with a steady sidebar, work in the center, and Evee close at hand. Its Vercel, Apple, and shadcn references land as solid grayscale surfaces, restrained controls, fine separators, and precise alignment. The user’s Multica references refine the connected top tabs, rounded inset workspace, independent Evee card, and compact bottom-left notification.

The main panel and Evee card sit apart with a visible gap; closing Evee returns that space to the main panel. Operational copy stays short, without redundant page subtitles.

Character lives in the open Agentinc monogram, Evee’s three inclined strokes, and the way active tabs meet the content panel. They sit inside the same restrained system as tasks, agents, home controls, and conversation. Quiet and Focus remain comparison layouts, not competing identities. This record describes the browser prototype; all integration state and content are illustrative.

**Key Characteristics:**
- Black, solid surfaces with fine tonal separation.
- Connected top tabs, an inset main panel, and an independent rounded Evee card.
- Sage and amber carry state; muted category colors distinguish calendar events.
- Compact controls, visible keyboard focus, and small custom vector/CSS details.

Ground truth: `.lavish/agentinc-os.css`, `.lavish/agentinc-os.html`, and `.lavish/agentinc-os.js`. The direction contract and `PRODUCT.md` establish the chosen world; the implemented styles establish the values. Frontmatter records a curated, reused set rather than every local shade. Sidecar tonal ramps are synthesized preview swatches, not additional implemented colors.

## Colors

Near-black neutrals establish the workspace; soft white actions and restrained semantic color supply the emphasis.

### Primary

- **Soft action white** (`primary`): filled primary buttons, with dark `primary-ink` text. Hover becomes white.
- **Sage state** (`green`): running/working badges, completed run-step icons, and notification confirmation icons.
- **Amber attention** (`amber`): waiting badges, review indicators, important task metadata, and active room lamp icons.

### Secondary

Calendar categories extend the status palette without changing primary actions. **Work** uses slate blue-gray fill, border, text, detail, and month-dot tokens; **Focus** uses neutral gray fill, border, text, and detail; **Life** uses warm brown fill, border, and cream text/detail. Month events use a slate dot for Work, amber for Life, and the base sage dot for Focus. Preserve this observed mapping rather than inferring identical colors between week blocks and month dots.

Soft red `field-error` marks the event end-time validation message. `delete-text` distinguishes the event deletion action. They are scoped form semantics, not general accents.

### Neutral

- **Canvas / shell / surface / raised**: outer page, shared titlebar/sidebar shell, inset workspace, and search/dialog layers.
- **Hover / selected navigation**: local interaction feedback, with the selected row visibly filled.
- **Border**: fine structural separators across lists, navigation, and containers.
- **Text / muted / dim**: primary content, supporting copy, and compact metadata respectively.
- **Evee mark**: pale green-gray for the assistant’s recurring signature.
- **Focus ring**: pale sage outline shared by keyboard-focusable controls.

**The State Color Rule.** Keep sage for working or connected states and amber for attention or active home cues. Primary actions remain neutral.

## Typography

**Display and body font:** the platform system sans stack in the frontmatter. This is the explicit user-pinned Apple/shadcn direction. It is recorded as a project-specific override, not a general recommendation to use system display typography.

**Label/mono font:** system monospace for keyboard hints and the agent diff sample. No webfont is loaded. The character is quiet and familiar; hierarchy comes primarily from size, weight, spacing, and contrast.

### Hierarchy

- **Headline:** the Control page heading; medium weight and tight tracking. Quiet uses a larger heading (30px), Focus a still larger one (34px), and all three converge on mobile (25px).
- **Title:** compact section headings. Agent-detail headings are a local larger title (19px).
- **Body:** base prose; Evee brief copy loosens to line-height (1.65), rail notes (1.5), and chat (1.8). The inherited brief measure is (66ch), though the current brief is a single status line.
- **Row:** task names, navigation labels, and other compact primary content.
- **Label:** metadata, status, filters, control text, and supporting details. Avoid inventing an uppercase eyebrow tier.
- **Calendar period / dialog title:** the calendar month label and event-dialog heading add intermediate title sizes, recorded in frontmatter.
- **Mono:** shortcuts and change summaries; event times use tabular numerals within the sans stack.

## Layout

The outer review page caps at (1504px) with desktop side padding (40px). The application shell is a bordered, clipped window with a titlebar (48px) and workspace toolbar (50px). The charcoal shell and sidebar share a background. The workspace wrapper is transparent and inset (8px) at the right and bottom. It arranges a flexible main panel and a fixed-width Evee card in a row with a (10px) gap. Both dark panels have their own thin border and rounded corners (14px). The main panel contains the toolbar and screen; hiding Evee removes its footprint and the flex gap. Active top tabs overlap the main panel boundary with smooth inverse shoulders. This wrapper belongs to the browser study, not an implemented native window.

Control uses a sidebar (178px), a flexible central workspace, and an Evee card (258px). Screen padding is (28px 26px). Today places tasks and agents above a two-column agenda/home row; the main grouping gap is (25px), and the lower pair uses (24px). List rows start at (42px) minimum height, while the full task list uses (52px). The eight-pixel base custom property exists in the source, but compact gaps and optical spacing use intermediate values; this is not a strict eight-pixel grid.

At widths up to (1180px), the Control sidebar narrows to (155px), the Evee card to (220px), and the agenda/home pair stacks. Up to (900px), the Control rail is hidden and its toolbar action opens Evee’s full page. Up to (640px), navigation becomes a horizontally scrolling shelf; the shell stacks vertically; screen padding becomes (25px 20px); room/app grids collapse to one column; library tiles retain two columns. Today’s brief and composer become visible in Control on mobile. At widths from (1450px), application minimum height grows from (758px) to (790px), and task rows grow to (46px).

Quiet retains a (210px) desktop sidebar and a more spacious two-column Today canvas. Focus uses a horizontal navigation shelf and a content maximum width (950px). These are retained studies. The removed decorative day thread is not part of the current system.

The connected tab strip scrolls horizontally as spaces accumulate. Its plus control stays immediately beside the tabs, while Search occupies the far right of the header. On mobile its titlebar shrinks to (42px), the workspace inset becomes (5px), and main-panel corners reduce to (10px), with no flex gap. Week/month calendar grids preserve column widths and scroll within their own containers; they do not compress seven days into the mobile screen. The week viewport is (590px) high, (535px) below the rail breakpoint, and (540px) on mobile; hour rows are (48px). The week grid minimum is (610px), rising to (620px) on mobile, while month uses (640px).

App footer slogans/status text are absent. Notification and reset icon controls remain at the bottom of the sidebar; prototype disclosure lives outside the app frame.

## Elevation & Depth

Persistent surfaces are flat, separated by borders and slight tonal changes. Overlays carry soft black shadows. No gradients are used.

### Shadow Vocabulary

- **Command dialog:** `0 10px 24px #0008`, with backdrop `#0009`.
- **Event dialog:** `0 12px 32px #0008`, with backdrop `#000a`.
- **Notifications / toast:** no cast shadow; borders and dark fills define them.
- **Compact comparison rail overlay:** `0 10px 30px #0009`, used by the non-Control rail at narrower widths.
- **Selected review direction:** `0 2px 4px #0003`, confined to the browser study selector.

**The Quiet Depth Rule.** Use borders and adjacent dark tones for persistent workspace surfaces; reserve cast shadows for overlays and the review selector.

## Shapes

Corners scale with the object: compact rectangular controls, slightly softer fields and tiles, and larger enclosing panels/windows. Frontmatter names the observed radius steps. Task checkboxes are compact squares; state dots are circles. Pill-shaped status containers are not used: badges are plain text with a tiny dot.

The Agentinc mark is an open, rounded-stroke SVG monogram above a baseline, rendered in a (25px) box. Its path uses a (28 × 28) viewBox, stroke width (2), and round caps/joins. Evee’s mark uses three narrow rounded strokes inclined (24deg), normally (4px) wide and (12px / 23px / 16px) tall in a (29px) box. Smaller composer/chat variants scale the strokes. UI icons use inline SVG strokes with round joins and caps; the base size is (17px), with contextual variants.

## Components

### Buttons

Compact and matter-of-fact. Primary and secondary buttons share a minimum height (32px) and the frontmatter’s control radius and padding. Primary uses a soft white fill, dark ink, and a matching border; hover brightens the fill. Secondary uses a thin gray border and transparent fill, gaining the shared hover surface. Text actions have no enclosing fill and brighten on hover. Disabled buttons use opacity (.45) and a not-allowed cursor.

Buttons transition background, color, and border color over (150ms). Interactive controls share a keyboard outline (2px) with offset (4px). These focus values live in the sidecar snippets because the frontmatter component schema cannot hold them.

### Chips and status

Scene choices are compact outlined buttons with icon and text; `aria-pressed` drives the selected light fill and dark text. They transition over (180ms). Status badges use a (5px) dot beside an explicit label; waiting is amber and working is sage. A badge is informational, not automatically an action.

### Cards / Containers

The main workspace and Evee are sibling cards rather than a divided single surface. Evee shares the workspace fill and border, has its own rounded corners, and stays separated by the layout gap. Its desktop padding is (24px 20px), reducing to (22px 16px) below the intermediate breakpoint.

Room controls use a thin border and panel radius with the frontmatter’s padding. They are flat and inherit the surrounding surface. Tasks and agent previews usually use rows and separators. The Evee composer is a distinct dark field container. Command and event dialogs use the overlay shadow vocabulary; notification cards and toasts remain flat.

### Inputs / Fields

Standard agent/event fields use surface fill, a fine border, the field radius, and muted placeholder text. Their observed padding is in the frontmatter. Search and Evee inputs shed their individual borders inside a containing field. Labels remain visible in the new-agent/event forms. The event dialog uses a darker field variant and a soft-red message when end time does not follow start time. Inputs use native required/date/time validation as well. Disabled buttons retain the shared subdued treatment.

### Navigation

Control’s navigation uses icon plus label, compact row padding, and a filled active state. Task counts are tabular; pending review uses an amber dot. Active navigation carries `aria-current="page"`. The mobile shelf scrolls rather than compressing labels into illegibility. Command search opens a native HTML dialog with the keyboard shortcut, supports moving to the first result, and restores navigation focus through the screen container.

### Evee

Evee stays alongside work in a separate desktop Control card and remains available as a dedicated page. Its contextual introduction is a single line: “One review waiting.” or “You’re all caught up.” Its mobile Today brief is also one line, with an “Ask Evee” action. The rail title is simply “Evee”, followed by status, a short “Recent” list, and the composer. Its composer uses the placeholder “Ask Evee…” and a small light send control; replies are scripted. The assistant’s first and third strokes shift by two pixels on relevant hover targets, using (180ms) and `cubic-bezier(.16,1,.3,1)`. The latest chat message arrives with a three-pixel vertical movement over (220ms). Reduced-motion preferences remove transitions and animations.

### Connected tabs

Opening a space adds a top tab; selecting it changes the current page. Today remains available while other tabs have SVG close controls. The adjacent plus control opens a New tab page containing a searchable space picker. Selecting a space replaces that blank tab; if the space is already open, it activates the existing tab and removes the blank one. Closing another tab preserves the remaining open spaces. Search is a separate right-side control that opens command search. Resting tabs are transparent within the shell; the active tab adopts the workspace fill and border, with rounded top corners (10px) and an open bottom edge. Desktop resting/active heights are (38px / 40px); both are (36px) on mobile. Desktop tab label buttons have a minimum width (140px), or (112px) when a close button shares the tab. Mobile equivalents are (96px / 76px). Label padding is (10px 16px) on desktop and (8px 10px) on mobile. The tab strip gap is (8px), reducing to (4px) on mobile. Inactive close buttons appear on hover or focus; mobile keeps them visible. The active tab uses two inverse geometric SVG shoulders (12px square) to continue the panel’s border, with a (2px) tab-strip overlap. They are geometric interface assets embedded in CSS, not illustration assets. On mobile the shoulders are hidden and the strip overlap is removed.

### Header Search and New tab

Search is a bordered dark button with a magnifier, the visible label “Search”, and a right-aligned keyboard hint. Its dimensions and fill are recorded in frontmatter; the border brightens alongside the fill on hover. At widths up to (900px), it narrows to (142px). Up to (640px), it becomes a centered icon-only control (28px wide), retaining its accessible name and keyboard action.

The New tab picker is a focused page within the main panel, not a dialog. It caps at (420px), uses desktop margins (65px auto 40px), and reduces those margins to (18px auto 30px) on mobile. “Open a space” is its heading (24px). The search field has a bottom border, no individual input box, and a pale sage focus-within border. Results use an icon, a space name, and a trailing SVG arrow; their padding and typography are recorded in frontmatter.

### Calendar and event modal

Week and Month share local events with Today’s agenda. Week uses a conventional hour grid, sticky day header (65px), neutral current-date circle, and category-tinted event blocks. Events occupy time-proportional heights and divide into lanes when they overlap. Short events hide secondary text. Month uses a seven-column grid with two-line event labels, small category dots, and subdued adjacent-month cells. Date controls create events; event controls edit them.

Create and edit use the same native dialog, sized by the frontmatter token, with a concise title and no explanatory subtitle. Its form padding is (24px), reducing to (20px) on mobile. Event name, date, start/end time, and optional location are labeled; start/end fields share two columns with a (14px) gap. A separated footer places Cancel and Create/Save together, with Delete appearing only while editing. Events stay local to the browser; new ones receive the Life category, and editing retains an existing category. Escape, Cancel, close, and outside-dialog interaction dismiss the modal. No drag/resizing interaction is implied.

### Notifications

The compact notification card anchors inside the app frame at the bottom left (14px inset). It pairs a small sage icon tile with a short title, one supporting sentence, a secondary action, and a close control. Its maximum width is constrained by the shell, and the nominal width is in frontmatter. It opens from the sidebar bell for the pending review, links to the run, and can be dismissed. Reviewed state removes the bell’s attention dot.

Transient action feedback also appears at the bottom left, but uses a fixed viewport inset (24px desktop, 16px mobile), a dark fill, thin border, and no shadow. It enters over (180ms) from a (12px) vertical offset and is announced through a polite live region.

Page headings render only the title and any action. Empty states report the state directly, for example “All caught up.”, “No completed tasks.”, or “No matches.” App entries omit redundant descriptions.

## Native interaction principles

### Native visual tokens

`src/palette.rs` is the native color source of truth. The native shell remains darker than the older browser-study frontmatter above. Selection, hover, text selection, and keyboard focus use neutral tones; focus keeps a distinct, brighter ring. `src/style.rs` owns the shared component and spacing vocabulary.

| Role | Native value |
|---|---|
| Shell / surface / border / text / muted | `SHELL` / `SURFACE` / `BORDER` / `TEXT` / `TEXT_MUTED` |
| Hover / row hover / selected / focus surface / focus ring | `HOVER` / `HOVER_ROW` / `SELECTED` / `FOCUS_SURFACE` / `FOCUS` |
| Error text / error border | `ERROR` / `ERROR_BORDER` |
| Primary fill / ink; destructive fill / text | `PRIMARY` / `TEXT_ON_PRIMARY`; `DESTRUCTIVE` / `DESTRUCTIVE_TEXT` |
| Dialog / menu surface / overlay border | `SURFACE_RAISED` / `SURFACE_MENU` / `BORDER_OVERLAY` |
| Page inset X / Y; panel gap | `26px` / `28px`; `10px` |
| Header control / control height / field height | `30px` / `32px` / `42px` |
| Panel / dialog / menu / control / field radius | `14px` / `12px` / `6px` / `6px` / `7px` |
| Body / label / caption / dialog title | `13px` / `12px` / `11px` / `18px` |
| Hover / grip / panel / message motion | `140ms` / `160ms` / `180ms` / `220ms` |

The shared button contract handles click, Enter/Space, disabled activation, and focus styling. Feature views still compose their own rows and hover fades. The supplied button label is retained as a semantic requirement, but GPUI 0.2.2 currently does not expose it in the observed macOS accessibility tree; accessibility remains an explicit framework investigation. Reduce Motion disables the hover fade and skips motion transitions. Preserve optical exceptions and avoid a runtime theme loader.

- Show a page name once, in the connected tab. Start page content with its action or substance, without repeating the tab name in a breadcrumb or heading.
- Open create and edit forms in a centred modal instead of inline fields. Escape and outside click dismiss them.
- Put destructive row actions behind a three-dot menu and confirm them before applying them.
- Omit filler count lines. Prefer icons to text labels for familiar compact actions, such as send.
- Keep list selection neutral grey with one active row, and avoid slide-in entrance motion.

## Do's and Don'ts

### Do:

- Do preserve the black-only, solid-surface palette and consistent compact spacing.
- Do reuse the open monogram, three-stroke Evee mark, connected tab/content geometry, and separated Evee card.
- Do pair state color with text, checked state, or a clear control label.
- Do keep keyboard focus visible and honor reduced motion.
- Do keep prototype disclosure outside the app frame while interactions use synthetic local state.

### Don't:

- Don't add gradients or decorative imagery to this established workspace.
- Don't turn every content group into a raised card; task and agent lists use separators.
- Don't use semantic state or calendar category colors as general primary-action fills.
- Don't restore the removed day timeline or ornamental footer slogans.
- Don't present scripted replies or local demo statuses as real integration activity.

- Don't add redundant page subtitles, verbose empty-state prose, or a second introduction beneath Evee’s name.

Not canonized: dormant timeline/footer CSS is not part of the rendered system. System display type records the explicit user choice; supplemental reference arrows and keyboard labels do not define the SVG icon vocabulary. No source repair was made by this documentation pass; synthetic content and browser-only behavior remain deliberate scope limits.
