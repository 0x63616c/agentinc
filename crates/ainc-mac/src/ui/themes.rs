//! The built-in themes. Each is a complete `Theme`; `DARK` is the look the app
//! shipped with and the base every custom theme starts from. This file and
//! `tokens.rs` are the only places a color value is written.
use super::tokens::Theme;

/// A built-in theme as the theme picker lists it.
#[derive(Clone, Copy, Debug)]
pub struct Preset {
    /// The stable id saved in UI state.
    pub id: &'static str,
    pub name: &'static str,
    /// Whether the canvas is dark; the picker groups by this.
    pub dark: bool,
    pub theme: Theme,
}

/// The built-in themes in picker order: the dark ones, then the light ones.
pub const PRESETS: &[Preset] = &[
    Preset {
        id: "dark",
        name: "Dark",
        dark: true,
        theme: DARK,
    },
    Preset {
        id: "graphite",
        name: "Graphite",
        dark: true,
        theme: GRAPHITE,
    },
    Preset {
        id: "midnight",
        name: "Midnight",
        dark: true,
        theme: MIDNIGHT,
    },
    Preset {
        id: "paper",
        name: "Paper",
        dark: false,
        theme: PAPER,
    },
    Preset {
        id: "linen",
        name: "Linen",
        dark: false,
        theme: LINEN,
    },
    Preset {
        id: "mist",
        name: "Mist",
        dark: false,
        theme: MIST,
    },
];

/// The built-in theme with this id.
pub fn preset(id: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|preset| preset.id == id)
}

/// A near-black canvas, white primary actions, restrained grays and thin
/// borders instead of heavy fills. Every gray is a step of one neutral ramp.
pub const DARK: Theme = Theme {
    shell: 0x000000,
    surface: 0x0a0a0a,
    surface_raised: 0x101010,
    surface_overlay: 0x131313,
    surface_input: 0x0d0d0d,
    surface_control: 0x131313,
    surface_sunken: 0x060606,
    surface_error: 0x231414,
    skeleton: 0x131313,
    hover: 0x101010,
    hover_strong: 0x1d1d1d,
    active: 0x222222,
    selected: 0x181818,
    selected_strong: 0x2a2a2a,
    focus: 0xbdbdbd,
    focus_field: 0x707070,
    text_selection: 0x454545ff,
    border: 0x222222,
    border_subtle: 0x181818,
    border_strong: 0x303030,
    error_border: 0x7a3736,
    text: 0xf2f2f2,
    text_secondary: 0x9c9c9c,
    text_tertiary: 0x858585,
    text_placeholder: 0x5a5a5a,
    text_on_primary: 0x0a0a0a,
    primary: 0xffffff,
    primary_hover: 0xe9e9e9,
    primary_active: 0xd9d9d9,
    destructive_hover: 0x321a1a,
    destructive_text: 0xf08c8b,
    error: 0xf08c8b,
    status_neutral: 0xa3a3a3,
    status_neutral_surface: 0x181818,
    status_green: 0x8fd9a8,
    status_green_surface: 0x13231a,
    status_blue: 0x94bdf0,
    status_blue_surface: 0x142033,
    status_amber: 0xe8c37e,
    status_amber_surface: 0x2a2213,
    status_red: 0xf08c8b,
    status_red_surface: 0x2c1717,
    status_purple: 0xc1a6ec,
    status_purple_surface: 0x231a33,
    grip: 0x3a3a3a,
    scrim: 0x000000cc,
    shadow: 0x000000,
    label_colors: [
        0x94bdf0, 0x8fd9a8, 0xe8c37e, 0xc1a6ec, 0xeba2c8, 0x86d0cf, 0xeeaa7c, 0xa9b4c6,
    ],
    terminal_ansi: [
        0x171717, 0xb67171, 0x8eae9b, 0xcbb38a, 0x7aa7d8, 0xb69aca, 0x82b4bc, 0xc6c6c6, 0x555555,
        0xe6acac, 0xb1d4ba, 0xe2cea0, 0xa4c9f1, 0xd0b2e2, 0xa6d5dd, 0xededed,
    ],
};

