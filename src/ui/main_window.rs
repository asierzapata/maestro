use crate::git::{Worktree, get_repository_name, list_worktrees, worktree};
use crate::ui::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::path::PathBuf;

/// Main application window that orchestrates the sidebar and feature view
pub struct MainWindow {
    workspace_path: PathBuf,
    workspace_name: SharedString,
    worktrees: Vec<Worktree>,
    selected_worktree_index: usize,
    theme: Theme,
    error_message: Option<String>,
    // Dialog state
    show_create_dialog: bool,
    dialog_branch_name: SharedString,
    dialog_error: Option<SharedString>,
    focus_handle: FocusHandle,
}

impl MainWindow {
    /// Create a new MainWindow instance
    ///
    /// # Arguments
    ///
    /// * `workspace_path` - Path to the workspace (git repository)
    /// * `cx` - GPUI context for creating views
    ///
    /// # Returns
    ///
    /// A new MainWindow or an error if workspace loading fails
    pub fn new(workspace_path: PathBuf, cx: &mut Context<Self>) -> Result<Self, String> {
        // Get workspace name
        let workspace_name = get_repository_name(&workspace_path)
            .map_err(|e| format!("Failed to get repository name: {}", e))?;

        // Load worktrees
        let worktrees = list_worktrees(&workspace_path)
            .map_err(|e| format!("Failed to load worktrees: {}", e))?;

        if worktrees.is_empty() {
            return Err("No worktrees found in repository".to_string());
        }

        Ok(Self {
            workspace_path,
            workspace_name: workspace_name.into(),
            worktrees,
            selected_worktree_index: 0,
            theme: Theme::new(),
            error_message: None,
            show_create_dialog: false,
            dialog_branch_name: "".into(),
            dialog_error: None,
            focus_handle: cx.focus_handle(),
        })
    }

    /// Create a MainWindow with an error message
    pub fn with_error(workspace_path: PathBuf, error: String, cx: &mut Context<Self>) -> Self {
        let workspace_name = workspace_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown")
            .to_string();

        Self {
            workspace_path,
            workspace_name: workspace_name.into(),
            worktrees: vec![],
            selected_worktree_index: 0,
            theme: Theme::new(),
            error_message: Some(error),
            show_create_dialog: false,
            dialog_branch_name: "".into(),
            dialog_error: None,
            focus_handle: cx.focus_handle(),
        }
    }

    /// Get the currently selected worktree
    fn selected_worktree(&self) -> Option<&Worktree> {
        self.worktrees.get(self.selected_worktree_index)
    }

    /// Handle worktree selection
    fn handle_worktree_click(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx < self.worktrees.len() {
            self.selected_worktree_index = idx;
            println!(
                "Selected worktree: {} (branch: {})",
                self.worktrees[idx].path.display(),
                self.worktrees[idx].branch
            );
            cx.notify();
        }
    }

    /// Refresh the list of worktrees from git
    fn refresh_worktrees(&mut self, cx: &mut Context<Self>) {
        match list_worktrees(&self.workspace_path) {
            Ok(worktrees) => {
                self.worktrees = worktrees;
                // Keep selection valid
                if self.selected_worktree_index >= self.worktrees.len() {
                    self.selected_worktree_index = 0;
                }
                cx.notify();
            }
            Err(e) => {
                eprintln!("Failed to refresh worktrees: {}", e);
            }
        }
    }

    /// Handle worktree creation
    fn handle_create_worktree(
        &mut self,
        branch_name: String,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Validate and create worktree
        match worktree::validate_branch_name(&branch_name) {
            Ok(_) => {
                match worktree::create_worktree(&self.workspace_path, &branch_name, None) {
                    Ok(new_worktree) => {
                        println!(
                            "Created worktree: {} ({})",
                            new_worktree.path.display(),
                            new_worktree.branch
                        );

                        // Hide dialog
                        self.show_create_dialog = false;
                        self.dialog_branch_name = "".into();
                        self.dialog_error = None;

                        // Refresh worktree list
                        self.refresh_worktrees(cx);

                        // Select the newly created worktree
                        if let Some(idx) = self
                            .worktrees
                            .iter()
                            .position(|wt| wt.path == new_worktree.path)
                        {
                            self.selected_worktree_index = idx;
                            cx.notify();
                        }
                    }
                    Err(e) => {
                        // Show error in dialog
                        self.dialog_error = Some(e.to_string().into());
                        cx.notify();
                    }
                }
            }
            Err(e) => {
                // Show validation error in dialog
                self.dialog_error = Some(e.to_string().into());
                cx.notify();
            }
        }
    }

