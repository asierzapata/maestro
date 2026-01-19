use gpui::*;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::terminal::render::{RenderableContent, Rgba};
use crate::terminal::TerminalSession;

/// Text selection state for copy/paste functionality
#[derive(Clone, Debug)]
pub struct Selection {
    pub start: (u16, u16),
    pub end: (u16, u16),
}

/// Terminal view state
#[derive(Clone, Debug, PartialEq)]
pub enum TerminalState {
    Loading,
    Ready,
    Error(String),
}

/// Default terminal font family with good Unicode/Powerline support
/// Users should have a Nerd Font or similar installed for best results
const DEFAULT_TERMINAL_FONT: &str = "JetBrains Mono";

/// Fallback fonts in order of preference (GPUI will try these if primary fails)
/// Note: GPUI uses system font fallback, so listing multiple serves as documentation
const FALLBACK_FONTS: &[&str] = &[
    "JetBrains Mono",     // Excellent Unicode coverage
    "Hack",               // Good alternative
    "Fira Code",          // Popular coding font
    "Monaco",             // macOS default
    "Consolas",           // Windows default
    "DejaVu Sans Mono",   // Linux common
    "Menlo",              // macOS alternative
];

/// Terminal view component that renders terminal content and handles user input
pub struct TerminalView {
    session: Arc<Mutex<TerminalSession>>,
    font_size: f32,
    font_family: String,
    scroll_offset: usize,
    selection: Option<Selection>,
    state: TerminalState,
    worktree_path: PathBuf,
    focus_handle: FocusHandle,
    /// Cached renderable content for efficient batched rendering
    cached_content: Option<RenderableContent>,
    /// Counter for polling frames after user input
    poll_frames_remaining: u32,
}

impl TerminalView {
    /// Creates a new terminal view with the given session
    pub fn new(session: TerminalSession, cx: &mut Context<Self>) -> Self {
        let worktree_path = session.worktree_path().clone();
        let session_arc = Arc::new(Mutex::new(session));
        let focus_handle = cx.focus_handle();

        // Get initial renderable content
        let cached_content = {
            let session = session_arc.lock().unwrap();
            Some(session.get_renderable_content())
        };

        TerminalView {
            session: session_arc,
            font_size: 14.0,
            font_family: DEFAULT_TERMINAL_FONT.to_string(),
            scroll_offset: 0,
            selection: None,
            state: TerminalState::Ready,
            worktree_path,
            focus_handle,
            cached_content,
            poll_frames_remaining: 10, // Start with some polling to catch initial output
        }
    }

    /// Sets the terminal font family
    pub fn set_font_family(&mut self, font_family: impl Into<String>) {
        self.font_family = font_family.into();
    }

    /// Gets the current font family
    pub fn font_family(&self) -> &str {
        &self.font_family
    }

