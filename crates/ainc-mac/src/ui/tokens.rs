//! The AgentInc design tokens: color, type, spacing, radius, shadow and motion.
//! Every authored value in the native app lives here; components and pages
//! compose these names and never restate a raw value.
use gpui::{BoxShadow, Pixels, Point, SpringConfig, px, rgba};
use std::sync::atomic::{AtomicU32, Ordering};

// ---------------------------------------------------------------------------
// Color. A near-black canvas, white primary actions, restrained grays and thin
// borders instead of heavy fills. Status colors stay muted so text leads.
// ---------------------------------------------------------------------------

/// The window canvas behind every panel.
pub const SHELL: u32 = 0x000000;
/// The content card, sidebar-level surfaces and inputs.
pub const SURFACE: u32 = 0x0a0a0a;
/// Cards, grouped sections and list containers that sit on `SURFACE`.
pub const SURFACE_RAISED: u32 = 0x111111;
/// Menus, dialogs, popovers and the command palette.
pub const SURFACE_OVERLAY: u32 = 0x161616;
/// Text inputs, search fields and the composer.
pub const SURFACE_INPUT: u32 = 0x0d0d0d;
/// Segmented-control tracks and secondary chips.
pub const SURFACE_CONTROL: u32 = 0x171717;
/// Error banners and failed rows.
pub const SURFACE_ERROR: u32 = 0x231414;

/// Quiet hover on rows and ghost buttons.
pub const HOVER: u32 = 0x161616;
/// Hover on controls that already have a surface.
pub const HOVER_STRONG: u32 = 0x212121;
/// Pressed state on gray controls.
pub const ACTIVE: u32 = 0x2a2a2a;
/// The selected row, tab or segment.
pub const SELECTED: u32 = 0x1e1e1e;
/// A stronger selected segment inside a control track.
pub const SELECTED_STRONG: u32 = 0x323232;

/// The default hairline between and around surfaces.
pub const BORDER: u32 = 0x262626;
/// Dividers inside a surface.
pub const BORDER_SUBTLE: u32 = 0x1e1e1e;
/// Borders on raised overlays and selected chips.
pub const BORDER_STRONG: u32 = 0x333333;
/// The keyboard focus ring.
pub const FOCUS: u32 = 0xbdbdbd;
/// The border of a text field or composer while it is being edited.
pub const FOCUS_FIELD: u32 = 0x707070;
/// Selected text inside inputs. Values that carry an alpha byte say so in a
/// comment; every other color token is opaque RGB.
pub const TEXT_SELECTION: u32 = 0x454545ff;

/// Primary text.
pub const TEXT: u32 = 0xf2f2f2;
/// Secondary text: descriptions, timestamps, captions.
pub const TEXT_SECONDARY: u32 = 0x9c9c9c;
/// Tertiary text: hints, disabled labels, section eyebrows.
pub const TEXT_TERTIARY: u32 = 0x6b6b6b;
/// Placeholder text inside inputs.
pub const TEXT_PLACEHOLDER: u32 = 0x5a5a5a;
/// Ink on white primary surfaces.
pub const TEXT_ON_PRIMARY: u32 = 0x0a0a0a;

/// The white primary action.
pub const PRIMARY: u32 = 0xffffff;
pub const PRIMARY_HOVER: u32 = 0xe9e9e9;
pub const PRIMARY_ACTIVE: u32 = 0xd9d9d9;

/// The destructive action is an outlined red control, never a solid slab;
/// this is its hover surface.
pub const DESTRUCTIVE_HOVER: u32 = 0x321a1a;
/// Destructive text on a neutral surface.
pub const DESTRUCTIVE_TEXT: u32 = 0xf08c8b;
/// Inline error text.
pub const ERROR: u32 = 0xf08c8b;
pub const ERROR_BORDER: u32 = 0x7a3736;

/// Status palette: each tone has a foreground and a matching quiet surface.
pub const STATUS_NEUTRAL: u32 = 0xa3a3a3;
pub const STATUS_NEUTRAL_SURFACE: u32 = 0x1c1c1c;
pub const STATUS_GREEN: u32 = 0x8fd9a8;
pub const STATUS_GREEN_SURFACE: u32 = 0x13231a;
pub const STATUS_BLUE: u32 = 0x94bdf0;
pub const STATUS_BLUE_SURFACE: u32 = 0x142033;
pub const STATUS_AMBER: u32 = 0xe8c37e;
pub const STATUS_AMBER_SURFACE: u32 = 0x2a2213;
pub const STATUS_RED: u32 = 0xf08c8b;
pub const STATUS_RED_SURFACE: u32 = 0x2c1717;
pub const STATUS_PURPLE: u32 = 0xc1a6ec;
pub const STATUS_PURPLE_SURFACE: u32 = 0x231a33;
/// The unread dot and other single accents.
pub const ACCENT: u32 = 0xffffff;

