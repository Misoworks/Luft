use crate::ipc::ShellModel;
use luft_ipc::WindowSummary;
use std::path::Path;

use super::PanelApp;

#[derive(Debug, Clone)]
pub struct PanelAppState {
    pub running: bool,
    pub active: bool,
    pub window_ids: Vec<luft_ipc::WindowId>,
    pub active_window_id: Option<luft_ipc::WindowId>,
}

impl PanelAppState {
    pub fn for_app(app: &PanelApp, model: &ShellModel) -> Self {
        let mut window_ids = Vec::new();
        let mut active_window_id = None;
        for window in &model.windows {
            if panel_app_matches_window(app, window) {
                window_ids.push(window.id);
                if window.is_active {
                    active_window_id = Some(window.id);
                }
            }
        }

        Self {
            running: !window_ids.is_empty(),
            active: active_window_id.is_some(),
            window_ids,
            active_window_id,
        }
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
    let value = normalized_identifier(value);
    if value.is_empty() {
        return false;
    }
    [command, label]
        .into_iter()
        .map(normalized_identifier)
        .filter(|candidate| !candidate.is_empty())
        .any(|candidate| {
            value == candidate
                || (candidate.len() >= 4 && value.contains(&candidate))
                || (value.len() >= 4 && candidate.contains(&value))
        })
}

fn normalized_identifier(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn command_name(command: &str) -> Option<&str> {
    let first = command
        .split_whitespace()
        .next()?
        .trim_matches('"')
        .trim_matches('\'');
    Path::new(first).file_name()?.to_str()
}
