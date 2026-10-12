//! The AgentInc design tokens: color, type, spacing, radius, shadow and motion.
//! Every authored value in the native app lives here or, for the color values
//! of each theme, in `ui/themes.rs`; components and pages compose these names
//! and never restate a raw value.
use gpui::{BoxShadow, Pixels, Point, SpringConfig, px, rgba};
use std::sync::atomic::{AtomicU32, Ordering};

// ---------------------------------------------------------------------------
// Color. Every color is a slot on the current `Theme`; the values themselves
// are authored per preset in `ui/themes.rs`. Components read `theme().slot`
// at render time, so switching the theme re-tints the whole app on the next
// frame. The default look is a near-black canvas, white primary actions,
// restrained grays and thin borders instead of heavy fills.
// ---------------------------------------------------------------------------

/// Which hex form a slot is written in: `#rrggbb`, or `#rrggbbaa` for the few
/// that carry their own opacity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorForm {
    Rgb,
    Rgba,
}

/// One editable color on a `Theme`: its JSON key, label and group for the
/// theme editor, and how to read and write it.
#[derive(Clone, Copy)]
pub struct ThemeSlot {
    pub key: &'static str,
    pub label: &'static str,
    pub group: &'static str,
    pub form: ColorForm,
    pub get: fn(&Theme) -> u32,
    pub set: fn(&mut Theme, u32),
}

