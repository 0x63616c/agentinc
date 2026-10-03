# AgentInc design system

The native app is built from one token set and one component library, both in
`crates/ainc-mac/src/ui/`. Tokens name every value; components compose tokens;
pages compose components and own only their own composition. The app renders the
whole library on its **Components** page (open ⌘K and type "Components"), and the
rendered-shell test captures every section of it.

## Direction

Black canvas, white action. Pure black behind the window, near-black surfaces on
it, thin `BORDER` hairlines instead of fills, one white `PRIMARY` button per
surface with black ink, and restrained grays for everything secondary. Status
colors stay muted so text leads. Spacing is a 4-point scale and the same number
appears above a title as to its left (`PAGE_X`, with `TITLE_OPTICAL_LIFT` putting
the cap height, not the line box, on that line). Motion is quick and springy:
hover surfaces fade over `HOVER_MS`, toggles and toasts use `SPRING_SNAPPY` and
`SPRING_GENTLE`, and everything respects Reduce Motion.

## Tokens (`ui/tokens.rs`)

| Group | Names | Use |
| --- | --- | --- |
| Surfaces | `SHELL`, `SURFACE`, `SURFACE_RAISED`, `SURFACE_OVERLAY`, `SURFACE_INPUT`, `SURFACE_CONTROL`, `SURFACE_ERROR`, `SURFACE_SUNKEN` | Window, content card, cards, menus and dialogs, inputs, control tracks, error banners, wells recessed into the card (board lanes). |
| States | `HOVER`, `HOVER_STRONG`, `ACTIVE`, `SELECTED`, `SELECTED_STRONG`, `FOCUS` | Hover on quiet rows and on controls, pressed, selected rows and segments, the focus ring. |
| Borders | `BORDER`, `BORDER_SUBTLE`, `BORDER_STRONG`, `ERROR_BORDER` | Around surfaces, inside them, on overlays, on invalid fields. |
| Text | `TEXT`, `TEXT_SECONDARY`, `TEXT_TERTIARY`, `TEXT_PLACEHOLDER`, `TEXT_ON_PRIMARY` | Body, descriptions, eyebrows and hints, placeholders, ink on white. |
| Actions | `PRIMARY`, `PRIMARY_HOVER`, `PRIMARY_ACTIVE`, `DESTRUCTIVE`, `DESTRUCTIVE_HOVER`, `DESTRUCTIVE_TEXT`, `ERROR` | The white button, the red button, red text. |
| Status | `STATUS_{NEUTRAL,GREEN,BLUE,AMBER,RED,PURPLE}` and `_SURFACE` pairs, `LABEL_COLORS` | Badge and pill tones; eight muted label hues. |
| Spacing | `SPACE_HALF`, `SPACE_1` … `SPACE_10`, `PAGE_X`, `PANEL_GAP`, `SIDEBAR_INSET`, `SECTION_GAP`, control and row sizes | Every gap, inset and control dimension. |
| Radius | `RADIUS_XS` … `RADIUS_XL`, `PANEL_RADIUS`, `CONTROL_RADIUS`, `FIELD_RADIUS` | Chips and hints, controls, cards, panels. |
| Type | `DISPLAY_SIZE`, `TITLE_SIZE`, `HEADING_SIZE`, `BODY_SIZE`, `LABEL_SIZE`, `CAPTION_SIZE`, `MICRO_SIZE`; `type_size()` | Page titles, dialog titles, section headings, body, labels, captions, hints. |
| Shadow | `shadow_overlay()`, `shadow_dialog()`, `shadow_toast()`, `focus_ring()` | Menus, dialogs and the palette, toasts, keyboard focus. Focus rings appear after keyboard navigation and hide on the next pointer press. |
| Motion | `HOVER_MS`, `PANEL_MS`, `MESSAGE_MS`, `SKELETON_MS`, `SPRING_SNAPPY`, `SPRING_GENTLE` | Fades, panel reveal, message arrival, skeleton pulse, springs. |

`cargo xtask check-ui` (`crates/ainc-xtask/src/checks/colors.rs`) fails the build when a color literal appears anywhere
else; `checks/ui_spacing.rs` fails on new raw spacing literals in the files listed in
`crates/ainc-mac/scripts/ui-spacing-baseline.json`.

## Components

Every interactive component takes the host view's `HoverFade` (so its hover
surface can fade between frames) and a `cx.listener`-style action; hosts implement
`HoverHost` and call `hover.animate(window)` once per render. `HoverFade::track`
is the one place a control gets its hover amount and listener.

Labelled add/create buttons place their `+` after the text ("New Agent +").
Other button icons lead the text; icon-only buttons stay centered.

Sheets, tabs and checkboxes are built for the 1.0 workstreams (onboarding,
providers) and are exercised only on the Components page today.

