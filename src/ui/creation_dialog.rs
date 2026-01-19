use crate::ui::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::*;

/// Callback type for when the user confirms creation
pub type OnCreateCallback =
    Box<dyn Fn(String, &mut Window, &mut Context<CreationDialog>) + 'static>;

/// Modal dialog for creating a new worktree
pub struct CreationDialog {
    branch_name: SharedString,
    is_visible: bool,
    error_message: Option<SharedString>,
    theme: Theme,
    on_create: Option<OnCreateCallback>,
    focus_handle: FocusHandle,
}

impl CreationDialog {
    /// Create a new CreationDialog instance
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            branch_name: "".into(),
            is_visible: false,
            error_message: None,
            theme: Theme::new(),
            on_create: None,
            focus_handle: cx.focus_handle(),
        }
    }

    /// Show the dialog
    pub fn show(&mut self, cx: &mut Context<Self>) {
        self.is_visible = true;
        self.branch_name = "".into();
        self.error_message = None;
        cx.notify();
    }

    /// Hide the dialog
    pub fn hide(&mut self, cx: &mut Context<Self>) {
        self.is_visible = false;
        self.branch_name = "".into();
        self.error_message = None;
        cx.notify();
    }

    /// Check if the dialog is visible
    pub fn is_visible(&self) -> bool {
        self.is_visible
    }

    /// Set the error message to display
    pub fn set_error(&mut self, error: String, cx: &mut Context<Self>) {
        self.error_message = Some(error.into());
        cx.notify();
    }

    /// Clear the error message
    pub fn clear_error(&mut self, cx: &mut Context<Self>) {
        self.error_message = None;
        cx.notify();
    }

    /// Set the callback for when the user confirms creation
    pub fn on_create(
        mut self,
        callback: impl Fn(String, &mut Window, &mut Context<Self>) + 'static,
    ) -> Self {
        self.on_create = Some(Box::new(callback));
        self
    }

    /// Handle the Create button click
    fn handle_create(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let branch_name = self.branch_name.to_string();

        if branch_name.trim().is_empty() {
            self.set_error("Branch name cannot be empty".to_string(), cx);
            return;
        }

        // Call the callback if set
        if let Some(ref callback) = self.on_create {
            callback(branch_name, window, cx);
        }
    }

    /// Handle the Cancel button click
    fn handle_cancel(&mut self, cx: &mut Context<Self>) {
        self.hide(cx);
    }

    /// Handle keyboard input for typing
    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = event.keystroke.key.as_str();

        if key == "enter" {
            self.handle_create(window, cx);
        } else if key == "escape" {
            self.handle_cancel(cx);
        } else if key == "backspace" {
            // Remove last character
            let mut name = self.branch_name.to_string();
            name.pop();
            self.branch_name = name.into();
            if self.error_message.is_some() {
                self.clear_error(cx);
            }
            cx.notify();
        } else if key.len() == 1 {
            // Add character to branch name (single character keys only)
            let mut name = self.branch_name.to_string();
            name.push_str(key);
            self.branch_name = name.into();
            if self.error_message.is_some() {
                self.clear_error(cx);
            }
            cx.notify();
        }
    }

    /// Render the modal backdrop
    fn render_backdrop(&self, cx: &mut Context<Self>) -> Div {
        div()
            .absolute()
            .top_0()
            .left_0()
            .w_full()
            .h_full()
            .bg(hsla(0.0, 0.0, 0.0, 0.5))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _event: &MouseDownEvent, _window, cx| {
                    this.handle_cancel(cx);
                }),
            )
    }

    /// Render the dialog box
    fn render_dialog(&self, cx: &mut Context<Self>) -> Div {
        div()
            .absolute()
            .top_1_2()
            .left_1_2()
            .w(px(400.0))
            .bg(self.theme.bg_surface)
            .border_1()
            .border_color(self.theme.border_subtle)
            .rounded_lg()
            .shadow_lg()
            .flex()
            .flex_col()
            .gap_4()
            .p_6()
            .child(self.render_header())
            .child(self.render_input())
            .when_some(self.error_message.clone(), |this, error| {
                this.child(self.render_error(error))
            })
            .child(self.render_buttons(cx))
    }

    /// Render the dialog header
    fn render_header(&self) -> Div {
        div()
            .text_lg()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(self.theme.text_primary)
            .child("Create New Worktree")
    }

    /// Render the input field
    fn render_input(&self) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .text_color(self.theme.text_secondary)
                    .child("Branch name"),
            )
            .child(
                div()
                    .w_full()
                    .px_3()
                    .py_2()
                    .bg(self.theme.bg_primary)
                    .border_1()
                    .border_color(self.theme.border_subtle)
                    .rounded_md()
                    .text_color(self.theme.text_primary)
                    .when(self.branch_name.is_empty(), |this| {
                        this.text_color(self.theme.text_tertiary)
                            .child("feature/my-branch")
                    })
                    .when(!self.branch_name.is_empty(), |this| {
                        this.child(self.branch_name.clone())
                    }),
            )
    }

    /// Render the error message
    fn render_error(&self, error: SharedString) -> Div {
        div()
            .px_3()
            .py_2()
            .bg(hsla(0.0, 0.7, 0.3, 0.2))
            .border_1()
            .border_color(hsla(0.0, 0.7, 0.5, 0.5))
            .rounded_md()
            .text_sm()
            .text_color(hsla(0.0, 0.7, 0.7, 1.0))
            .child(error)
    }

    /// Render the action buttons
    fn render_buttons(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_row()
            .justify_end()
            .gap_2()
            .child(self.render_cancel_button(cx))
            .child(self.render_create_button(cx))
    }

    /// Render the Cancel button
    fn render_cancel_button(&self, cx: &mut Context<Self>) -> Div {
        div()
            .px_4()
            .py_2()
            .bg(self.theme.bg_hover)
            .rounded_md()
            .cursor_pointer()
            .hover(|style| style.bg(self.theme.bg_selected))
            .text_sm()
            .text_color(self.theme.text_primary)
            .child("Cancel")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _event: &MouseDownEvent, _window, cx| {
                    this.handle_cancel(cx);
                }),
            )
    }

    /// Render the Create button
    fn render_create_button(&self, cx: &mut Context<Self>) -> Div {
        div()
            .px_4()
            .py_2()
            .bg(self.theme.accent)
            .rounded_md()
            .cursor_pointer()
            .hover(|style| style.bg(self.theme.accent_hover))
            .text_sm()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(hsla(0.0, 0.0, 1.0, 1.0))
            .child("Create")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _event: &MouseDownEvent, window, cx| {
                    this.handle_create(window, cx);
                }),
            )
    }
}

impl Render for CreationDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.is_visible {
            return div();
        }

        div()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::handle_key_down))
            .child(self.render_backdrop(cx))
            .child(self.render_dialog(cx))
    }
}
