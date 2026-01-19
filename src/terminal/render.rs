//! Rendering utilities for efficient terminal display.
//!
//! This module provides batching and rendering primitives to reduce draw calls
//! when rendering terminal content. Instead of rendering each cell individually,
//! cells with identical styling are grouped into batched text runs.

use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::Term;
use alacritty_terminal::vte::ansi::NamedColor;

use crate::terminal::decorative::is_decorative_character;
use crate::terminal::session::EventProxy;

/// RGBA color representation
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Rgba { r, g, b, a }
    }

    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Rgba { r, g, b, a: 255 }
    }

    /// Convert to GPUI-compatible u32 format (0xRRGGBB)
    pub fn to_u32(&self) -> u32 {
        ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }
}

/// Style attributes for a cell or text run
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellStyle {
    pub fg: Rgba,
    pub bg: Rgba,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    /// Whether this is a decorative character (Powerline, box drawing, etc.)
    pub is_decorative: bool,
}

impl Default for CellStyle {
    fn default() -> Self {
        CellStyle {
            fg: Rgba::from_rgb(255, 255, 255),
            bg: Rgba::from_rgb(0, 0, 0),
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
            is_decorative: false,
        }
    }
}

/// A batched text run - multiple adjacent cells with identical styling
#[derive(Debug, Clone)]
pub struct BatchedTextRun {
    /// Row index (0-based)
    pub row: u16,
    /// Starting column index (0-based)
    pub start_col: u16,
    /// The accumulated text content
    pub text: String,
    /// Style for this run
    pub style: CellStyle,
    /// Number of cells this run spans
    pub cell_count: usize,
}

/// A background rectangle to render
#[derive(Debug, Clone)]
pub struct BackgroundRect {
    /// Row index
    pub row: u16,
    /// Starting column
    pub start_col: u16,
    /// Ending column (exclusive)
    pub end_col: u16,
    /// Background color
    pub color: Rgba,
}

/// Complete renderable content for a terminal frame
#[derive(Debug, Clone)]
pub struct RenderableContent {
    /// Batched text runs for efficient text rendering
    pub text_runs: Vec<BatchedTextRun>,
    /// Background rectangles (merged for efficiency)
    pub backgrounds: Vec<BackgroundRect>,
    /// Cursor position (row, col)
    pub cursor: (u16, u16),
    /// Grid dimensions (rows, cols)
    pub size: (u16, u16),
}

/// Default terminal color palette (basic 16 colors)
fn named_color_to_rgba(color: NamedColor) -> Rgba {
    match color {
        NamedColor::Black => Rgba::from_rgb(0, 0, 0),
        NamedColor::Red => Rgba::from_rgb(205, 49, 49),
        NamedColor::Green => Rgba::from_rgb(13, 188, 121),
        NamedColor::Yellow => Rgba::from_rgb(229, 229, 16),
        NamedColor::Blue => Rgba::from_rgb(36, 114, 200),
        NamedColor::Magenta => Rgba::from_rgb(188, 63, 188),
        NamedColor::Cyan => Rgba::from_rgb(17, 168, 205),
        NamedColor::White => Rgba::from_rgb(229, 229, 229),
        NamedColor::BrightBlack => Rgba::from_rgb(102, 102, 102),
        NamedColor::BrightRed => Rgba::from_rgb(241, 76, 76),
        NamedColor::BrightGreen => Rgba::from_rgb(35, 209, 139),
        NamedColor::BrightYellow => Rgba::from_rgb(245, 245, 67),
        NamedColor::BrightBlue => Rgba::from_rgb(59, 142, 234),
        NamedColor::BrightMagenta => Rgba::from_rgb(214, 112, 214),
        NamedColor::BrightCyan => Rgba::from_rgb(41, 184, 219),
        NamedColor::BrightWhite => Rgba::from_rgb(255, 255, 255),
        NamedColor::Foreground => Rgba::from_rgb(255, 255, 255),
        NamedColor::Background => Rgba::from_rgb(0, 0, 0),
        NamedColor::Cursor => Rgba::from_rgb(255, 255, 255),
        _ => Rgba::from_rgb(255, 255, 255),
    }
}

/// Convert Alacritty color to RGBA
fn alacritty_color_to_rgba(color: alacritty_terminal::vte::ansi::Color) -> Rgba {
    use alacritty_terminal::vte::ansi::Color;
    match color {
        Color::Named(named) => named_color_to_rgba(named),
        Color::Spec(rgb) => Rgba::from_rgb(rgb.r, rgb.g, rgb.b),
        Color::Indexed(idx) => {
            // 256-color palette
            if idx < 16 {
                // Standard colors
                named_color_to_rgba(match idx {
                    0 => NamedColor::Black,
                    1 => NamedColor::Red,
                    2 => NamedColor::Green,
                    3 => NamedColor::Yellow,
                    4 => NamedColor::Blue,
                    5 => NamedColor::Magenta,
                    6 => NamedColor::Cyan,
                    7 => NamedColor::White,
                    8 => NamedColor::BrightBlack,
                    9 => NamedColor::BrightRed,
                    10 => NamedColor::BrightGreen,
                    11 => NamedColor::BrightYellow,
                    12 => NamedColor::BrightBlue,
                    13 => NamedColor::BrightMagenta,
                    14 => NamedColor::BrightCyan,
                    15 => NamedColor::BrightWhite,
                    _ => NamedColor::White,
                })
            } else if idx < 232 {
                // 216 color cube (6x6x6)
                let idx = idx - 16;
                let r = (idx / 36) % 6;
                let g = (idx / 6) % 6;
                let b = idx % 6;
                Rgba::from_rgb(
                    if r == 0 { 0 } else { r * 40 + 55 },
                    if g == 0 { 0 } else { g * 40 + 55 },
                    if b == 0 { 0 } else { b * 40 + 55 },
                )
            } else {
                // Grayscale (24 shades)
                let gray = (idx - 232) * 10 + 8;
                Rgba::from_rgb(gray, gray, gray)
            }
        }
    }
}

