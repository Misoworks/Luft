use super::PanelApp;
use crate::apps::normalize_launch_command;
use asher_config::{AsherConfig, PinnedAppConfig};

pub fn pin_app(
    config: &mut AsherConfig,
    current: &[PanelApp],
    label: String,
    command: String,
    icon: Option<String>,
) -> bool {
    let command = normalize_launch_command(&command);
    if command.is_empty() {
        return false;
    }

    materialize(config, current);
    let pin = PinnedAppConfig {
        label: clean_label(label, &command),
        command,
        icon: clean_icon(icon),
    };

    match config
        .panel
        .pinned
        .iter()
        .position(|app| commands_equal(&app.command, &pin.command))
    {
        Some(index) => config.panel.pinned[index] = pin,
        None => config.panel.pinned.push(pin),
    }

    true
}

pub fn unpin_app(config: &mut AsherConfig, current: &[PanelApp], command: &str) -> bool {
    materialize(config, current);
    let before = config.panel.pinned.len();
    config
        .panel
        .pinned
        .retain(|pin| !commands_equal(&pin.command, command));
    before != config.panel.pinned.len()
}

pub fn reorder_apps(config: &mut AsherConfig, current: &[PanelApp], commands: Vec<String>) -> bool {
    if commands.is_empty() {
        return false;
    }
    materialize(config, current);

    let before = config.panel.pinned.clone();
    let mut remaining = before.clone();
    let mut next = Vec::new();
    for command in commands {
        if let Some(index) = remaining
            .iter()
            .position(|pin| commands_equal(&pin.command, &command))
        {
            next.push(remaining.remove(index));
        }
    }
    next.extend(remaining);
    if next == before {
        return false;
    }
    config.panel.pinned = next;
    true
}

fn materialize(config: &mut AsherConfig, current: &[PanelApp]) {
    if config.panel.customized || !config.panel.pinned.is_empty() {
        config.panel.customized = true;
        return;
    }

    config.panel.pinned = current
        .iter()
        .map(|app| PinnedAppConfig {
            label: app.label.clone(),
            command: app.command.clone(),
            icon: app
                .icon_path
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
        })
        .collect();
    config.panel.customized = true;
}

fn clean_label(label: String, command: &str) -> String {
    let label = label.trim();
    if label.is_empty() {
        command
            .split_whitespace()
            .next()
            .unwrap_or(command)
            .to_string()
    } else {
        label.to_string()
    }
}

fn clean_icon(icon: Option<String>) -> Option<String> {
    icon.map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn commands_equal(left: &str, right: &str) -> bool {
    normalize_launch_command(left) == normalize_launch_command(right)
}