macro_rules! theme_slots {
    (
        $( $group:literal { $( $(#[doc = $doc:literal])* $name:ident: $form:ident = $label:literal ),* $(,)? } )*
    ) => {
        /// The color slots every component paints with. Opaque slots hold
        /// `0xrrggbb`; `Rgba` slots hold `0xrrggbbaa`. Every preset fills all
        /// of them, and a saved custom theme falls back to the default for any
        /// slot it leaves out.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub struct Theme {
            $( $(
                $(#[doc = $doc])*
                pub $name: u32,
            )* )*
            /// Label dots: eight muted hues at the status palette's lightness,
            /// so labels stay apart from each other without shouting. Red is
            /// left to status.
            pub label_colors: [u32; 8],
            /// The terminal's sixteen ANSI colors.
            pub terminal_ansi: [u32; 16],
        }
        /// Every scalar slot, in editor order; `Theme::slots()` adds the lists.
        const SCALAR_SLOTS: &[ThemeSlot] = &[
            $( $(
                ThemeSlot {
                    key: stringify!($name),
                    label: $label,
                    group: $group,
                    form: theme_slots!(@form $form),
                    get: |theme| theme.$name,
                    set: |theme, value| theme.$name = value,
                },
            )* )*
        ];
    };
    (@form rgb) => { ColorForm::Rgb };
    (@form rgba) => { ColorForm::Rgba };
}

theme_slots! {
    "Surfaces" {
        /// The window canvas behind every panel.
        shell: rgb = "Shell",
        /// The content card, sidebar-level surfaces and inputs.
        surface: rgb = "Surface",
        /// Cards, grouped sections and list containers that sit on `surface`.
        surface_raised: rgb = "Raised surface",
        /// Menus, dialogs, popovers and the command palette.
        surface_overlay: rgb = "Overlay surface",
        /// Text inputs, search fields and the composer.
        surface_input: rgb = "Input surface",
        /// Segmented-control tracks and secondary chips.
        surface_control: rgb = "Control surface",
        /// Wells recessed into the content card, such as board lanes, so the
        /// cards on them read as raised.
        surface_sunken: rgb = "Sunken surface",
        /// Error banners and failed rows.
        surface_error: rgb = "Error surface",
        /// Skeleton placeholder surface.
        skeleton: rgb = "Skeleton",
    }
    "States" {
        /// Quiet hover on rows and ghost buttons.
        hover: rgb = "Hover",
        /// Hover on controls that already have a surface.
        hover_strong: rgb = "Strong hover",
        /// Pressed state on gray controls.
        active: rgb = "Pressed",
        /// The selected row, tab or segment.
        selected: rgb = "Selected",
        /// A stronger selected segment inside a control track.
        selected_strong: rgb = "Strong selected",
        /// The keyboard focus ring.
        focus: rgb = "Focus ring",
        /// The border of a text field or composer while it is being edited.
        focus_field: rgb = "Editing field border",
        /// Selected text inside inputs.
        text_selection: rgba = "Text selection",
    }
    "Borders" {
        /// The default hairline between and around surfaces.
        border: rgb = "Border",
        /// Dividers inside a surface.
        border_subtle: rgb = "Subtle border",
        /// Borders on raised overlays and selected chips.
        border_strong: rgb = "Strong border",
        /// The border of an invalid field.
        error_border: rgb = "Error border",
    }
    "Text" {
        /// Primary text.
        text: rgb = "Text",
        /// Secondary text: descriptions, timestamps, captions.
        text_secondary: rgb = "Secondary text",
        /// Tertiary text: hints, disabled labels, section eyebrows. Keep it AA
        /// (4.5:1 or better) on every surface up to the overlay.
        text_tertiary: rgb = "Tertiary text",
        /// Placeholder text inside inputs.
        text_placeholder: rgb = "Placeholder text",
        /// Ink on primary surfaces.
        text_on_primary: rgb = "Text on primary",
    }
    "Actions" {
        /// The primary action.
        primary: rgb = "Primary",
        primary_hover: rgb = "Primary hover",
        primary_active: rgb = "Primary pressed",
        /// The destructive action is an outlined red control, never a solid
        /// slab; this is its hover surface.
        destructive_hover: rgb = "Destructive hover",
        /// Destructive text on a neutral surface.
        destructive_text: rgb = "Destructive text",
        /// Inline error text.
        error: rgb = "Error text",
    }
    "Status" {
        /// Status palette: each tone has a foreground and a matching quiet surface.
        status_neutral: rgb = "Neutral",
        status_neutral_surface: rgb = "Neutral surface",
        status_green: rgb = "Green",
        status_green_surface: rgb = "Green surface",
        status_blue: rgb = "Blue",
        status_blue_surface: rgb = "Blue surface",
        status_amber: rgb = "Amber",
        status_amber_surface: rgb = "Amber surface",
        status_red: rgb = "Red",
        status_red_surface: rgb = "Red surface",
        status_purple: rgb = "Purple",
        status_purple_surface: rgb = "Purple surface",
    }
    "Effects" {
        /// The resize grip's tint; the pane adds its own opacity.
        grip: rgb = "Resize grip",
        /// The dimming layer behind dialogs and the palette.
        scrim: rgba = "Scrim",
        /// Shadow ink; each shadow adds its own opacity.
        shadow: rgb = "Shadow",
    }
}

impl Theme {
    /// Every editable slot in editor order: the scalars, then the eight label
    /// hues and the sixteen terminal colors.
    pub fn slots() -> Vec<ThemeSlot> {
        let mut slots = SCALAR_SLOTS.to_vec();
        slots.extend(LABEL_SLOTS);
        slots.extend(TERMINAL_SLOTS);
        slots
    }
    /// The slot with this JSON key (`label_colors.3` for a list entry).
    pub fn slot(key: &str) -> Option<ThemeSlot> {
        Self::slots().into_iter().find(|slot| slot.key == key)
    }
}

macro_rules! list_slots {
    ($const:ident, $field:ident, $group:literal, $label:literal, $( $index:literal ),*) => {
        const $const: [ThemeSlot; list_slots!(@count $($index)*)] = [
            $( ThemeSlot {
                key: concat!(stringify!($field), ".", $index),
                label: concat!($label, " ", $index),
                group: $group,
                form: ColorForm::Rgb,
                get: |theme| theme.$field[$index],
                set: |theme, value| theme.$field[$index] = value,
            }, )*
        ];
    };
    (@count) => { 0 };
    (@count $head:literal $($tail:literal)*) => { 1 + list_slots!(@count $($tail)*) };
}
list_slots!(
    LABEL_SLOTS,
    label_colors,
    "Labels",
    "Label",
    0,
    1,
    2,
    3,
    4,
    5,
    6,
    7
);
list_slots!(
    TERMINAL_SLOTS,
    terminal_ansi,
    "Terminal",
    "ANSI",
    0,
    1,
    2,
    3,
    4,
    5,
    6,
    7,
    8,
    9,
    10,
    11,
    12,
    13,
    14,
    15
);

/// Parses `#rrggbb` / `rrggbb`, or `#rrggbbaa` for an `Rgba` slot. An `Rgba`
/// slot accepts six digits and fills in full opacity; an `Rgb` slot refuses
/// eight.
pub fn parse_hex(text: &str, form: ColorForm) -> Option<u32> {
    let digits = text.trim().trim_start_matches('#');
    if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let value = u32::from_str_radix(digits, 16).ok()?;
    match (digits.len(), form) {
        (6, ColorForm::Rgb) => Some(value),
        (6, ColorForm::Rgba) => Some((value << 8) | 0xff),
        (8, ColorForm::Rgba) => Some(value),
        _ => None,
    }
}

/// Formats a slot's value the way `parse_hex` reads it.
pub fn format_hex(value: u32, form: ColorForm) -> String {
    match form {
        ColorForm::Rgb => format!("#{value:06x}"),
        ColorForm::Rgba => format!("#{value:08x}"),
    }
}

/// A theme serializes as one flat JSON object of slot keys to hex strings, so
/// a saved file is easy to edit by hand. Reading starts from the default theme
/// and overrides the slots present; an unknown key or a malformed color is an
/// error rather than a silent miss.
impl serde::Serialize for Theme {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let slots = Self::slots();
        let mut map = s.serialize_map(Some(slots.len()))?;
        for slot in slots {
            map.serialize_entry(slot.key, &format_hex((slot.get)(self), slot.form))?;
        }
        map.end()
    }
}
impl<'de> serde::Deserialize<'de> for Theme {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let entries: Vec<(String, String)> = serde::Deserialize::deserialize(d)
            .map(|map: std::collections::BTreeMap<String, String>| map.into_iter().collect())?;
        let mut theme = Self::default();
        for (key, text) in entries {
            let slot = Self::slot(&key)
                .ok_or_else(|| D::Error::custom(format!("unknown color slot: {key}")))?;
            let value = parse_hex(&text, slot.form)
                .ok_or_else(|| D::Error::custom(format!("{key}: not a hex color: {text}")))?;
            (slot.set)(&mut theme, value);
        }
        Ok(theme)
    }
}

impl Default for Theme {
    fn default() -> Self {
        super::themes::DARK
    }
}

// The single app window shares a theme with its separately rendered page entities.
static THEME: std::sync::RwLock<Theme> = std::sync::RwLock::new(super::themes::DARK);

/// The theme every component paints with this frame.
pub fn theme() -> Theme {
    *THEME
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Switches the whole app to `theme`; callers redraw afterwards.
pub fn set_theme(theme: Theme) {
    *THEME
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = theme;
}

/// The applied theme is process-wide; tests that change it take turns.
#[cfg(test)]
pub fn theme_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

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
/// Side inset of the status bar; the version sits this far from the card edge.
pub const STATUS_BAR_X: f32 = SPACE_3;
/// The sidebar's own side inset.
pub const SIDEBAR_INSET: f32 = SPACE_3;
/// The gap between a navigation icon and its label, chosen so every sidebar
/// text edge (search, navigation, workspace, user) lands on one line.
pub const SIDEBAR_TEXT_GAP: f32 = 11.;
/// Match navigation row padding at the avatar and the chevron's visible edge.
pub const SIDEBAR_PROFILE_INSET: f32 = SPACE_2;

pub const HEADER_CONTROL: f32 = 30.;
/// The title bar's right inset, shared by the title bar controls.
pub const HEADER_EDGE_INSET: f32 = 9.;
pub const HEADER_ICON_SIZE: f32 = 16.;
pub const ICON_SIZE: f32 = 16.;
pub const ICON_SIZE_SM: f32 = 14.;
/// `chevronUpDown.svg` has a 24-point viewBox and rounded 1.5-point strokes
/// ending at x=7 and x=17. Compensate for its transparent sides at small size.
pub const CHEVRON_UP_DOWN_GLYPH_INSET: f32 = (7. - 1.5 / 2.) * ICON_SIZE_SM / 24.;
/// Glyphs inside pills and card metadata.
pub const ICON_SIZE_XS: f32 = 12.;
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
pub const SHEET_WIDTH: f32 = 420.;
/// The search field at the start of a page toolbar.
pub const TOOLBAR_SEARCH_WIDTH: f32 = 260.;
/// The narrowest a board lane gets before the board scrolls sideways.
pub const BOARD_LANE_MIN_WIDTH: f32 = 168.;
/// The inset inside a board lane, around its cards.
pub const BOARD_LANE_INSET: f32 = SPACE_2;
/// A board lane's header row.
pub const LANE_HEADER_HEIGHT: f32 = 38.;
/// An empty lane's drop area.
pub const EMPTY_LANE_HEIGHT: f32 = 64.;
/// Badges, status pills and the square priority chip beside them.
pub const PILL_HEIGHT: f32 = 22.;
/// The gap between a pill's dot and its label.
pub const PILL_GAP: f32 = 6.;
/// The dot in a status pill, a tag or beside "Working".
pub const STATUS_DOT_SIZE: f32 = 6.;
/// The unread count bubble.
pub const COUNT_BADGE_SIZE: f32 = 18.;
/// Keyboard hints (`⌘K`) and the count bubble share one side inset.
pub const KBD_HEIGHT: f32 = 20.;
pub const KBD_INSET_X: f32 = 5.;
/// The gap between a keyboard hint and its action.
pub const KBD_HINT_GAP: f32 = 6.;
/// The slot a palette row keeps for its Return hint.
pub const PALETTE_ENTER_SLOT: f32 = 24.;
/// A sidebar item's glyph.
pub const NAV_ICON_SIZE: f32 = 17.;
/// The empty state's icon tile and its text's widest line.
pub const EMPTY_ICON_TILE: f32 = 40.;
pub const EMPTY_TEXT_MAX_WIDTH: f32 = 480.;
/// A multiline field's minimum height.
pub const TEXTAREA_MIN_HEIGHT: f32 = 96.;
/// A form select's width, and a short numeric field's.
pub const SELECT_WIDTH: f32 = 240.;
/// The color swatch beside each field of the theme editor.
pub const THEME_SWATCH_SIZE: f32 = 18.;
/// The hex field of the theme editor.
pub const THEME_HEX_WIDTH: f32 = 112.;
pub const SHORT_FIELD_WIDTH: f32 = 180.;
/// The Assistant's chat column, a reply's measure and a sent message's.
pub const READING_WIDTH: f32 = 900.;
pub const CHAT_REPLY_WIDTH: f32 = 680.;
pub const CHAT_MESSAGE_WIDTH: f32 = 620.;
/// Fixed columns in the Temporal work table.
pub const WORK_STATUS_WIDTH: f32 = 150.;
pub const WORK_STARTED_WIDTH: f32 = 170.;
pub const WORK_DURATION_WIDTH: f32 = 90.;
/// The current-space tab in the header: its face, the slot it sits in and the
/// contour painted around it. The face is `TAB_FACE_HEIGHT` tall inside a
/// contour `TAB_HEIGHT` tall whose last pixels overlap the panel border.
pub const TAB_WIDTH: f32 = 176.;
pub const TAB_HEIGHT: f32 = 40.;
pub const TAB_FACE_HEIGHT: f32 = 38.;
pub const TAB_RADIUS: f32 = 10.;
pub const TAB_SLOT_WIDTH: f32 = TAB_WIDTH + 2. * TAB_CONTOUR_SHOULDER;
pub const TAB_SLOT_HEIGHT: f32 = 42.;
/// How far the slot sinks below the header so the tab meets the panel.
pub const TAB_SLOT_SINK: f32 = 2.;
/// The inverse shoulder where the tab meets the panel; the contour extends one
/// shoulder past the face on each side.
pub const TAB_CONTOUR_SHOULDER: f32 = 12.;
pub const TAB_CONTOUR_WIDTH: f32 = TAB_WIDTH + 2. * TAB_CONTOUR_SHOULDER;
/// The tab's label sits this far above the face's centre line, an optical
/// lift that offsets the contour's 2px overlap, plus its side inset and gap.
pub const TAB_LABEL_LIFT: f32 = 2.;
pub const TAB_LABEL_INSET: f32 = 14.;
pub const TAB_LABEL_GAP: f32 = 7.;
pub const TAB_ICON_SIZE: f32 = 14.;
pub const HEADER_CONTROLS_INSET: f32 = 9.;
/// How much wider than the sidebar the header's left controls run.
pub const HEADER_LEFT_EXTRA: f32 = 60.;
pub const HEADER_TRAFFIC_WIDTH: f32 = 80.;
pub const TAB_CLOSE_SIZE: f32 = 24.;
/// The close control's inset from the tab's edge, so its glyph sits as far
/// in as the label's icon does on the left.
pub const TAB_CLOSE_INSET: f32 = TAB_LABEL_INSET - (TAB_CLOSE_SIZE - TAB_ICON_SIZE) / 2.;
/// How far a tab fades into the header at either end of a scrolled strip.
pub const TAB_FADE_WIDTH: f32 = TAB_CONTOUR_SHOULDER * 2.;
/// How long after the last trackpad scroll event the tab strip snaps to whole tabs.
pub const TAB_SETTLE_MS: u64 = 120;
/// The segmented track's inset around its segments and the gap between them.
pub const SEGMENT_TRACK_INSET: f32 = 3.;
pub const SEGMENT_GAP: f32 = 2.;
/// The bar under the selected tab.
pub const TAB_UNDERLINE: f32 = 2.;
/// The empty side of a small icon-only button around its glyph; a trailing one
/// bleeds by it so its glyph, not its hit area, ends on the column's edge.
pub const TRAILING_ICON_BLEED: f32 = (CONTROL_HEIGHT_SM - ICON_SIZE_SM) / 2.;
/// A card's top inset less the space above its first line's cap height, so the
/// text sits as far from the top as from the side.
pub const CARD_OPTICAL_LIFT: f32 = 4.;
/// The remove control inside a pill.
pub const PILL_REMOVE_SIZE: f32 = 18.;
/// Fixed columns in the Tickets list.
pub const LIST_KEY_WIDTH: f32 = 44.;
pub const LIST_ASSIGNEE_WIDTH: f32 = 168.;
pub const LIST_UPDATED_WIDTH: f32 = 72.;
/// Below this window width a record's properties stack above its content.
pub const PROPERTIES_BESIDE_MIN_WIDTH: f32 = 1100.;
/// The drop indicator between board cards.
pub const DROP_INDICATOR_HEIGHT: f32 = 2.;
/// The properties column beside a record's main content.
pub const PROPERTIES_WIDTH: f32 = 296.;
/// The label column inside a property row.
pub const PROPERTY_LABEL_WIDTH: f32 = 88.;
/// The height of one property row.
pub const PROPERTY_ROW_HEIGHT: f32 = 36.;
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
/// Avatars inside dense rows and board cards.
pub const AVATAR_SIZE_SM: f32 = 20.;
pub const AVATAR_SIZE_LG: f32 = 40.;
pub const TOGGLE_WIDTH: f32 = 36.;
pub const TOGGLE_HEIGHT: f32 = 20.;
pub const CHECKBOX_SIZE: f32 = 16.;
pub const DIALOG_PADDING: f32 = SPACE_6;
// The shortcut badge needs only 4 px after it to balance the search control.
pub const HEADER_SEARCH_RIGHT_INSET: f32 = 4.;

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
        color: rgba((theme().shadow << 8) | alpha).into(),
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
        color: rgba((theme().focus << 8) | 0xff).into(),
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
/// How long a dropped card's edge takes to settle back to rest.
pub const SETTLE_MS: u64 = 600;
/// How long a transient toast stays before `Toasts::dismiss_later` removes it.
pub const TOAST_MS: u64 = 5000;
/// How far a dragged card's copy floats from where it was picked up.
pub const DRAG_LIFT: f32 = SPACE_3;
/// The card left behind while its copy follows the pointer.
pub const GHOST_OPACITY: f32 = 0.35;
/// Controls that snap into place: toggles, segment thumbs, palette selection.
pub const SPRING_SNAPPY: SpringConfig = SpringConfig::new(420., 34., 1.);
/// Surfaces that settle: popovers, sheets, sidebar reveal.
pub const SPRING_GENTLE: SpringConfig = SpringConfig::new(260., 27., 1.);
