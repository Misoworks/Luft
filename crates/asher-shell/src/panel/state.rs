use crate::ipc::ShellModel;
use asher_ipc::WindowSummary;
use std::path::Path;

use super::PanelApp;

#[derive(Debug, Clone, Copy)]
pub struct PanelAppState {
    pub running: bool,
    pub active: bool,
}

impl PanelAppState {
    pub fn for_app(app: &PanelApp, model: &ShellModel) -> Self {
        let mut running = false;
        let mut active = false;
        for window in &model.windows {
            if panel_app_matches_window(app, window) {
                running = true;
                active |= window.is_active;
            }
        }

        Self { running, active }
    }
}

pub fn panel_app_matches_window(app: &PanelApp, window: &WindowSummary) -> bool {
    let label = app.label.to_lowercase();
    let command = command_name(&app.command).unwrap_or("").to_lowercase();

    window
        .app_id
        .as_deref()
        .is_some_and(|app_id| identifier_matches(app_id, &command, &label))
        || window
            .title
            .as_deref()
            .is_some_and(|title| identifier_matches(title, &command, &label))
}

fn identifier_matches(value: &str, command: &str, label: &str) -> bool {
    let value = value.to_lowercase();
    (!command.is_empty() && value.contains(command)) || (!label.is_empty() && value.contains(label))
}

fn command_name(command: &str) -> Option<&str> {
    let first = command
        .split_whitespace()
        .next()?
        .trim_matches('"')
        .trim_matches('\'');
    Path::new(first).file_name()?.to_str()
}