    /// Returns the list of recommended fallback fonts for terminal rendering
    pub fn recommended_fonts() -> &'static [&'static str] {
        FALLBACK_FONTS
    }

    /// Polls for output and updates cached content if there's new data
    fn poll_and_update_cache(&mut self) -> bool {
        if let Ok(mut session) = self.session.lock() {
            // Process events from the async channel
            let had_output = match session.process_events() {
                Some(had_output) => had_output,
                None => {
                    // Process exited
                    self.state = TerminalState::Error("Process exited".to_string());
                    false
                }
            };
            if had_output {
                // Use batched renderable content for efficient rendering
                self.cached_content = Some(session.get_renderable_content());
            }
            had_output
        } else {
            false
        }
    }

    /// Restarts the terminal session
    pub fn restart_session(&mut self, cx: &mut Context<Self>) {
        self.state = TerminalState::Loading;
        cx.notify();

        // Create a new session
        match TerminalSession::new(self.worktree_path.clone(), None, 24, 80) {
            Ok(new_session) => {
                let session_arc = Arc::new(Mutex::new(new_session));

                // Get initial renderable content
                {
                    let session = session_arc.lock().unwrap();
                    self.cached_content = Some(session.get_renderable_content());
                }

                self.session = session_arc;
                self.state = TerminalState::Ready;
                self.selection = None;
                self.scroll_offset = 0;
                self.poll_frames_remaining = 10;

                eprintln!("Terminal session restarted successfully");
            }
            Err(e) => {
                let error_msg = format!("Failed to restart terminal: {}", e);
                eprintln!("{}", error_msg);
                self.state = TerminalState::Error(error_msg);
            }
        }
        cx.notify();
    }

    /// Checks if the session is alive and updates state accordingly
    fn check_session_health(&mut self) {
        if let Ok(session) = self.session.lock() {
            if !session.is_alive() && self.state == TerminalState::Ready {
                self.state = TerminalState::Error("Terminal session ended".to_string());
            }
        }
    }

    /// Handles keyboard input
    fn handle_key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        // Convert key event to terminal input sequence
        let input = self.key_event_to_input(event);

        if let Some(data) = input {
            if let Ok(mut session) = self.session.lock() {
                let _ = session.write_input(&data);
            }
            // Start polling for output
            self.poll_frames_remaining = 30; // Poll for ~500ms after input
            cx.notify();
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
                // Get the raw string content for copy operation
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

    /// Converts our Rgba color to GPUI rgb format
    fn rgba_to_gpui(&self, color: &Rgba) -> u32 {
        ((color.r as u32) << 16) | ((color.g as u32) << 8) | (color.b as u32)
    }

    /// Renders terminal content using batched text runs for performance
    fn render_terminal_content(&self) -> Div {
        let Some(ref content) = self.cached_content else {
            // No content yet - render empty terminal
            return div()
                .w_full()
                .h_full()
                .bg(rgb(0x000000))
                .font_family(self.font_family.clone())
                .text_size(px(self.font_size));
        };

        let (cursor_row, cursor_col) = content.cursor;
        let (rows, _cols) = content.size;
        let line_height = px(self.font_size * 1.2);
        let char_width = px(self.font_size * 0.6); // Approximate monospace char width

        // Build row elements
        let mut row_elements: Vec<Div> = Vec::with_capacity(rows as usize);

        for row in 0..rows {
            // Get background rectangles for this row
            let row_backgrounds: Vec<_> = content
                .backgrounds
                .iter()
                .filter(|bg| bg.row == row)
                .collect();

            // Get text runs for this row
            let row_runs: Vec<_> = content
                .text_runs
                .iter()
                .filter(|run| run.row == row)
                .collect();

            // Build the row with backgrounds and text overlaid
            let mut row_div = div()
                .h(line_height)
                .w_full()
                .relative();

            // Render background rectangles first (positioned absolutely)
            for bg in row_backgrounds {
                let left_offset = px((bg.start_col as f32) * self.font_size * 0.6);
                let width = px(((bg.end_col - bg.start_col) as f32) * self.font_size * 0.6);
                let bg_color = self.rgba_to_gpui(&bg.color);

                row_div = row_div.child(
                    div()
                        .absolute()
                        .top_0()
                        .left(left_offset)
                        .w(width)
                        .h(line_height)
                        .bg(rgb(bg_color)),
                );
            }

            // Render text runs (each positioned absolutely)
            for run in &row_runs {
                let left_offset = px((run.start_col as f32) * self.font_size * 0.6);
                let fg_color = self.rgba_to_gpui(&run.style.fg);

                let mut text_div = div()
                    .absolute()
                    .top_0()
                    .left(left_offset)
                    .text_color(rgb(fg_color));

                // Apply text styling
                if run.style.bold {
                    text_div = text_div.font_weight(FontWeight::BOLD);
                }
                if run.style.italic {
                    text_div = text_div.italic();
                }

                // Handle cursor within this run
                let run_start = run.start_col;
                let run_end = run.start_col + run.cell_count as u16;

                if row == cursor_row && cursor_col >= run_start && cursor_col < run_end {
                    // Cursor is within this run - split the text
                    let cursor_offset = (cursor_col - run_start) as usize;
                    let chars: Vec<char> = run.text.chars().collect();

                    let before: String = chars.iter().take(cursor_offset).collect();
                    let cursor_char = chars.get(cursor_offset).copied().unwrap_or(' ');
                    let after: String = chars.iter().skip(cursor_offset + 1).collect();

                    text_div = text_div
                        .flex()
                        .flex_row()
                        .child(div().child(before))
                        .child(
                            div()
                                .bg(rgb(0xffffff))
                                .text_color(rgb(0x000000))
                                .child(cursor_char.to_string()),
                        )
                        .child(div().child(after));
                } else {
                    text_div = text_div.child(run.text.clone());
                }

                row_div = row_div.child(text_div);
            }

            // If cursor is on this row but not within any text run (empty line), render cursor
            if row == cursor_row {
                let cursor_in_run = row_runs
                    .iter()
                    .any(|run| {
                        cursor_col >= run.start_col
                            && cursor_col < run.start_col + run.cell_count as u16
                    });

                if !cursor_in_run {
                    let cursor_left = px((cursor_col as f32) * self.font_size * 0.6);
                    row_div = row_div.child(
                        div()
                            .absolute()
                            .top_0()
                            .left(cursor_left)
                            .w(char_width)
                            .h(line_height)
                            .bg(rgb(0xffffff)),
                    );
                }
            }

            row_elements.push(row_div);
        }

        div()
            .w_full()
            .h_full()
            .bg(rgb(0x000000))
            .font_family(self.font_family.clone())
            .text_size(px(self.font_size))
            .text_color(rgb(0xffffff))
            .overflow_hidden()
            .children(row_elements)
    }
}