/// A softer dark: charcoal surfaces lifted off a dark gray canvas.
pub const GRAPHITE: Theme = Theme {
    shell: 0x161618,
    surface: 0x1c1c1f,
    surface_raised: 0x232327,
    surface_overlay: 0x26262a,
    surface_input: 0x202024,
    surface_control: 0x26262a,
    surface_sunken: 0x19191c,
    surface_error: 0x2d1b1b,
    skeleton: 0x26262a,
    hover: 0x232327,
    hover_strong: 0x2e2e33,
    active: 0x35353b,
    selected: 0x2a2a2f,
    selected_strong: 0x3a3a40,
    focus: 0xc8c8cc,
    focus_field: 0x7c7c82,
    text_selection: 0x4a4a52ff,
    border: 0x2f2f35,
    border_subtle: 0x262629,
    border_strong: 0x3d3d44,
    error_border: 0x7f3b3a,
    text: 0xf0f0f2,
    text_secondary: 0xa3a3a8,
    text_tertiary: 0x8c8c92,
    text_placeholder: 0x63636a,
    text_on_primary: 0x1c1c1f,
    primary: 0xf5f5f7,
    primary_hover: 0xe4e4e8,
    primary_active: 0xd4d4d8,
    destructive_hover: 0x3a1f1f,
    destructive_text: 0xf29291,
    error: 0xf29291,
    status_neutral: 0xa8a8ad,
    status_neutral_surface: 0x2a2a2f,
    status_green: 0x93dcac,
    status_green_surface: 0x16281e,
    status_blue: 0x98c0f2,
    status_blue_surface: 0x182438,
    status_amber: 0xebc684,
    status_amber_surface: 0x2e2617,
    status_red: 0xf29291,
    status_red_surface: 0x301b1b,
    status_purple: 0xc4aaee,
    status_purple_surface: 0x271e38,
    grip: 0x45454b,
    scrim: 0x000000b3,
    shadow: 0x000000,
    label_colors: [
        0x98c0f2, 0x93dcac, 0xebc684, 0xc4aaee, 0xeda6cb, 0x8ad3d2, 0xf0ae82, 0xadb8ca,
    ],
    terminal_ansi: [
        0x222226, 0xb87474, 0x91b19e, 0xceb68d, 0x7daadb, 0xb99dcd, 0x85b7bf, 0xc9c9cd, 0x5c5c62,
        0xe8afaf, 0xb4d7bd, 0xe5d1a3, 0xa7ccf4, 0xd3b5e5, 0xa9d8e0, 0xf0f0f2,
    ],
};

