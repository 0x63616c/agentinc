//! Shared color, spacing, radius, size, type, and motion values.
use gpui::{Pixels, px};
use std::sync::atomic::{AtomicU32, Ordering};

// Native color roles. Keep all authored color values here.

pub const SHELL: u32 = 0x0c0c0c;
pub const SURFACE: u32 = 0x040404;
pub const SURFACE_RAISED: u32 = 0x171717;
pub const SURFACE_MENU: u32 = 0x1c1c1c;
pub const SURFACE_SEARCH: u32 = 0x0a0a0a;
pub const SURFACE_SEGMENT: u32 = 0x1b1b1b;
pub const SURFACE_COMPOSER: u32 = 0x181818;
pub const SURFACE_ERROR: u32 = 0x241818;
pub const HOVER: u32 = 0x191919;
pub const HOVER_ROW: u32 = 0x252525;
pub const HOVER_CONTROL: u32 = 0x292929;
pub const HOVER_SEND: u32 = 0x2a2a2a;
pub const SELECTED: u32 = 0x252525;
pub const SELECTED_SEGMENT: u32 = 0x333333;
pub const SELECTED_BORDER: u32 = 0x555555;
pub const FOCUS: u32 = 0xb5b5b5;
pub const FOCUS_SURFACE: u32 = 0x252525;
pub const TEXT_SELECTION: u32 = 0x4a4a4aff;
pub const BORDER: u32 = 0x272727;
pub const BORDER_SUBTLE: u32 = 0x1a1a1a;
pub const BORDER_OVERLAY: u32 = 0x353535;
pub const TEXT: u32 = 0xededed;
pub const TEXT_MUTED: u32 = 0xa0a0a0;
pub const TEXT_PLACEHOLDER: u32 = 0x888888;
pub const TEXT_ACCENT: u32 = 0xc6c6c6;
pub const STATUS_UNREAD: u32 = TEXT_ACCENT;
pub const TEXT_ON_PRIMARY: u32 = 0x141414;
pub const PRIMARY: u32 = 0xe8e8e8;
pub const ACCENT: u32 = 0x0a84ff;
#[cfg(target_os = "macos")]
pub const TERMINAL_ANSI: [u32; 16] = [
    0x171717, 0xb67171, 0x8eae9b, 0xcbb38a, 0x7aa7d8, 0xb69aca, 0x82b4bc, 0xc6c6c6, 0x555555,
    0xe6acac, 0xb1d4ba, 0xe2cea0, 0xa4c9f1, 0xd0b2e2, 0xa6d5dd, 0xededed,
];
pub const ERROR: u32 = 0xe6acac;
pub const ERROR_BORDER: u32 = 0xb67171;
pub const DESTRUCTIVE: u32 = 0x5b2b2b;
pub const DESTRUCTIVE_TEXT: u32 = 0xdaa7a7;
pub const GRIP_TINT: u32 = 0x33333300;
pub const SCRIM: u32 = 0x000000aa;

// Existing names remain aliases for the shared component vocabulary.
pub const MUTED: u32 = TEXT_MUTED;
pub const PRIMARY_INK: u32 = TEXT_ON_PRIMARY;
pub const DIALOG_SURFACE: u32 = SURFACE_RAISED;
pub const MENU_SURFACE: u32 = SURFACE_MENU;
pub const OVERLAY_BORDER: u32 = BORDER_OVERLAY;

// Native Control layout vocabulary. Keep optical exceptions at their measured values.
pub const PAGE_X: f32 = 26.;
pub const PANEL_GAP: f32 = 10.;
pub const HEADER_CONTROL: f32 = 30.;
pub const HEADER_ICON_SIZE: f32 = 16.;
pub const CONTROL_HEIGHT: f32 = 32.;
pub const FIELD_HEIGHT: f32 = 42.;
pub const CONTROL_GAP: f32 = 8.;
pub const CONTROL_INSET_X: f32 = 12.;
pub const COMPACT_CONTROL_INSET_X: f32 = 10.;
pub const COMPACT_CONTROL_INSET_Y: f32 = 6.;
pub const FIELD_LABEL_GAP: f32 = 8.;
pub const FIELD_INSET_X: f32 = 12.;
pub const FORM_STACK_GAP: f32 = 16.;
pub const SETTINGS_ROW_HEIGHT: f32 = 52.;
pub const SETTINGS_INSET: f32 = 18.;
pub const LIST_ROW_HEIGHT: f32 = 48.;
pub const DIALOG_WIDTH: f32 = 440.;
pub const MENU_WIDTH: f32 = 146.;
pub const MENU_INSET: f32 = 4.;
// The search icon needs 9 px before it to align its visible edge with header text.
pub const HEADER_SEARCH_LEFT_INSET: f32 = 9.;
// The shortcut badge needs only 4 px after it to balance the search control.
pub const HEADER_SEARCH_RIGHT_INSET: f32 = 4.;
// The workspace mark's left edge uses a half pixel to balance its icon.
pub const SIDEBAR_IDENTITY_LEFT_INSET: f32 = 6.5;
// The workspace label's right edge retains the measured 6 px optical inset.
pub const SIDEBAR_IDENTITY_RIGHT_INSET: f32 = 6.;
pub const PANEL_RADIUS: f32 = 14.;
pub const DIALOG_RADIUS: f32 = 12.;
pub const MENU_RADIUS: f32 = 6.;
pub const CONTROL_RADIUS: f32 = 6.;
pub const FIELD_RADIUS: f32 = 7.;
pub const DIALOG_PADDING: f32 = 24.;
pub const BODY_SIZE: f32 = 13.;
pub const LABEL_SIZE: f32 = 12.;
pub const CAPTION_SIZE: f32 = 11.;
pub const DIALOG_TITLE_SIZE: f32 = 18.;
// The single app window shares a scale with its separately rendered page entities.
static TYPE_SCALE: AtomicU32 = AtomicU32::new(1f32.to_bits());

pub fn set_type_scale(scale: f32) {
    TYPE_SCALE.store(scale.to_bits(), Ordering::Relaxed);
}

/// Each type step is two points larger than the original Control scale.
pub fn type_size(size: f32) -> Pixels {
    px((size + 2.) * f32::from_bits(TYPE_SCALE.load(Ordering::Relaxed)))
}
pub const DISABLED_OPACITY: f32 = 0.45;
pub const HOVER_MS: u64 = 140;
pub const GRIP_MS: u64 = 160;
pub const PANEL_MS: u64 = 180;
pub const MESSAGE_MS: u64 = 220;