impl Render for TerminalView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Check session health
        self.check_session_health();

        // Poll for output and update cache
        if self.state == TerminalState::Ready {
            let had_output = self.poll_and_update_cache();

            if had_output {
                // Keep polling while there's output
                self.poll_frames_remaining = 30;
                cx.notify();
            } else if self.poll_frames_remaining > 0 {
                // Continue polling for a bit after input
                self.poll_frames_remaining -= 1;
                cx.notify();
            }
        }

        match &self.state {
            TerminalState::Loading => self.render_loading_state().into_any_element(),
            TerminalState::Error(msg) => self.render_error_state(msg.clone(), cx).into_any_element(),
            TerminalState::Ready => self.render_ready_state(cx).into_any_element(),
        }
    }
}

impl TerminalView {
    /// Renders the loading state
    fn render_loading_state(&self) -> impl IntoElement {
        div()
            .w_full()
            .h_full()
            .bg(rgb(0x1e1e1e))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_color(rgb(0xffffff))
                            .child("Initializing terminal..."),
                    ),
            )
    }

    /// Renders the error state with restart button
    fn render_error_state(&self, error_msg: String, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w_full()
            .h_full()
            .bg(rgb(0x1e1e1e))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_4()
                    .child(
                        div()
                            .text_color(rgb(0xff6b6b))
                            .child("Terminal Error"),
                    )
                    .child(
                        div()
                            .text_color(rgb(0xaaaaaa))
                            .text_sm()
                            .max_w(px(400.0))
                            .text_center()
                            .child(error_msg),
                    )
                    .child(
                        div()
                            .px_4()
                            .py_2()
                            .bg(rgb(0x3a3a3a))
                            .rounded_md()
                            .cursor_pointer()
                            .text_color(rgb(0xffffff))
                            .hover(|style| style.bg(rgb(0x4a4a4a)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _event, _window, cx| {
                                    this.restart_session(cx);
                                }),
                            )
                            .child("Restart Terminal"),
                    ),
            )
    }

    /// Renders the ready/normal terminal state
    fn render_ready_state(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w_full()
            .h_full()
            .bg(rgb(0x1e1e1e))
            .flex()
            .flex_col()
            .track_focus(&self.focus_handle)
            .child(self.render_terminal_content())
            .child(self.render_status_bar())
            .on_key_down(cx.listener(|this, event, _window, cx| {
                this.handle_key_down(event, cx);
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _event, window, cx| {
                    // Focus the terminal when clicked
                    this.focus_handle.focus(window);
                    cx.notify();
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

    /// Renders the status bar with keyboard shortcuts hint
    fn render_status_bar(&self) -> Div {
        div()
            .w_full()
            .h(px(20.0))
            .bg(rgb(0x2d2d2d))
            .px_2()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x808080))
                    .child(self.worktree_path.display().to_string()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x606060))
                    .child("Ctrl+Shift+C: Copy | Ctrl+Shift+V: Paste"),
            )
    }
}
