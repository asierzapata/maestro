use gpui::*;

/// Zed-inspired color palette for a minimalist, professional look
pub struct Theme {
    // Backgrounds
    pub bg_primary: Hsla,
    pub bg_surface: Hsla,
    pub bg_hover: Hsla,
    pub bg_selected: Hsla,

    // Text
    pub text_primary: Hsla,
    pub text_secondary: Hsla,
    pub text_tertiary: Hsla,

    // Accent
    pub accent: Hsla,
    pub accent_hover: Hsla,

    // Borders
    pub border_subtle: Hsla,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            // Backgrounds
            bg_primary: hsla(0.0, 0.0, 0.12, 1.0), // #1e1e1e
            bg_surface: hsla(0.0, 0.0, 0.16, 1.0), // #2a2a2a
            bg_hover: hsla(0.0, 0.0, 0.23, 1.0),   // #3a3a3a
            bg_selected: hsla(220.0, 0.13, 0.26, 1.0), // #3d4450

            // Text
            text_primary: hsla(0.0, 0.0, 0.90, 1.0), // #e4e4e7
            text_secondary: hsla(0.0, 0.0, 0.63, 1.0), // #a1a1aa
            text_tertiary: hsla(0.0, 0.0, 0.44, 1.0), // #71717a

            // Accent
            accent: hsla(211.0, 1.0, 0.45, 1.0),       // #0071e3
            accent_hover: hsla(211.0, 1.0, 0.52, 1.0), // Lighter blue

            // Borders
            border_subtle: hsla(0.0, 0.0, 1.0, 0.08), // rgba(255, 255, 255, 0.08)
        }
    }
}

impl Theme {
    pub fn new() -> Self {
        Self::default()
    }
}