/// A deep navy canvas with cool grays and slightly brighter status tones.
pub const MIDNIGHT: Theme = Theme {
    shell: 0x070b16,
    surface: 0x0d1322,
    surface_raised: 0x121a2d,
    surface_overlay: 0x151e33,
    surface_input: 0x101829,
    surface_control: 0x151e33,
    surface_sunken: 0x0a0f1c,
    surface_error: 0x2a1620,
    skeleton: 0x151e33,
    hover: 0x121a2d,
    hover_strong: 0x1c2740,
    active: 0x222f4a,
    selected: 0x182238,
    selected_strong: 0x2a3754,
    focus: 0xb9c6e6,
    focus_field: 0x6b7a9c,
    text_selection: 0x36466bff,
    border: 0x212c47,
    border_subtle: 0x182238,
    border_strong: 0x2f3d5e,
    error_border: 0x7d3a45,
    text: 0xeef2fb,
    text_secondary: 0x9aa6c2,
    text_tertiary: 0x8591ad,
    text_placeholder: 0x56627f,
    text_on_primary: 0x0d1322,
    primary: 0xe9eefc,
    primary_hover: 0xd7dff5,
    primary_active: 0xc6d0ec,
    destructive_hover: 0x3a1d28,
    destructive_text: 0xf39a9e,
    error: 0xf39a9e,
    status_neutral: 0xa4afc9,
    status_neutral_surface: 0x1c2740,
    status_green: 0x9adeb1,
    status_green_surface: 0x14291f,
    status_blue: 0x9cc3f5,
    status_blue_surface: 0x152647,
    status_amber: 0xecc98a,
    status_amber_surface: 0x2d2616,
    status_red: 0xf39a9e,
    status_red_surface: 0x311b25,
    status_purple: 0xc7adf0,
    status_purple_surface: 0x261f3f,
    grip: 0x3a4663,
    scrim: 0x02050ecc,
    shadow: 0x02050e,
    label_colors: [
        0x9cc3f5, 0x9adeb1, 0xecc98a, 0xc7adf0, 0xeca7cc, 0x8cd4d6, 0xefb088, 0xaab6d0,
    ],
    terminal_ansi: [
        0x141b2e, 0xb87580, 0x8fb49f, 0xcdb78e, 0x7fa9dc, 0xb89fd0, 0x86b8c2, 0xc9cfdd, 0x55617d,
        0xe8b0b6, 0xb3d8bf, 0xe4d2a4, 0xa7ccf3, 0xd3b7e5, 0xa8d8e0, 0xeef2fb,
    ],
};

/// A plain light theme: white cards on a light gray canvas, black primary
/// actions, status tones darkened for contrast.
pub const PAPER: Theme = Theme {
    shell: 0xe9e9e9,
    surface: 0xffffff,
    surface_raised: 0xf6f6f6,
    surface_overlay: 0xffffff,
    surface_input: 0xfbfbfb,
    surface_control: 0xefefef,
    surface_sunken: 0xf1f1f1,
    surface_error: 0xfdecec,
    skeleton: 0xebebeb,
    hover: 0xf2f2f2,
    hover_strong: 0xe6e6e6,
    active: 0xdcdcdc,
    selected: 0xebebeb,
    selected_strong: 0xdadada,
    focus: 0x303030,
    focus_field: 0x8c8c8c,
    text_selection: 0xb8d4f8ff,
    border: 0xdcdcdc,
    border_subtle: 0xececec,
    border_strong: 0xc9c9c9,
    error_border: 0xe2a6a3,
    text: 0x161616,
    text_secondary: 0x5c5c5c,
    text_tertiary: 0x666666,
    text_placeholder: 0x9b9b9b,
    text_on_primary: 0xffffff,
    primary: 0x141414,
    primary_hover: 0x2b2b2b,
    primary_active: 0x3d3d3d,
    destructive_hover: 0xfbe3e2,
    destructive_text: 0xb4261f,
    error: 0xb4261f,
    status_neutral: 0x5c5c5c,
    status_neutral_surface: 0xededed,
    status_green: 0x1d7a3e,
    status_green_surface: 0xe2f4e8,
    status_blue: 0x1f5fb3,
    status_blue_surface: 0xe3edfb,
    status_amber: 0x8a5a00,
    status_amber_surface: 0xfbf0d8,
    status_red: 0xb4261f,
    status_red_surface: 0xfbe3e2,
    status_purple: 0x6a3fb5,
    status_purple_surface: 0xeee6fa,
    grip: 0xb3b3b3,
    scrim: 0x00000059,
    shadow: 0x000000,
    label_colors: [
        0x1f5fb3, 0x1d7a3e, 0x8a5a00, 0x6a3fb5, 0xb03a7a, 0x1b7f80, 0xb55a1e, 0x5d6b85,
    ],
    terminal_ansi: [
        0x2c2c2c, 0xb3261f, 0x1d7a3e, 0x8a5a00, 0x1f5fb3, 0x6a3fb5, 0x1b7f80, 0x8a8a8a, 0x5c5c5c,
        0xd0443c, 0x2a9a52, 0xb07a10, 0x3a79d0, 0x8a5fd6, 0x2aa0a2, 0x161616,
    ],
};

