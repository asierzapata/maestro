#![recursion_limit = "256"]

use gpui::*;

mod git;
mod settings;
mod terminal;
mod ui;
mod workspace;

use ui::WorkspaceSelector;

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.open_window(WindowOptions::default(), |_window, cx| {
            cx.new(|_cx| WorkspaceSelector::new())
        })
        .unwrap();
    });
}