/// Batch terminal cells into text runs for efficient rendering.
///
/// This function iterates through the terminal grid and groups adjacent cells
/// with identical styling into single text runs, dramatically reducing the
/// number of draw calls needed.
pub fn batch_cells(term: &Term<EventProxy>, rows: u16, cols: u16) -> RenderableContent {
    let grid = term.grid();
    let mut text_runs = Vec::new();
    let mut backgrounds = Vec::new();

    let default_bg = Rgba::from_rgb(0, 0, 0);

    for row in 0..rows {
        let line_idx = Line(row as i32);
        let mut current_run: Option<BatchedTextRun> = None;
        let mut current_bg: Option<BackgroundRect> = None;

        for col in 0..cols {
            let point = Point::new(line_idx, Column(col as usize));
            let cell = &grid[point];

            let ch = cell.c;
            let fg = alacritty_color_to_rgba(cell.fg);
            let bg = alacritty_color_to_rgba(cell.bg);

            let is_decorative = is_decorative_character(ch);
            let flags = cell.flags;

            let style = CellStyle {
                fg,
                bg,
                bold: flags.contains(Flags::BOLD),
                italic: flags.contains(Flags::ITALIC),
                underline: flags.contains(Flags::UNDERLINE),
                strikethrough: flags.contains(Flags::STRIKEOUT),
                is_decorative,
            };

            // Handle background batching
            if bg != default_bg {
                if let Some(ref mut bg_rect) = current_bg {
                    if bg_rect.color == bg {
                        bg_rect.end_col = col + 1;
                    } else {
                        backgrounds.push(bg_rect.clone());
                        current_bg = Some(BackgroundRect {
                            row,
                            start_col: col,
                            end_col: col + 1,
                            color: bg,
                        });
                    }
                } else {
                    current_bg = Some(BackgroundRect {
                        row,
                        start_col: col,
                        end_col: col + 1,
                        color: bg,
                    });
                }
            } else if let Some(bg_rect) = current_bg.take() {
                backgrounds.push(bg_rect);
            }

            // Handle text run batching
            // We only batch if styles match (excluding background, handled separately)
            let text_style = CellStyle {
                bg: default_bg, // Normalize bg for text comparison
                ..style
            };

            if let Some(ref mut run) = current_run {
                let run_style = CellStyle {
                    bg: default_bg,
                    ..run.style
                };

                if run_style == text_style {
                    run.text.push(ch);
                    run.cell_count += 1;
                } else {
                    text_runs.push(run.clone());
                    current_run = Some(BatchedTextRun {
                        row,
                        start_col: col,
                        text: ch.to_string(),
                        style,
                        cell_count: 1,
                    });
                }
            } else {
                current_run = Some(BatchedTextRun {
                    row,
                    start_col: col,
                    text: ch.to_string(),
                    style,
                    cell_count: 1,
                });
            }
        }

        // Flush remaining run and background for this row
        if let Some(run) = current_run.take() {
            text_runs.push(run);
        }
        if let Some(bg_rect) = current_bg.take() {
            backgrounds.push(bg_rect);
        }
    }

    // Get cursor position
    let cursor_point = term.renderable_content().cursor.point;
    let cursor = (cursor_point.line.0 as u16, cursor_point.column.0 as u16);

    RenderableContent {
        text_runs,
        backgrounds,
        cursor,
        size: (rows, cols),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgba_creation() {
        let color = Rgba::from_rgb(255, 128, 64);
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 128);
        assert_eq!(color.b, 64);
        assert_eq!(color.a, 255);
    }

    #[test]
    fn test_rgba_to_u32() {
        let color = Rgba::from_rgb(0xFF, 0x00, 0x00);
        assert_eq!(color.to_u32(), 0xFF0000);

        let color = Rgba::from_rgb(0x00, 0xFF, 0x00);
        assert_eq!(color.to_u32(), 0x00FF00);

        let color = Rgba::from_rgb(0x00, 0x00, 0xFF);
        assert_eq!(color.to_u32(), 0x0000FF);
    }

    #[test]
    fn test_cell_style_default() {
        let style = CellStyle::default();
        assert_eq!(style.fg, Rgba::from_rgb(255, 255, 255));
        assert_eq!(style.bg, Rgba::from_rgb(0, 0, 0));
        assert!(!style.bold);
        assert!(!style.italic);
        assert!(!style.underline);
    }

    #[test]
    fn test_named_color_conversion() {
        let red = named_color_to_rgba(NamedColor::Red);
        assert!(red.r > 200);
        assert!(red.g < 100);
        assert!(red.b < 100);

        let green = named_color_to_rgba(NamedColor::Green);
        assert!(green.g > 150);
    }
}
