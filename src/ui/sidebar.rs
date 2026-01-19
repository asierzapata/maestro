use crate::git::{Worktree, worktree};
use crate::ui::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::path::PathBuf;

/// Sidebar component that displays worktrees with selection state
pub struct Sidebar {
    workspace_name: SharedString,
    workspace_path: PathBuf,
    worktrees: Vec<Worktree>,
    selected_index: usize,
    theme: Theme,
    show_create_dialog: bool,
    dialog_branch_name: SharedString,
    dialog_error: Option<SharedString>,
}

impl Sidebar {
    /// Create a new Sidebar instance
    ///
    /// # Arguments
    ///
    /// * `workspace_name` - Name of the workspace to display in header
    /// * `workspace_path` - Path to the workspace root directory
    /// * `worktrees` - List of worktrees to display (root should be first)
    /// * `cx` - GPUI context for creating models
    ///
    /// # Returns
    ///
    /// A new Sidebar with root worktree selected by default
    pub fn new(workspace_name: String, workspace_path: PathBuf, worktrees: Vec<Worktree>) -> Self {
        Self {
            workspace_name: workspace_name.into(),
            workspace_path,
            worktrees,
            selected_index: 0, // Root worktree selected by default
            theme: Theme::new(),
            show_create_dialog: false,
            dialog_branch_name: "".into(),
            dialog_error: None,
        }
    }

    /// Get the currently selected worktree
    pub fn selected_worktree(&self) -> Option<&Worktree> {
        self.worktrees.get(self.selected_index)
    }

    /// Set the selected worktree by index
    pub fn set_selected_index(&mut self, index: usize) {
        if index < self.worktrees.len() {
            self.selected_index = index;
        }
    }

    /// Refresh the list of worktrees from git
    fn refresh_worktrees(&mut self, cx: &mut Context<Self>) {
        match worktree::list_worktrees(&self.workspace_path) {
            Ok(worktrees) => {
                self.worktrees = worktrees;
                // Keep selection valid
                if self.selected_index >= self.worktrees.len() {
                    self.selected_index = 0;
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
                            self.selected_index = idx;
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
    fn handle_create_button_click(&mut self, cx: &mut Context<Self>) {
        self.show_create_dialog = true;
        self.dialog_branch_name = "".into();
        self.dialog_error = None;
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

    /// Render the header with workspace name and settings button
    fn render_header(&self, cx: &mut Context<Self>) -> Div {
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
                cx.listener(|this, _event: &MouseDownEvent, _window, cx| {
                    this.handle_create_button_click(cx);
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
                    // TODO: Implement settings navigation
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

        for (idx, worktree) in self.worktrees.iter().enumerate() {
            container = container.child(self.render_worktree_item(idx, worktree, cx));
        }

        container
    }

    /// Render a single worktree item
    fn render_worktree_item(&self, idx: usize, worktree: &Worktree, cx: &mut Context<Self>) -> Div {
        let is_selected = self.selected_index == idx;
        let is_root = idx == 0; // First worktree is the root

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

    /// Handle worktree item click
    fn handle_worktree_click(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx < self.worktrees.len() {
            self.selected_index = idx;
            println!(
                "Selected worktree: {} (branch: {})",
                self.worktrees[idx].path.display(),
                self.worktrees[idx].branch
            );
            cx.notify();
        }
    }

    /// Render the creation dialog if visible
    fn render_creation_dialog(&self, cx: &mut Context<Self>) -> Option<Div> {
        if !self.show_create_dialog {
            return None;
        }

        Some(
            div()
                .absolute()
                .top_0()
                .left_0()
                .w_full()
                .h_full()
                .child(self.render_dialog_backdrop(cx))
                .child(self.render_dialog_box(cx)),
        )
    }

    fn render_dialog_backdrop(&self, cx: &mut Context<Self>) -> Div {
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
                    this.handle_dialog_cancel(cx);
                }),
            )
    }

    fn render_dialog_box(&self, cx: &mut Context<Self>) -> Div {
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
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(self.theme.text_primary)
                    .child("Create New Worktree"),
            )
            .child(self.render_dialog_input())
            .when_some(self.dialog_error.clone(), |this, error| {
                this.child(self.render_dialog_error(error))
            })
            .child(self.render_dialog_buttons(cx))
    }

    fn render_dialog_input(&self) -> Div {
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
                    .when(self.dialog_branch_name.is_empty(), |this| {
                        this.text_color(self.theme.text_tertiary)
                            .child("feature/my-branch")
                    })
                    .when(!self.dialog_branch_name.is_empty(), |this| {
                        this.child(self.dialog_branch_name.clone())
                    }),
            )
    }

    fn render_dialog_error(&self, error: SharedString) -> Div {
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

    fn render_dialog_buttons(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_row()
            .justify_end()
            .gap_2()
            .child(
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
                            this.handle_dialog_cancel(cx);
                        }),
                    ),
            )
            .child(
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
                            let branch_name = this.dialog_branch_name.to_string();
                            if !branch_name.trim().is_empty() {
                                this.handle_create_worktree(branch_name, window, cx);
                            } else {
                                this.dialog_error = Some("Branch name cannot be empty".into());
                                cx.notify();
                            }
                        }),
                    ),
            )
    }
}

