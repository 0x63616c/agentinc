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
| Surfaces | `SHELL`, `SURFACE`, `SURFACE_RAISED`, `SURFACE_OVERLAY`, `SURFACE_INPUT`, `SURFACE_CONTROL`, `SURFACE_ERROR` | Window, content card, cards, menus and dialogs, inputs, control tracks, error banners. |
| States | `HOVER`, `HOVER_STRONG`, `ACTIVE`, `SELECTED`, `SELECTED_STRONG`, `FOCUS` | Hover on quiet rows and on controls, pressed, selected rows and segments, the focus ring. |
| Borders | `BORDER`, `BORDER_SUBTLE`, `BORDER_STRONG`, `ERROR_BORDER` | Around surfaces, inside them, on overlays, on invalid fields. |
| Text | `TEXT`, `TEXT_SECONDARY`, `TEXT_TERTIARY`, `TEXT_PLACEHOLDER`, `TEXT_ON_PRIMARY` | Body, descriptions, eyebrows and hints, placeholders, ink on white. |
| Actions | `PRIMARY`, `PRIMARY_HOVER`, `PRIMARY_ACTIVE`, `DESTRUCTIVE`, `DESTRUCTIVE_HOVER`, `DESTRUCTIVE_TEXT`, `ERROR` | The white button, the red button, red text. |
| Status | `STATUS_{NEUTRAL,GREEN,BLUE,AMBER,RED,PURPLE}` and `_SURFACE` pairs, `ACCENT` | Badge and pill tones; the unread dot. |
| Spacing | `SPACE_HALF`, `SPACE_1` … `SPACE_10`, `PAGE_X`, `PANEL_GAP`, `SIDEBAR_INSET`, `SECTION_GAP`, control and row sizes | Every gap, inset and control dimension. |
| Radius | `RADIUS_XS` … `RADIUS_XL`, `PANEL_RADIUS`, `CONTROL_RADIUS`, `FIELD_RADIUS` | Chips and hints, controls, cards, panels. |
| Type | `DISPLAY_SIZE`, `TITLE_SIZE`, `HEADING_SIZE`, `BODY_SIZE`, `LABEL_SIZE`, `CAPTION_SIZE`, `MICRO_SIZE`; `type_size()` | Page titles, dialog titles, section headings, body, labels, captions, hints. |
| Shadow | `shadow_overlay()`, `shadow_dialog()`, `shadow_toast()`, `focus_ring()` | Menus, dialogs and the palette, toasts, keyboard focus. Focus rings appear after keyboard navigation and hide on the next pointer press. |
| Motion | `HOVER_MS`, `PANEL_MS`, `MESSAGE_MS`, `SKELETON_MS`, `SPRING_SNAPPY`, `SPRING_GENTLE` | Fades, panel reveal, message arrival, skeleton pulse, springs. |

`scripts/check-colors.py` fails the build when a color literal appears anywhere
else; `scripts/check-ui-spacing.py` warns on raw spacing literals in migrated
files.

## Components

Every interactive component takes the host view's `HoverFade` (so its hover
surface can fade between frames) and a `cx.listener`-style action; hosts implement
`HoverHost` and call `hover.animate(window)` once per render. `HoverFade::track`
is the one place a control gets its hover amount and listener.

Sheets, tabs, checkboxes, leading field icons and select option descriptions are
built for the 1.0 workstreams that follow (Kanban, onboarding, providers) and are
exercised only on the Components page today.