/// The resize grip tint before its opacity byte.
pub const GRIP_TINT: u32 = 0x3a3a3a00;
/// The dimming layer behind dialogs and the palette.
pub const SCRIM: u32 = 0x000000cc;
/// Shadow ink before its opacity byte.
pub const SHADOW_INK: u32 = 0x00000000;
/// Skeleton placeholder surface.
pub const SKELETON: u32 = 0x161616;

#[cfg(target_os = "macos")]
pub const TERMINAL_ANSI: [u32; 16] = [
    0x171717, 0xb67171, 0x8eae9b, 0xcbb38a, 0x7aa7d8, 0xb69aca, 0x82b4bc, 0xc6c6c6, 0x555555,
    0xe6acac, 0xb1d4ba, 0xe2cea0, 0xa4c9f1, 0xd0b2e2, 0xa6d5dd, 0xededed,
];

// ---------------------------------------------------------------------------
// Spacing. One 4-point scale; page and control insets are named so the same
// number appears above a title and to its left.
// ---------------------------------------------------------------------------

/// A half step: the gap between a title and its subtitle inside one row.
pub const SPACE_HALF: f32 = 2.;
pub const SPACE_1: f32 = 4.;
pub const SPACE_2: f32 = 8.;
pub const SPACE_3: f32 = 12.;
pub const SPACE_4: f32 = 16.;
pub const SPACE_5: f32 = 20.;
pub const SPACE_6: f32 = 24.;
pub const SPACE_8: f32 = 32.;
pub const SPACE_10: f32 = 40.;

/// The even inset around every document page: above the title and to its left.
pub const PAGE_X: f32 = SPACE_6;
/// The optical lift that puts a page title's cap height, not its line box, at
/// `PAGE_X` when the title's line box is `CONTROL_HEIGHT` tall.
pub const TITLE_OPTICAL_LIFT: f32 = 6.;
/// The gap between the sidebar and the content card.
pub const PANEL_GAP: f32 = SPACE_2;
/// The height of the title bar above the panels.
pub const TITLEBAR_HEIGHT: f32 = 48.;
/// The status bar inside the bottom of the content card.
pub const STATUS_BAR_HEIGHT: f32 = 28.;
/// The sidebar's own side inset.
pub const SIDEBAR_INSET: f32 = SPACE_3;
/// The gap between a navigation icon and its label, chosen so every sidebar
/// text edge (search, navigation, workspace, user) lands on one line.
pub const SIDEBAR_TEXT_GAP: f32 = 11.;

pub const HEADER_CONTROL: f32 = 30.;
/// The title bar's right inset, shared by the bell and the panel under it.
pub const HEADER_EDGE_INSET: f32 = 9.;
pub const HEADER_ICON_SIZE: f32 = 16.;
pub const ICON_SIZE: f32 = 16.;
pub const ICON_SIZE_SM: f32 = 14.;
pub const ICON_SIZE_LG: f32 = 18.;
pub const CONTROL_HEIGHT: f32 = 32.;
pub const CONTROL_HEIGHT_SM: f32 = 26.;
pub const CONTROL_HEIGHT_LG: f32 = 40.;
pub const FIELD_HEIGHT: f32 = CONTROL_HEIGHT;
pub const CONTROL_GAP: f32 = SPACE_2;
/// The gap between chips in a wrapping group.
pub const CHIP_GAP: f32 = 6.;
pub const CONTROL_INSET_X: f32 = SPACE_3;
pub const CONTROL_INSET_X_SM: f32 = 10.;
pub const FIELD_LABEL_GAP: f32 = SPACE_2;
pub const FIELD_INSET_X: f32 = SPACE_3;
pub const FORM_STACK_GAP: f32 = SPACE_4;
pub const SECTION_GAP: f32 = SPACE_6;
pub const SETTINGS_ROW_HEIGHT: f32 = 56.;
pub const SETTINGS_INSET: f32 = SPACE_4;
pub const LIST_ROW_HEIGHT: f32 = 44.;
pub const TABLE_ROW_HEIGHT: f32 = 48.;
pub const TABLE_HEADER_HEIGHT: f32 = 36.;
pub const DIALOG_WIDTH: f32 = 440.;
/// The widest an inline form grows; wider fields read as search bars.
pub const FORM_WIDTH: f32 = 560.;
pub const SHEET_WIDTH: f32 = 420.;
pub const MENU_WIDTH: f32 = 220.;
pub const MENU_INSET: f32 = SPACE_1;
pub const MENU_ITEM_HEIGHT: f32 = 32.;
pub const PALETTE_WIDTH: f32 = 560.;
pub const PALETTE_ROW_HEIGHT: f32 = 40.;
/// The palette's distance from the top of the window.
pub const PALETTE_TOP: f32 = 96.;
/// The palette's search row and footer heights.
pub const PALETTE_HEADER_HEIGHT: f32 = 56.;
pub const PALETTE_FOOTER_HEIGHT: f32 = 40.;
/// The tallest result list; smaller windows shrink it to clear the status bar.
pub const PALETTE_RESULTS_MAX_HEIGHT: f32 = 420.;
pub const POPOVER_WIDTH: f32 = 264.;
pub const TOAST_WIDTH: f32 = 360.;
pub const AVATAR_SIZE: f32 = 24.;
pub const AVATAR_SIZE_LG: f32 = 40.;
pub const TOGGLE_WIDTH: f32 = 36.;
pub const TOGGLE_HEIGHT: f32 = 20.;
pub const CHECKBOX_SIZE: f32 = 16.;
pub const DIALOG_PADDING: f32 = SPACE_6;
// The shortcut badge needs only 4 px after it to balance the search control.
pub const HEADER_SEARCH_RIGHT_INSET: f32 = 4.;
/// The workspace mark inside the sidebar card.
pub const WORKSPACE_MARK_SIZE: f32 = 32.;