/// A warm light theme: cream surfaces and brown-black ink.
pub const LINEN: Theme = Theme {
    shell: 0xebe6dd,
    surface: 0xfbf8f2,
    surface_raised: 0xf5f1e9,
    surface_overlay: 0xfcf9f3,
    surface_input: 0xf9f6ef,
    surface_control: 0xefeae1,
    surface_sunken: 0xf2ede4,
    surface_error: 0xf9e6e1,
    skeleton: 0xece7de,
    hover: 0xf1ece3,
    hover_strong: 0xe6e0d5,
    active: 0xdbd4c8,
    selected: 0xebe5da,
    selected_strong: 0xdcd5c8,
    focus: 0x3b3630,
    focus_field: 0x8f8779,
    text_selection: 0xe0cfb0ff,
    border: 0xddd6c9,
    border_subtle: 0xece6da,
    border_strong: 0xc9c1b2,
    error_border: 0xdca59c,
    text: 0x1f1b16,
    text_secondary: 0x625b50,
    text_tertiary: 0x6c6559,
    text_placeholder: 0x9c9486,
    text_on_primary: 0xfbf8f2,
    primary: 0x221d17,
    primary_hover: 0x37312a,
    primary_active: 0x48413a,
    destructive_hover: 0xf7ded8,
    destructive_text: 0xa9321f,
    error: 0xa9321f,
    status_neutral: 0x625b50,
    status_neutral_surface: 0xebe5da,
    status_green: 0x2a7a3a,
    status_green_surface: 0xe3f1e1,
    status_blue: 0x2b5f9e,
    status_blue_surface: 0xe2eaf5,
    status_amber: 0x8a5400,
    status_amber_surface: 0xf7ecd2,
    status_red: 0xa9321f,
    status_red_surface: 0xf7ded8,
    status_purple: 0x6b4397,
    status_purple_surface: 0xece4f4,
    grip: 0xb5ad9e,
    scrim: 0x1a140c59,
    shadow: 0x1a140c,
    label_colors: [
        0x2b5f9e, 0x2a7a3a, 0x8a5400, 0x6b4397, 0xa7407a, 0x24797a, 0xb05a26, 0x6a6b7c,
    ],
    terminal_ansi: [
        0x2e2a24, 0xa9321f, 0x2a7a3a, 0x8a5400, 0x2b5f9e, 0x6b4397, 0x24797a, 0x8c857a, 0x625b50,
        0xc84a37, 0x3a9a4c, 0xae7513, 0x4279c2, 0x8a62bd, 0x36999b, 0x1f1b16,
    ],
};

