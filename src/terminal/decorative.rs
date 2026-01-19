//! Decorative character detection for terminal rendering.
//!
//! This module provides utilities for detecting special characters that need
//! special rendering treatment, such as Powerline symbols, box-drawing characters,
//! and other decorative glyphs.
//!
//! Decorative characters typically need to:
//! - Bypass contrast adjustment (use exact colors specified)
//! - Be rendered with special fonts or fallback chains
//! - Connect properly with adjacent decorative characters (box drawing)

/// Checks if a character is a decorative/special character that needs
/// special rendering treatment.
///
/// Decorative characters include:
/// - Box Drawing characters (U+2500-U+257F)
/// - Block Elements (U+2580-U+259F)
/// - Geometric Shapes (U+25A0-U+25FF)
/// - Powerline symbols (U+E0B0-U+E0BF)
/// - Powerline extra symbols (U+E0C0-U+E0D7)
///
/// These characters are used by terminal themes like oh-my-zsh, starship,
/// and powerlevel10k for creating visual separators and status indicators.
pub fn is_decorative_character(ch: char) -> bool {
    let code = ch as u32;
    matches!(
        code,
        // Box Drawing (─ │ ┌ ┐ └ ┘ ├ ┤ ┬ ┴ ┼ etc.)
        0x2500..=0x257F
        // Block Elements (▀ ▄ █ ▌ ▐ ░ ▒ ▓ etc.)
        | 0x2580..=0x259F
        // Geometric Shapes (■ □ ▲ △ ● ○ etc.)
        | 0x25A0..=0x25FF
        // Powerline symbols ( etc.)
        | 0xE0B0..=0xE0BF
        // Powerline extra symbols
        | 0xE0C0..=0xE0D7
        // Braille patterns (used for graphics)
        | 0x2800..=0x28FF
        // Dingbats (often used in prompts)
        | 0x2700..=0x27BF
    )
}

/// Checks if a character is a Powerline-specific symbol.
/// These are the most common decorative characters in terminal prompts.
pub fn is_powerline_symbol(ch: char) -> bool {
    let code = ch as u32;
    matches!(code, 0xE0B0..=0xE0D7)
}

/// Checks if a character is a box-drawing character.
/// Box drawing characters need to connect properly with adjacent characters.
pub fn is_box_drawing(ch: char) -> bool {
    let code = ch as u32;
    matches!(code, 0x2500..=0x257F)
}

/// Checks if a character is a block element.
/// Block elements are used for progress bars and visual indicators.
pub fn is_block_element(ch: char) -> bool {
    let code = ch as u32;
    matches!(code, 0x2580..=0x259F)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_powerline_triangle_is_decorative() {
        // U+E0B0 - Powerline right-pointing triangle
        assert!(is_decorative_character('\u{E0B0}'));
    }

    #[test]
    fn test_powerline_arrow_is_decorative() {
        // U+E0B1 - Powerline right-pointing arrow
        assert!(is_decorative_character('\u{E0B1}'));
    }

    #[test]
    fn test_box_drawing_horizontal_is_decorative() {
        // U+2500 - Box drawing light horizontal
        assert!(is_decorative_character('\u{2500}'));
    }

    #[test]
    fn test_box_drawing_vertical_is_decorative() {
        // U+2502 - Box drawing light vertical
        assert!(is_decorative_character('\u{2502}'));
    }

    #[test]
    fn test_box_drawing_corner_is_decorative() {
        // U+250C - Box drawing light down and right (corner)
        assert!(is_decorative_character('\u{250C}'));
    }

    #[test]
    fn test_block_full_is_decorative() {
        // U+2588 - Full block
        assert!(is_decorative_character('\u{2588}'));
    }

    #[test]
    fn test_block_half_is_decorative() {
        // U+2580 - Upper half block
        assert!(is_decorative_character('\u{2580}'));
        // U+2584 - Lower half block
        assert!(is_decorative_character('\u{2584}'));
    }

    #[test]
    fn test_geometric_square_is_decorative() {
        // U+25A0 - Black square
        assert!(is_decorative_character('\u{25A0}'));
    }

    #[test]
    fn test_regular_ascii_not_decorative() {
        assert!(!is_decorative_character('A'));
        assert!(!is_decorative_character('z'));
        assert!(!is_decorative_character('0'));
        assert!(!is_decorative_character(' '));
        assert!(!is_decorative_character('!'));
    }

    #[test]
    fn test_emoji_not_decorative() {
        // Common emoji are not in our decorative ranges
        assert!(!is_decorative_character('😀')); // U+1F600
        assert!(!is_decorative_character('🎉')); // U+1F389
    }

    #[test]
    fn test_cjk_not_decorative() {
        // CJK characters should not be considered decorative
        assert!(!is_decorative_character('中'));
        assert!(!is_decorative_character('日'));
    }

    #[test]
    fn test_is_powerline_symbol() {
        assert!(is_powerline_symbol('\u{E0B0}'));
        assert!(is_powerline_symbol('\u{E0B1}'));
        assert!(is_powerline_symbol('\u{E0B2}'));
        assert!(is_powerline_symbol('\u{E0B3}'));
        assert!(!is_powerline_symbol('A'));
        assert!(!is_powerline_symbol('\u{2500}')); // Box drawing is not powerline
    }

    #[test]
    fn test_is_box_drawing() {
        assert!(is_box_drawing('\u{2500}')); // Horizontal
        assert!(is_box_drawing('\u{2502}')); // Vertical
        assert!(is_box_drawing('\u{250C}')); // Top-left corner
        assert!(is_box_drawing('\u{2510}')); // Top-right corner
        assert!(is_box_drawing('\u{2514}')); // Bottom-left corner
        assert!(is_box_drawing('\u{2518}')); // Bottom-right corner
        assert!(!is_box_drawing('A'));
        assert!(!is_box_drawing('\u{E0B0}')); // Powerline is not box drawing
    }

    #[test]
    fn test_is_block_element() {
        assert!(is_block_element('\u{2580}')); // Upper half
        assert!(is_block_element('\u{2584}')); // Lower half
        assert!(is_block_element('\u{2588}')); // Full block
        assert!(is_block_element('\u{2591}')); // Light shade
        assert!(is_block_element('\u{2592}')); // Medium shade
        assert!(is_block_element('\u{2593}')); // Dark shade
        assert!(!is_block_element('A'));
    }

    #[test]
    fn test_braille_is_decorative() {
        // Braille patterns are often used for terminal graphics
        assert!(is_decorative_character('\u{2800}')); // Braille blank
        assert!(is_decorative_character('\u{28FF}')); // All dots
    }

    #[test]
    fn test_dingbats_is_decorative() {
        // Dingbats used in terminal prompts
        assert!(is_decorative_character('\u{2714}')); // Check mark
        assert!(is_decorative_character('\u{2718}')); // X mark
    }
}