| Component | File | Use it for |
| --- | --- | --- |
| `Button` (primary, secondary, ghost, destructive; small, regular, large; `.icon_only()`, `.tint()`, `.leading()`) | `button.rs` | Every labeled action. One white primary per surface (the header's, never also the empty state's); secondary is outlined; ghost is transparent at rest and paints its hover as an overlay, so it sits on any surface; destructive is an outlined red control, never a solid slab; a disabled primary is an outline, not a gray block. |
| `Field` / `text_field` / `field_label` | `field.rs` | Labeled single-line inputs and `.multiline()` text areas, with hint, error and a quiet `FOCUS_FIELD` border while editing. Fields, selects and buttons share `CONTROL_HEIGHT`; forms are dialogs that end with a `DialogFooter`. |
| `Select` | `select.rs` | Choosing one option from a short list. The menu floats over the trigger like a macOS pop-up button, with the current option on the trigger's line, so the row never resizes and the menu never has to choose a side. `.below()` drops it under the trigger instead (dialogs, where a menu over the trigger would cover the fields above); `.quiet()` is a surfaceless trigger for property rows whose menu drops below, right edges aligned. Options can carry a colored `.glyph()` or an `.avatar()`, shown on the trigger and in the menu. |
| `MenuEntry` (`.glyph()` for a colored leading icon, `.leading()` for an avatar), `MenuButton`, `menu_label`, `menu_divider`, `floating` | `menu.rs` | Dropdown and context menu rows and the deferred, anchored placement they share with popovers. `menu_shell` and `popover_shell` in `overlay.rs` are their surfaces. A `MenuButton` is a toolbar button, such as a filter, whose menu drops below it, left edges aligned. |
| `banner`, `error_text` | `banner.rs` | A toned full-width notice inside a page; short red copy under a field or inside a dialog. |
| `toggle`, `checkbox` | `toggle.rs` | Boolean settings. Toggles for immediate effect, checkboxes inside forms. |
| `segmented`, `tabs`, `chip` | `segmented.rs` | One of a few options (segments hug their content), switching views inside a page (tabs underline their label), picking a value in a form (chips: the chosen one is filled with full-strength text, the rest are outlined). |
| `badge`, `status_pill`, `status_dot`, `count_badge`, `tag`, `Tone` | `badge.rs` | States and counts. Use `badge` in lists and headers, `status_pill` in tables and property rows, `tag` for labels and `chip` for form picks; never mix them on one surface. A `tag` is a label: a bordered pill with a dot in one of the `LABEL_COLORS`, never red, which stays for status. |
| `avatar`, `agent_avatar`, `AssigneeFace`, `assignee_avatar` | `avatar.rs` | Show an assignee with `assignee_avatar(&AssigneeFace, size)` everywhere (cards, rows, selects, the Agents page). People are round, with a photo or initials; agents are rounded squares with initials, so the two read apart wherever they appear together. |
| `card`, `panel`, `divider`, `ListRow`, `list_item`, `status_bar`, `property_row`, `PageFrame`, `PageHeader` | `layout.rs`, `display.rs` | Page frames, grouped content, interactive rows and static entries. A detail page makes its record the one H1 and puts the way back in `PageHeader::leading`; rows live in a `card` with hairlines between them. `PageFrame::fill` is a document page whose content fills the height under its header and scrolls its own parts (the Tickets board). `property_row` is a record's labeled value, the label kept beside a wrapping value's first line. |
| `LoadState`, `page_frame`, `skeleton_rows`, `LoadingFrame` | `loading.rs` | Every data page's loading frame: `page_frame(id, &state, rows, ui, body)` gives the `{id}.error` banner, the reconnecting hint, `{id}.loading` skeleton rows until the first load, then the page's body. Pages never hand-assemble these. |
| `table_container`, `table_header`, `table_cells`, `table_row`, `TableColumn` | `table.rs` | Columnar data with an eyebrow header and clickable rows. |
| `dialog_shell`, `DialogFooter`, `Verb`, `sheet_shell`, `popover_shell`, `menu_shell`, `OverlayHost<O>` | `overlay.rs` | Modal confirmations and forms with a Cancel / confirm footer whose confirm label and pending label ("Creating…", "Deleting…") follow a `Verb`, side panels, floating surfaces; one active overlay per window with focus return. The overlay vocabulary itself (`overlay::Overlay`) belongs to the shell; a page registers its own dialogs under its route through `page::PageOverlays`. |
| `Toasts` | `toast.rs` | Transient notices; the host anchors the stack on its content rail and schedules dismissal for transient ones (the shell keeps a sticky one while the session cannot be saved). |
| `EmptyState` | `empty.rs` | A page or section with nothing in it: icon, title, one line, one action. |
| `skeleton`, `skeleton_rows`, `LoadingFrame` | `loading.rs` | Placeholders while data loads; the Evee mark for longer waits. |
| `kbd`, `kbd_hint`, `eyebrow`, `heading`, `caption`, `icon` | `display.rs` | Shortcut hints, section labels, secondary copy, icons. |
| `settings_section`, `settings_row`, `settings_divider` | `settings.rs` | Grouped preference rows with a label, description and right-aligned control. |
| `render_palette`, `palette_frame`, `palette_header`, `PaletteGroup`, `PaletteEntry`, `fuzzy_match` | `palette.rs`, `fuzzy.rs` | The ⌘K palette: grouped, fuzzy-matched results with highlighted matches, keyboard navigation and shortcut hints; the frame is reused by any form that replaces the results. |