impl Render for Sidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut base = div()
            .flex()
            .flex_col()
            .w(px(280.0))
            .h_full()
            .bg(self.theme.bg_surface)
            .border_r_1()
            .border_color(self.theme.border_subtle)
            .on_key_down(cx.listener(Self::handle_dialog_key))
            .child(self.render_header(cx))
            .child(self.render_worktree_list(cx));

        if let Some(dialog) = self.render_creation_dialog(cx) {
            base = base.child(dialog);
        }

        base
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_sidebar_creation() {
        let worktrees = vec![
            Worktree::new(
                PathBuf::from("/path/to/repo"),
                "main".to_string(),
                false,
                false,
            ),
            Worktree::new(
                PathBuf::from("/path/to/repo/feature-1"),
                "feature-1".to_string(),
                false,
                false,
            ),
        ];

        let sidebar = Sidebar::new("test-workspace".to_string(), worktrees);

        assert_eq!(sidebar.workspace_name.as_ref(), "test-workspace");
        assert_eq!(sidebar.worktrees.len(), 2);
        assert_eq!(sidebar.selected_index, 0); // Root selected by default
    }

    #[test]
    fn test_selected_worktree() {
        let worktrees = vec![
            Worktree::new(
                PathBuf::from("/path/to/repo"),
                "main".to_string(),
                false,
                false,
            ),
            Worktree::new(
                PathBuf::from("/path/to/repo/feature-1"),
                "feature-1".to_string(),
                false,
                false,
            ),
        ];

        let sidebar = Sidebar::new("test-workspace".to_string(), worktrees);

        let selected = sidebar.selected_worktree().unwrap();
        assert_eq!(selected.branch, "main");
    }

    #[test]
    fn test_set_selected_index() {
        let worktrees = vec![
            Worktree::new(
                PathBuf::from("/path/to/repo"),
                "main".to_string(),
                false,
                false,
            ),
            Worktree::new(
                PathBuf::from("/path/to/repo/feature-1"),
                "feature-1".to_string(),
                false,
                false,
            ),
        ];

        let mut sidebar = Sidebar::new("test-workspace".to_string(), worktrees);

        sidebar.set_selected_index(1);
        assert_eq!(sidebar.selected_index, 1);

        let selected = sidebar.selected_worktree().unwrap();
        assert_eq!(selected.branch, "feature-1");

        // Test bounds checking
        sidebar.set_selected_index(10);
        assert_eq!(sidebar.selected_index, 1); // Should not change
    }
}