| Component | File | Use it for |
| --- | --- | --- |
| `Button` (primary, secondary, ghost, destructive; small, regular, large; `.icon_only()`, `.tint()`) | `button.rs` | Every labeled action. One white primary per surface (the header's, never also the empty state's); secondary is outlined; ghost is transparent at rest and paints its hover as an overlay, so it sits on any surface; destructive is an outlined red control, never a solid slab; a disabled primary is an outline, not a gray block. |
| `Field` / `text_field` | `field.rs` | Labeled single-line inputs and `.multiline()` text areas, with hint, error and a quiet `FOCUS_FIELD` border while editing. Fields, selects and buttons share `CONTROL_HEIGHT`; inline forms cap at `FORM_WIDTH` and end with a `dialog_footer`. |
| `Select` | `select.rs` | Choosing one option from a short list. The menu floats over the trigger like a macOS pop-up button, with the current option on the trigger's line, so the row never resizes and the menu never has to choose a side. |
| `MenuEntry`, `menu_label`, `menu_divider`, `floating` | `menu.rs` | Dropdown and context menu rows and the deferred, anchored placement they share with popovers. `menu_shell` and `popover_shell` in `overlay.rs` are their surfaces. |
| `banner`, `error_text` | `banner.rs` | A toned full-width notice inside a page; short red copy under a field or inside a dialog. |
| `toggle`, `checkbox` | `toggle.rs` | Boolean settings. Toggles for immediate effect, checkboxes inside forms. |
| `segmented`, `tabs`, `chip` | `segmented.rs` | One of a few options (segments hug their content), switching views inside a page (tabs underline their label), picking a value in a form (chips: the chosen one is filled with full-strength text, the rest are outlined). |
| `badge`, `status_pill`, `status_dot`, `count_badge`, `Tone` | `badge.rs` | States and counts. Badges in lists and headers, pills in tables. |
| `avatar` | `avatar.rs` | People and agents, with an initials fallback. |
| `card`, `panel`, `divider`, `ListRow`, `list_item`, `status_bar`, `Page`, `PageHeader` | `layout.rs`, `display.rs` | Page frames, grouped content, interactive rows and static entries. A detail page makes its record the one H1 and puts the way back in `PageHeader::leading`; rows live in a `card` with hairlines between them. |
| `table_container`, `table_header`, `table_cells`, `table_row`, `TableColumn` | `table.rs` | Columnar data with an eyebrow header and clickable rows. |
| `dialog_shell`, `dialog_footer`, `sheet_shell`, `popover_shell`, `menu_shell`, `OverlayHost<O>` | `overlay.rs` | Modal confirmations and forms with a Cancel / confirm footer, side panels, floating surfaces; one active overlay per window with focus return. The overlay vocabulary itself (`model::Overlay`) belongs to the shell. |
| `Toasts` | `toast.rs` | Transient notices; the host anchors the stack on its content rail and schedules dismissal for transient ones (the shell keeps a sticky one while the session cannot be saved). |
| `EmptyState` | `empty.rs` | A page or section with nothing in it: icon, title, one line, one action. |
| `skeleton`, `skeleton_rows`, `LoadingFrame` | `loading.rs` | Placeholders while data loads; the Evee mark for longer waits. |
| `kbd`, `kbd_hint`, `eyebrow`, `heading`, `caption`, `icon` | `display.rs` | Shortcut hints, section labels, secondary copy, icons. |
| `settings_section`, `settings_row`, `settings_divider` | `settings.rs` | Grouped preference rows with a label, description and right-aligned control. |
| `render_palette`, `palette_frame`, `palette_header`, `PaletteGroup`, `PaletteEntry`, `fuzzy_match` | `palette.rs`, `fuzzy.rs` | The ⌘K palette: grouped, fuzzy-matched results with highlighted matches, keyboard navigation and shortcut hints; the frame is reused by any form that replaces the results. |

## Shell

The shell (`src/shell.rs` and `src/shell/`) composes the sidebar (workspace card,
⌘K search, navigation ending in Settings, user row), the title bar with the current-space
tab contour, the content card with its status bar, the command palette, the user
menu popover, the notification panel and toasts. Pages receive the content card and
render inside `Page::document` (header plus scrolling content) or `Page::canvas`
(chat, terminal).

## Adding a component

Put it in `src/ui/`, use tokens only, give it a `debug_selector` where a test will
need it, add it to the Components page and to the table above, and cover it in the
rendered-shell test.