## Shell

The shell (`src/shell.rs` and `src/shell/`) composes the sidebar (workspace card,
⌘K Go to…, navigation, user row), the title bar with the current Page tab contour,
the content card with its status bar, the Go to… palette, the user menu popover and
toasts. Pages receive the content card and render inside `PageFrame::document` (header
plus scrolling content), `PageFrame::fill` (header plus content that fills the height: the
board, the Conversation view) or `PageFrame::canvas` (the Terminal).

## Copy

Every string a person reads in the app follows these rules. Constructs are the nouns in
[CONTEXT.md](../CONTEXT.md).

1. Title Case for buttons, menu items and dialog, page and section titles.
2. Sentence case for field labels, descriptions, hints, placeholders, empty-state bodies,
   toasts and errors.
3. Capitalize a construct (Ticket, Comment, Conversation, Automation, Agent, Occurrence,
   Connection) when it means the construct.
4. Full sentences end with a period; titles, labels and buttons do not.
5. Typographic `…` (U+2026) and `’`, always. An action gets `…` only when it opens further
   input.
6. Verbs: New creates a top-level record, Add attaches to a record, Delete removes
   permanently, Remove detaches, Save commits an edit, Cancel dismisses a dialog, Close
   dismisses a panel.
7. Destructive confirmation: title `Delete “{name}”?`, body `{What} will be permanently
   deleted.`, button `Delete`.
8. Shortcuts are glyph-only with no separators (`⌘K` `⌘[` `⌘,` `⎋`) and come from the one
   shortcuts table.
9. Errors read `{Thing} is unavailable. {Recovery}.` Raw detail sits behind a disclosure,
   never interpolated. Validation errors go to `Field::error`, load errors to `banner`.
10. Empty states read `No {things} yet.` plus one action, always through `EmptyState`.
11. British spelling: Cancelled.
12. People are round avatars (`avatar`); agents are rounded squares (`agent_avatar`). Everywhere.

The decisions the rules leave open, settled once:

- Placeholders name the value as a sentence-case noun phrase ("Ticket title", "Name",
  "Automation name", "Minutes", "Comment"); search fields read "Search {Things}"; the
  composer reads "Message Evee".
- An empty state reads `No {things} yet.` when nothing exists and `No matching {things}.`
  when a filter hides everything. The title keeps its period; it is a sentence.
- A detail page is a `PageHeader` whose `leading` is a small ghost back control (the parent
  page's name with `chevronLeft`), on Tickets, Automations and the Conversation view alike.
- Every page that polls the daemon through `Sync` has an icon-only secondary Refresh as the
  first header action and shows `banner` + "Reconnecting…" the same way; Temporal, which
  loads on demand, has the same icon-only Refresh. Nothing else offers a refresh.
- Loading lists show `SKELETON_ROWS` skeleton rows, never a page-specific count.
- A toast stays until dismissed; a transient one follows `Toasts::push` with
  `Toasts::dismiss_later`, which removes it after `TOAST_MS`.
- A disabled destructive control stays visible and the nearest section says why ("A Ticket
  with Work cannot be deleted.").

`cargo xtask check-copy` (`crates/ainc-xtask/src/checks/copy.rs`) enforces the mechanical
rules over the string literals passed to `Button::new`, `MenuEntry::new`, `PaletteEntry::new`,
`PageHeader::new`, `EmptyState::new`, `dialog_shell`, `DialogFooter::label` and
`TextInput::field`: no `...`, no straight apostrophe in prose, no "Canceled", none of
CONTEXT.md's avoid-words, Title Case for buttons, menu and palette entries and dialog titles,
and a period on every `EmptyState` title. The case heuristic is deliberately simple: every
word longer than three letters is capitalized unless it is in the small-word list (a, an, and,
as, at, by, for, from, in, of, on, or, the, to, with); a literal that is not a title (an
accessible label with a `·`, an interpolated `format!`) is skipped. Add a construct or a
brand name that the heuristic misjudges to the allow-list in that file.

## Adding a component

Put it in `src/ui/`, use tokens only, give it a `debug_selector` where a test will
need it, add it to the Components page and to the table above, and cover it in the
rendered-shell test.