/// A cool light theme: blue-gray canvas and slate ink.
pub const MIST: Theme = Theme {
    shell: 0xe4e8ee,
    surface: 0xf9fafc,
    surface_raised: 0xf2f4f8,
    surface_overlay: 0xfbfcfd,
    surface_input: 0xf6f8fb,
    surface_control: 0xeaeef4,
    surface_sunken: 0xeef1f5,
    surface_error: 0xfae6e8,
    skeleton: 0xe8ecf2,
    hover: 0xeef1f6,
    hover_strong: 0xe1e6ee,
    active: 0xd5dce6,
    selected: 0xe6eaf1,
    selected_strong: 0xd6dce6,
    focus: 0x2c3542,
    focus_field: 0x8391a4,
    text_selection: 0xc3d6f2ff,
    border: 0xd6dde7,
    border_subtle: 0xe5eaf1,
    border_strong: 0xbfc9d6,
    error_border: 0xe0a4a8,
    text: 0x151a22,
    text_secondary: 0x586375,
    text_tertiary: 0x626d7f,
    text_placeholder: 0x98a3b3,
    text_on_primary: 0xf9fafc,
    primary: 0x1a2230,
    primary_hover: 0x2b3545,
    primary_active: 0x3c4758,
    destructive_hover: 0xfadfe2,
    destructive_text: 0xb22a35,
    error: 0xb22a35,
    status_neutral: 0x586375,
    status_neutral_surface: 0xe6eaf1,
    status_green: 0x1e7a48,
    status_green_surface: 0xe0f3e8,
    status_blue: 0x1f5fb3,
    status_blue_surface: 0xe0eafb,
    status_amber: 0x876000,
    status_amber_surface: 0xf8efd6,
    status_red: 0xb22a35,
    status_red_surface: 0xfadfe2,
    status_purple: 0x6241b4,
    status_purple_surface: 0xebe5fa,
    grip: 0xa9b4c3,
    scrim: 0x0a101a59,
    shadow: 0x0a101a,
    label_colors: [
        0x1f5fb3, 0x1e7a48, 0x876000, 0x6241b4, 0xb03a82, 0x1a7f86, 0xb35a2a, 0x5e6f8a,
    ],
    terminal_ansi: [
        0x2a3140, 0xb22a35, 0x1e7a48, 0x876000, 0x1f5fb3, 0x6241b4, 0x1a7f86, 0x8794a6, 0x586375,
        0xd3464f, 0x2d9b5c, 0xad7c0e, 0x3d7bd6, 0x8465d2, 0x2fa3a8, 0x151a22,
    ],
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_ids_are_unique_and_dark_is_the_default() {
        let mut ids: Vec<_> = PRESETS.iter().map(|p| p.id).collect();
        ids.dedup();
        assert_eq!(ids.len(), PRESETS.len());
        assert_eq!(Theme::default(), DARK);
        assert_eq!(preset("paper").map(|p| p.dark), Some(false));
        assert!(preset("neon").is_none());
    }

    #[test]
    fn every_preset_keeps_text_readable_on_its_surface() {
        fn luminance(color: u32) -> f32 {
            let channel = |shift: u32| {
                let c = ((color >> shift) & 0xff) as f32 / 255.;
                if c <= 0.03928 {
                    c / 12.92
                } else {
                    ((c + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
        }
        fn contrast(a: u32, b: u32) -> f32 {
            let (l1, l2) = (luminance(a), luminance(b));
            (l1.max(l2) + 0.05) / (l1.min(l2) + 0.05)
        }
        for preset in PRESETS {
            let t = &preset.theme;
            for surface in [
                t.surface,
                t.surface_raised,
                t.surface_overlay,
                t.surface_input,
            ] {
                assert!(contrast(t.text, surface) >= 7., "{} text", preset.id);
                assert!(
                    contrast(t.text_secondary, surface) >= 4.5,
                    "{} secondary",
                    preset.id
                );
                assert!(
                    contrast(t.text_tertiary, surface) >= 4.5,
                    "{} tertiary",
                    preset.id
                );
            }
            assert!(
                contrast(t.text_on_primary, t.primary) >= 7.,
                "{} primary",
                preset.id
            );
            assert_eq!(
                luminance(t.shell) < 0.3,
                preset.dark,
                "{} dark flag",
                preset.id
            );
        }
    }

    #[test]
    fn a_theme_round_trips_through_json_and_fills_missing_slots() {
        let json = serde_json::to_string(&PAPER).unwrap();
        assert!(json.contains("\"shell\":\"#e9e9e9\""));
        assert!(json.contains("\"scrim\":\"#00000059\""));
        assert!(json.contains("\"label_colors.7\":\"#5d6b85\""));
        assert_eq!(serde_json::from_str::<Theme>(&json).unwrap(), PAPER);
        let partial: Theme =
            serde_json::from_str(r##"{"shell":"#123456","scrim":"#ffffff"}"##).unwrap();
        assert_eq!(partial.shell, 0x123456);
        assert_eq!(partial.scrim, 0xffffffff);
        assert_eq!(partial.text, DARK.text);
        assert!(serde_json::from_str::<Theme>(r##"{"shelf":"#123456"}"##).is_err());
        assert!(serde_json::from_str::<Theme>(r##"{"shell":"#12345"}"##).is_err());
    }
}
