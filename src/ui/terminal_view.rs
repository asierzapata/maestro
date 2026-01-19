use gpui::*;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::terminal::TerminalSession;

/// Text selection state for copy/paste functionality
#[derive(Clone, Debug)]
pub struct Selection {
    pub start: (u16, u16),
    pub end: (u16, u16),
}

/// Terminal view component that renders terminal content and handles user input
pub struct TerminalView {
    session: Arc<Mutex<TerminalSession>>,
    font_size: f32,
    scroll_offset: usize,
    selection: Option<Selection>,
    needs_refresh: bool,
}

impl TerminalView {
    /// Creates a new terminal view with the given session
    pub fn new(session: TerminalSession, cx: &mut Context<Self>) -> Self {
        let session_arc = Arc::new(Mutex::new(session));

        TerminalView {
            session: session_arc,
            font_size: 14.0,
            scroll_offset: 0,
            selection: None,
            needs_refresh: false,
        }
    }

    /// Handles keyboard input
    fn handle_key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        // Convert key event to terminal input sequence
        let input = self.key_event_to_input(event);

        if let Some(data) = input {
            if let Ok(mut session) = self.session.lock() {
                let _ = session.write_input(&data);
                cx.notify();
            }
        }
    }

    /// Converts GPUI key events to terminal input sequences
    fn key_event_to_input(&self, event: &KeyDownEvent) -> Option<Vec<u8>> {
        let key = &event.keystroke.key;

        // Handle special keys
        match key.as_str() {
            "enter" => Some(b"\r".to_vec()),
            "tab" => Some(b"\t".to_vec()),
            "backspace" => Some(b"\x7f".to_vec()),
            "escape" => Some(b"\x1b".to_vec()),
            "up" => Some(b"\x1b[A".to_vec()),
            "down" => Some(b"\x1b[B".to_vec()),
            "right" => Some(b"\x1b[C".to_vec()),
            "left" => Some(b"\x1b[D".to_vec()),
            "home" => Some(b"\x1b[H".to_vec()),
            "end" => Some(b"\x1b[F".to_vec()),
            "pageup" => Some(b"\x1b[5~".to_vec()),
            "pagedown" => Some(b"\x1b[6~".to_vec()),
            "delete" => Some(b"\x1b[3~".to_vec()),
            _ => {
                // Handle regular character input
                if key.len() == 1 {
                    // Check for Ctrl modifier
                    if event.keystroke.modifiers.control {
                        let ch = key.chars().next()?;
                        if ch.is_ascii_alphabetic() {
                            // Ctrl+A = 0x01, Ctrl+B = 0x02, etc.
                            let ctrl_char = (ch.to_ascii_lowercase() as u8) - b'a' + 1;
                            return Some(vec![ctrl_char]);
                        }
                    }

                    // Regular character
                    Some(key.as_bytes().to_vec())
                } else {
                    None
                }
            }
        }
    }

    /// Handles mouse down event for text selection
    fn handle_mouse_down(&mut self, _event: &MouseDownEvent, _cx: &mut Context<Self>) {
        // TODO: Implement text selection start
    }

    /// Handles mouse move event for text selection
    fn handle_mouse_move(&mut self, _event: &MouseMoveEvent, _cx: &mut Context<Self>) {
        // TODO: Implement text selection update
    }

    /// Handles mouse up event for text selection
    fn handle_mouse_up(&mut self, _event: &MouseUpEvent, _cx: &mut Context<Self>) {
        // TODO: Implement text selection end
    }

    /// Handles copy operation
    fn handle_copy(&mut self, cx: &mut Context<Self>) {
        if let Some(selection) = &self.selection {
            if let Ok(session) = self.session.lock() {
                let content = session.get_visible_content();
                let mut selected_text = String::new();

                // Extract selected text from content
                let (start_row, start_col) = selection.start;
                let (end_row, end_col) = selection.end;

                for row in start_row..=end_row {
                    if let Some(line) = content.get(row as usize) {
                        if row == start_row && row == end_row {
                            // Single line selection
                            let start = start_col as usize;
                            let end = end_col as usize;
                            if start < line.len() && end <= line.len() {
                                selected_text.push_str(&line[start..end]);
                            }
                        } else if row == start_row {
                            // First line
                            let start = start_col as usize;
                            if start < line.len() {
                                selected_text.push_str(&line[start..]);
                                selected_text.push('\n');
                            }
                        } else if row == end_row {
                            // Last line
                            let end = end_col as usize;
                            if end <= line.len() {
                                selected_text.push_str(&line[..end]);
                            }
                        } else {
                            // Middle lines
                            selected_text.push_str(line);
                            selected_text.push('\n');
                        }
                    }
                }

                // Copy to clipboard
                cx.write_to_clipboard(ClipboardItem::new_string(selected_text));
            }
        }
    }

    /// Handles paste operation
    fn handle_paste(&mut self, cx: &mut Context<Self>) {
        if let Some(clipboard_item) = cx.read_from_clipboard() {
            if let Some(text) = clipboard_item.text() {
                if let Ok(mut session) = self.session.lock() {
                    let _ = session.write_input(text.as_bytes());
                    cx.notify();
                }
            }
        }
    }

    /// Renders terminal content
    fn render_terminal_content(&self, cx: &mut Context<Self>) -> Div {
        let session = self.session.lock().unwrap();
        let content = session.get_visible_content();
        let (cursor_row, cursor_col) = session.get_cursor_position();

        div()
            .w_full()
            .h_full()
            .bg(rgb(0x000000))
            .font_family("Monaco")
            .text_size(px(self.font_size))
            .text_color(rgb(0xffffff))
            .overflow_hidden()
            .children(
                content
                    .iter()
                    .enumerate()
                    .map(|(row_idx, line)| self.render_line(row_idx, line, cursor_row, cursor_col)),
            )
    }

    /// Renders a single line of terminal content
    fn render_line(&self, row_idx: usize, line: &str, cursor_row: u16, cursor_col: u16) -> Div {
        div()
            .flex()
            .flex_row()
            .h(px(self.font_size * 1.2))
            .children(line.chars().enumerate().map(|(col_idx, ch)| {
                let is_cursor = row_idx == cursor_row as usize && col_idx == cursor_col as usize;
                self.render_char(ch, is_cursor)
            }))
    }

    /// Renders a single character with optional cursor styling
    fn render_char(&self, ch: char, is_cursor: bool) -> Div {
        let mut div_element = div()
            .w(px(self.font_size * 0.6)) // Monospace character width
            .h_full();

        if is_cursor {
            div_element = div_element.bg(rgb(0xffffff)).text_color(rgb(0x000000));
        }

        div_element.child(ch.to_string())
    }
}

impl Render for TerminalView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w_full()
            .h_full()
            .bg(rgb(0x1e1e1e))
            .child(self.render_terminal_content(cx))
            .on_key_down(cx.listener(|this, event, _window, cx| {
                this.handle_key_down(event, cx);
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event, _window, cx| {
                    this.handle_mouse_down(event, cx);
                }),
            )
            .on_mouse_move(cx.listener(|this, event, _window, cx| {
                this.handle_mouse_move(event, cx);
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, event, _window, cx| {
                    this.handle_mouse_up(event, cx);
                }),
            )
    }
}