    /// Handle the create button click
    fn handle_create_button_click(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_create_dialog = true;
        self.dialog_branch_name = "".into();
        self.dialog_error = None;
        self.focus_handle.focus(window);
        cx.notify();
    }

    /// Handle dialog cancel
    fn handle_dialog_cancel(&mut self, cx: &mut Context<Self>) {
        self.show_create_dialog = false;
        self.dialog_branch_name = "".into();
        self.dialog_error = None;
        cx.notify();
    }

    /// Handle dialog key input
    fn handle_dialog_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Only handle keys when in creation mode
        if !self.show_create_dialog {
            return;
        }

        let key = event.keystroke.key.as_str();

        if key == "enter" {
            let branch_name = self.dialog_branch_name.to_string();
            if !branch_name.trim().is_empty() {
                self.handle_create_worktree(branch_name, window, cx);
            } else {
                self.dialog_error = Some("Branch name cannot be empty".into());
                cx.notify();
            }
        } else if key == "escape" {
            self.handle_dialog_cancel(cx);
        } else if key == "backspace" {
            let mut name = self.dialog_branch_name.to_string();
            name.pop();
            self.dialog_branch_name = name.into();
            self.dialog_error = None;
            cx.notify();
        } else if key.len() == 1 {
            let mut name = self.dialog_branch_name.to_string();
            name.push_str(key);
            self.dialog_branch_name = name.into();
            self.dialog_error = None;
            cx.notify();
        }
    }

    /// Render the sidebar with worktrees
    fn render_sidebar(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .w(px(280.0))
            .h_full()
            .bg(self.theme.bg_surface)
            .border_r_1()
            .border_color(self.theme.border_subtle)
            .child(self.render_sidebar_header(cx))
            .child(self.render_worktree_list(cx))
    }

    /// Render the sidebar header with workspace name and buttons
    fn render_sidebar_header(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px_4()
            .py_3()
            .border_b_1()
            .border_color(self.theme.border_subtle)
            .child(
                div()
                    .text_base()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(self.theme.text_primary)
                    .child(self.workspace_name.clone()),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_1()
                    .child(self.render_create_button(cx))
                    .child(self.render_settings_button(cx)),
            )
    }

    /// Render the create worktree button
    fn render_create_button(&self, cx: &mut Context<Self>) -> Div {
        div()
            .w(px(28.0))
            .h(px(28.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_md()
            .text_color(self.theme.text_secondary)
            .cursor_pointer()
            .hover(|style| style.bg(self.theme.bg_hover))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _event: &MouseDownEvent, window, cx| {
                    this.handle_create_button_click(window, cx);
                }),
            )
            .child("+")
    }

    /// Render the settings button placeholder
    fn render_settings_button(&self, cx: &mut Context<Self>) -> Div {
        div()
            .w(px(28.0))
            .h(px(28.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_md()
            .text_color(self.theme.text_secondary)
            .cursor_pointer()
            .hover(|style| style.bg(self.theme.bg_hover))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_this, _event: &MouseDownEvent, _window, _cx| {
                    println!("Settings button clicked");
                }),
            )
            .child("⚙")
    }

    /// Render the list of worktrees
    fn render_worktree_list(&self, cx: &mut Context<Self>) -> Div {
        let mut container = div().flex().flex_col().gap_1().px_2().py_2().child(
            div()
                .text_xs()
                .text_color(self.theme.text_tertiary)
                .px_2()
                .mb_1()
                .child("WORKTREES"),
        );

        // Show inline creation input if in creation mode
        if self.show_create_dialog {
            container = container.child(self.render_inline_creation_input(cx));
        }

        for (idx, worktree) in self.worktrees.iter().enumerate() {
            container = container.child(self.render_worktree_item(idx, worktree, cx));
        }

        container
    }

    /// Render inline creation input in the worktree list
    fn render_inline_creation_input(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .px_3()
            .py_2()
            .mb_2()
            .bg(self.theme.bg_primary)
            .border_1()
            .border_color(self.theme.accent)
            .rounded_md()
            .child(
                div()
                    .text_xs()
                    .text_color(self.theme.text_secondary)
                    .child("New branch name:"),
            )
            .child(
                div()
                    .px_2()
                    .py_1()
                    .text_sm()
                    .text_color(self.theme.text_primary)
                    .when(self.dialog_branch_name.is_empty(), |this| {
                        this.text_color(self.theme.text_tertiary)
                            .child("feature/my-branch")
                    })
                    .when(!self.dialog_branch_name.is_empty(), |this| {
                        this.child(self.dialog_branch_name.clone())
                    }),
            )
            .when_some(self.dialog_error.clone(), |this, error| {
                this.child(
                    div()
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(hsla(0.0, 0.7, 0.6, 1.0))
                        .child(error),
                )
            })
            .child(
                div().flex().flex_row().justify_end().gap_2().mt_2().child(
                    div()
                        .px_3()
                        .py_1()
                        .text_xs()
                        .text_color(self.theme.text_secondary)
                        .child("ESC to cancel • ENTER to create"),
                ),
            )
    }

    /// Render a single worktree item
    fn render_worktree_item(&self, idx: usize, worktree: &Worktree, cx: &mut Context<Self>) -> Div {
        let is_selected = self.selected_worktree_index == idx;
        let is_root = idx == 0;

        let bg_color = if is_selected {
            self.theme.bg_selected
        } else {
            self.theme.bg_surface
        };

        let worktree_name = if is_root {
            "root".to_string()
        } else {
            worktree
                .path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string()
        };

        let branch_display = if worktree.is_detached {
            format!("detached: {}", &worktree.branch)
        } else {
            worktree.branch.clone()
        };

        div()
            .flex()
            .flex_col()
            .gap_1()
            .px_3()
            .py_2()
            .bg(bg_color)
            .rounded_md()
            .cursor_pointer()
            .hover(|style| {
                if !is_selected {
                    style.bg(self.theme.bg_hover)
                } else {
                    style
                }
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _event: &MouseDownEvent, _window, cx| {
                    this.handle_worktree_click(idx, cx);
                }),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .text_color(if is_selected {
                                self.theme.text_primary
                            } else {
                                self.theme.text_secondary
                            })
                            .font_weight(if is_selected {
                                FontWeight::SEMIBOLD
                            } else {
                                FontWeight::NORMAL
                            })
                            .child(worktree_name),
                    )
                    .when(worktree.is_locked, |this| {
                        this.child(
                            div()
                                .text_xs()
                                .text_color(self.theme.text_tertiary)
                                .child("🔒"),
                        )
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(self.theme.text_tertiary)
                    .child(branch_display),
            )
    }

    /// Render the content area (placeholder for now)
    fn render_content(&self) -> Div {
        let selected_worktree = self.selected_worktree();

        div()
            .flex()
            .flex_col()
            .flex_1()
            .bg(self.theme.bg_primary)
            .items_center()
            .justify_center()
            .gap_4()
            .child(
                div()
                    .text_lg()
                    .text_color(self.theme.text_primary)
                    .child("Feature View"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(self.theme.text_secondary)
                    .child("Selected worktree:"),
            )
            .when_some(selected_worktree, |this, worktree| {
                this.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .px_6()
                        .py_4()
                        .bg(self.theme.bg_surface)
                        .rounded_md()
                        .border_1()
                        .border_color(self.theme.border_subtle)
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap_2()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(self.theme.text_tertiary)
                                        .child("Path:"),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(self.theme.text_primary)
                                        .child(worktree.path.display().to_string()),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap_2()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(self.theme.text_tertiary)
                                        .child("Branch:"),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(self.theme.accent)
                                        .child(worktree.branch.clone()),
                                ),
                        )
                        .when(worktree.is_detached, |this| {
                            this.child(
                                div()
                                    .text_xs()
                                    .text_color(self.theme.text_tertiary)
                                    .child("(detached HEAD)"),
                            )
                        })
                        .when(worktree.is_locked, |this| {
                            this.child(
                                div()
                                    .text_xs()
                                    .text_color(self.theme.text_tertiary)
                                    .child("🔒 Locked"),
                            )
                        }),
                )
            })
    }

    /// Render error state
    fn render_error(&self) -> Div {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .bg(self.theme.bg_primary)
            .items_center()
            .justify_center()
            .gap_4()
            .child(
                div()
                    .text_lg()
                    .text_color(self.theme.text_primary)
                    .child("Error Loading Workspace"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(self.theme.text_secondary)
                    .px_8()
                    .child(
                        self.error_message
                            .as_ref()
                            .unwrap_or(&"Unknown error".to_string())
                            .clone(),
                    ),
            )
    }
}

impl Render for MainWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.error_message.is_some() {
            return div()
                .flex()
                .flex_row()
                .size_full()
                .bg(self.theme.bg_primary)
                .child(self.render_error());
        }

        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(self.theme.bg_primary)
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::handle_dialog_key))
            .child(self.render_sidebar(cx))
            .child(self.render_content())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Tests requiring Context<Self> are skipped as they need GPUI runtime
    // The MainWindow::new and with_error methods require a GPUI context which is not available in unit tests
}
