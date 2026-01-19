use crate::ui::main_window::MainWindow;
use crate::ui::theme::Theme;
use crate::workspace::WorkspaceManager;
use gpui::*;
use rfd::FileDialog;

/// State for the workspace selector view
pub struct WorkspaceSelector {
    manager: WorkspaceManager,
    selected_index: Option<usize>,
    theme: Theme,
}

impl WorkspaceSelector {
    pub fn new() -> Self {
        let manager = WorkspaceManager::new().unwrap_or_default();
        Self {
            manager,
            selected_index: None,
            theme: Theme::new(),
        }
    }

    fn render_header(&self) -> Div {
        div().flex().flex_col().gap_2().items_center().child(
            div()
                .text_sm()
                .text_color(self.theme.text_secondary)
                .child("Choose a git repository to open"),
        )
    }

    fn render_browse_button(&self, cx: &mut Context<Self>) -> Div {
        div()
            .px_6()
            .py_3()
            .bg(self.theme.accent)
            .rounded_md()
            .cursor_pointer()
            .hover(|style| style.bg(self.theme.accent_hover))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _event, _window, cx| {
                    this.handle_browse_click(cx);
                }),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(self.theme.text_primary)
                    .child("Browse for Repository"),
            )
    }

    fn render_empty_state(&self) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .items_center()
            .py_12()
            .child(
                div()
                    .text_lg()
                    .text_color(self.theme.text_secondary)
                    .child("No recent workspaces"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(self.theme.text_tertiary)
                    .child("Browse for a git repository to get started"),
            )
    }

    fn render_workspace_list(&self, cx: &mut Context<Self>) -> Div {
        let workspaces = self.manager.load_recent_workspaces();

        if workspaces.is_empty() {
            return self.render_empty_state();
        }

        let mut container = div()
            .flex()
            .flex_col()
            .gap_2()
            .w_full()
            .max_w(px(600.0))
            .child(
                div()
                    .text_xs()
                    .text_color(self.theme.text_tertiary)
                    .mb_2()
                    .child("RECENT WORKSPACES"),
            );

        for (idx, workspace) in workspaces.iter().enumerate() {
            let is_selected = self.selected_index == Some(idx);
            let bg_color = if is_selected {
                self.theme.bg_selected
            } else {
                self.theme.bg_surface
            };

            container = container.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px_4()
                    .py_3()
                    .bg(bg_color)
                    .rounded_md()
                    .border_1()
                    .border_color(self.theme.border_subtle)
                    .cursor_pointer()
                    .hover(|style| style.bg(self.theme.bg_hover))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _event, window, cx| {
                            this.handle_workspace_click(idx, window, cx);
                        }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(self.theme.text_primary)
                                    .child(workspace.name.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(self.theme.text_secondary)
                                    .child(workspace.path.display().to_string()),
                            ),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_color(self.theme.text_tertiary)
                            .hover(|style| {
                                style
                                    .bg(hsla(0.0, 0.6, 0.45, 0.3))
                                    .text_color(hsla(0.0, 0.6, 0.50, 1.0))
                            })
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _event: &MouseDownEvent, _window, cx| {
                                    cx.stop_propagation();
                                    this.handle_remove_workspace(idx, cx);
                                }),
                            )
                            .child("×"),
                    ),
            );
        }

        container
    }

    fn handle_workspace_click(&mut self, idx: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_index = Some(idx);
        let workspaces = self.manager.load_recent_workspaces();

        if let Some(workspace) = workspaces.get(idx) {
            println!(
                "Selected workspace: {} at {:?}",
                workspace.name, workspace.path
            );

            // Update last opened timestamp
            if let Err(e) = self.manager.update_last_opened(&workspace.path) {
                eprintln!("Failed to update last opened timestamp: {}", e);
            }

            // Replace the current view with MainWindow
            let workspace_path = workspace.path.clone();
            window.replace_root(cx, |_window, cx| {
                match MainWindow::new(workspace_path.clone(), cx) {
                    Ok(main_window) => main_window,
                    Err(e) => {
                        eprintln!("Failed to open workspace: {}", e);
                        MainWindow::with_error(workspace_path, e, cx)
                    }
                }
            });
        }
    }

    fn handle_remove_workspace(&mut self, idx: usize, cx: &mut Context<Self>) {
        let workspaces = self.manager.load_recent_workspaces();

        if let Some(workspace) = workspaces.get(idx) {
            println!("Removing workspace: {}", workspace.name);
            match self.manager.remove_workspace(&workspace.path) {
                Ok(_) => {
                    // Adjust selected_index if necessary
                    if let Some(selected) = self.selected_index {
                        if selected >= idx {
                            self.selected_index = if selected > 0 {
                                Some(selected - 1)
                            } else {
                                None
                            };
                        }
                    }
                    cx.notify();
                }
                Err(e) => {
                    eprintln!("Failed to remove workspace: {}", e);
                }
            }
        }
    }

    fn handle_browse_click(&mut self, cx: &mut Context<Self>) {
        // Open native folder picker
        if let Some(folder) = FileDialog::new()
            .set_title("Select Git Repository")
            .pick_folder()
        {
            // Validate it's a git repository and add to recent workspaces
            match self.manager.add_workspace(folder) {
                Ok(_) => {
                    println!("Successfully added workspace");
                    cx.notify(); // Trigger re-render to show new workspace
                }
                Err(e) => {
                    eprintln!("Failed to add workspace: {}", e);
                    // TODO: Show error message to user
                }
            }
        }
    }

    fn handle_keyboard_input(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let workspaces = self.manager.load_recent_workspaces();
        if workspaces.is_empty() {
            return;
        }

        match event.keystroke.key.as_str() {
            "ArrowDown" | "down" => {
                // Move selection down
                let current = self.selected_index.unwrap_or(0);
                let next = if current + 1 < workspaces.len() {
                    current + 1
                } else {
                    current
                };
                self.selected_index = Some(next);
                cx.notify();
            }
            "ArrowUp" | "up" => {
                // Move selection up
                let current = self.selected_index.unwrap_or(0);
                let next = if current > 0 { current - 1 } else { 0 };
                self.selected_index = Some(next);
                cx.notify();
            }
            "Enter" | "return" => {
                // Open selected workspace
                if let Some(idx) = self.selected_index {
                    if let Some(workspace) = workspaces.get(idx) {
                        println!(
                            "Opening workspace: {} at {:?}",
                            workspace.name, workspace.path
                        );

                        // Update last opened timestamp
                        if let Err(e) = self.manager.update_last_opened(&workspace.path) {
                            eprintln!("Failed to update last opened timestamp: {}", e);
                        }

                        // Replace the current view with MainWindow
                        let workspace_path = workspace.path.clone();
                        window.replace_root(cx, |_window, cx| {
                            match MainWindow::new(workspace_path.clone(), cx) {
                                Ok(main_window) => main_window,
                                Err(e) => {
                                    eprintln!("Failed to open workspace: {}", e);
                                    MainWindow::with_error(workspace_path, e, cx)
                                }
                            }
                        });
                    }
                }
            }
            _ => {}
        }
    }
}

impl Render for WorkspaceSelector {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .size_full()
            .bg(self.theme.bg_primary)
            .gap_8()
            .on_key_down(cx.listener(|this, event, window, cx| {
                this.handle_keyboard_input(event, window, cx);
            }))
            .child(self.render_header())
            .child(self.render_workspace_list(cx))
            .child(self.render_browse_button(cx))
    }
}