// ---------------------------------------------------------------------------
// Radius.
// ---------------------------------------------------------------------------

pub const RADIUS_XS: f32 = 4.;
pub const RADIUS_SM: f32 = 6.;
pub const RADIUS_MD: f32 = 8.;
pub const RADIUS_LG: f32 = 12.;
pub const RADIUS_XL: f32 = 16.;
pub const PANEL_RADIUS: f32 = 14.;
pub const DIALOG_RADIUS: f32 = RADIUS_LG;
pub const MENU_RADIUS: f32 = RADIUS_MD;
pub const CONTROL_RADIUS: f32 = RADIUS_MD;
pub const FIELD_RADIUS: f32 = RADIUS_MD;

// ---------------------------------------------------------------------------
// Type. Sizes are authored at the original Control scale; `type_size` adds the
// two-point product offset and the user's chosen scale.
// ---------------------------------------------------------------------------

pub const DISPLAY_SIZE: f32 = 22.;
pub const TITLE_SIZE: f32 = 18.;
pub const HEADING_SIZE: f32 = 14.;
pub const BODY_SIZE: f32 = 13.;
pub const LABEL_SIZE: f32 = 12.;
pub const CAPTION_SIZE: f32 = 11.;
pub const MICRO_SIZE: f32 = 10.;
pub const DIALOG_TITLE_SIZE: f32 = TITLE_SIZE;
/// The line height for body copy.
pub const BODY_LINE_HEIGHT: f32 = 1.5;
/// The tighter line height for titles and display text.
pub const TITLE_LINE_HEIGHT: f32 = 1.2;

// The single app window shares a scale with its separately rendered page entities.
static TYPE_SCALE: AtomicU32 = AtomicU32::new(1f32.to_bits());

pub fn set_type_scale(scale: f32) {
    TYPE_SCALE.store(scale.to_bits(), Ordering::Relaxed);
}

/// Each type step is two points larger than the original Control scale.
pub fn type_size(size: f32) -> Pixels {
    px((size + 2.) * f32::from_bits(TYPE_SCALE.load(Ordering::Relaxed)))
}

// ---------------------------------------------------------------------------
// Shadow. Overlays float on a shadow rather than a heavy border.
// ---------------------------------------------------------------------------

fn shadow(alpha: u32, y: f32, blur: f32, spread: f32) -> BoxShadow {
    BoxShadow {
        color: rgba(SHADOW_INK | alpha).into(),
        offset: Point {
            x: px(0.),
            y: px(y),
        },
        blur_radius: px(blur),
        spread_radius: px(spread),
        inset: false,
    }
}

/// Menus, popovers and dropdowns.
pub fn shadow_overlay() -> Vec<BoxShadow> {
    vec![shadow(0x99, 8., 24., -4.), shadow(0x66, 1., 2., 0.)]
}

/// Dialogs, sheets and the command palette.
pub fn shadow_dialog() -> Vec<BoxShadow> {
    vec![shadow(0xb3, 24., 64., -12.), shadow(0x80, 2., 6., 0.)]
}

/// Toasts.
pub fn shadow_toast() -> Vec<BoxShadow> {
    vec![shadow(0x99, 6., 20., -6.)]
}

/// The keyboard focus ring, drawn outside a control without shifting its layout.
pub fn focus_ring() -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: rgba((FOCUS << 8) | 0xff).into(),
        offset: Point::default(),
        blur_radius: px(0.),
        spread_radius: px(2.),
        inset: false,
    }]
}

// ---------------------------------------------------------------------------
// Motion. Quick, springy, interruptible.
// ---------------------------------------------------------------------------

pub const DISABLED_OPACITY: f32 = 0.4;
pub const HOVER_MS: u64 = 120;
pub const GRIP_MS: u64 = 160;
pub const PANEL_MS: u64 = 180;
pub const MESSAGE_MS: u64 = 220;
pub const SKELETON_MS: u64 = 1400;
/// Controls that snap into place: toggles, segment thumbs, palette selection.
pub const SPRING_SNAPPY: SpringConfig = SpringConfig::new(420., 34., 1.);
/// Surfaces that settle: popovers, sheets, sidebar reveal.
pub const SPRING_GENTLE: SpringConfig = SpringConfig::new(260., 27., 1.);
