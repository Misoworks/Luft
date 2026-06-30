use super::model::{WebPanelApp, WebShellSnapshot, WebWindow};
use crate::apps::normalize_launch_command;

pub(crate) const QUICK_SETTINGS_WIDTH: i32 = 420;
pub(crate) const NOTIFICATION_TOAST_WIDTH: i32 = 380;
pub(crate) const NOTIFICATION_TOAST_BASE_HEIGHT: i32 = 136;
pub(crate) const NOTIFICATION_TOAST_ACTION_HEIGHT: i32 = 171;
pub(crate) const DOCK_MENU_WIDTH: i32 = 184;

pub(crate) fn quick_settings_size(snapshot: &WebShellSnapshot) -> (i32, i32) {
    let status_tiles = 1
        + i32::from(snapshot.status.network.is_some())
        + i32::from(snapshot.status.battery.is_some());
    let status_rows = (status_tiles + 1) / 2;
    let sliders = i32::from(snapshot.status.audio.is_some())
        + i32::from(snapshot.status.brightness.is_some());
    let mut height = 32 + 36 + 13 + status_rows * 58 + (status_rows - 1) * 10;
    if sliders > 0 {
        height += 13 + sliders * 58 + (sliders - 1) * 10;
    }
    (QUICK_SETTINGS_WIDTH, height)
}

pub(crate) fn notification_toast_size(snapshot: &WebShellSnapshot) -> (i32, i32) {
    let height = snapshot
        .toast_notifications
        .first()
        .filter(|notification| {
            notification
                .actions
                .iter()
                .any(|action| action.key != "default")
        })
        .map(|_| NOTIFICATION_TOAST_ACTION_HEIGHT)
        .unwrap_or(NOTIFICATION_TOAST_BASE_HEIGHT);
    (NOTIFICATION_TOAST_WIDTH, height)
}

pub(crate) fn panel_menu_size(snapshot: &WebShellSnapshot) -> (i32, i32) {
    let Some(command) = &snapshot.panel_menu_command else {
        return (DOCK_MENU_WIDTH, 128);
    };
    let Some(app) = snapshot.panel_apps.iter().find(|entry| {
        normalize_launch_command(&entry.command) == normalize_launch_command(command)
    }) else {
        return (DOCK_MENU_WIDTH, 128);
    };
    let window = matched_window(app, &snapshot.windows);
    let action_count = if let Some(window) = window {
        let focus = i32::from(!window.active);
        let open_new = i32::from(app.pinned);
        focus + open_new + 3
    } else if app.running {
        3
    } else {
        2
    };
    (DOCK_MENU_WIDTH, panel_menu_height(action_count))
}

fn panel_menu_height(action_count: i32) -> i32 {
    let content_items = action_count + 1;
    let content_height = 16 + 23 + action_count * 34 + (content_items - 1) * 4;
    content_height.clamp(128, 264)
}

fn matched_window<'a>(app: &WebPanelApp, windows: &'a [WebWindow]) -> Option<&'a WebWindow> {
    windows
        .iter()
        .find(|window| window.active && window.visible && window_matches_app(window, app))
        .or_else(|| {
            windows
                .iter()
                .find(|window| window.visible && window_matches_app(window, app))
        })
        .or_else(|| {
            windows
                .iter()
                .find(|window| window_matches_app(window, app))
        })
}

fn window_matches_app(window: &WebWindow, app: &WebPanelApp) -> bool {
    if app.window_id.is_some_and(|id| id == window.id) {
        return true;
    }
    let command = command_name(&app.command);
    let label = app.label.to_lowercase();
    [window.app_id.as_deref(), Some(window.title.as_str())]
        .into_iter()
        .flatten()
        .map(str::to_lowercase)
        .any(|text| {
            !text.is_empty()
                && ((!command.is_empty() && text.contains(&command))
                    || (!label.is_empty() && text.contains(&label)))
        })
}

fn command_name(command: &str) -> String {
    command
        .split_whitespace()
        .next()
        .and_then(|value| value.rsplit('/').next())
        .unwrap_or_default()
        .trim_matches(['\'', '"'])
        .to_lowercase()
}
