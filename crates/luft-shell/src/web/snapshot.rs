use super::{
    icons::icon_data_uri,
    model::{
        WebApplication, WebPanelApp, WebShellSnapshot, WebTrayItem, WebTrayStatus, WebWindow,
        WebWorkspace,
    },
    palette::WebPalette,
};
use crate::{
    apps::{AppEntry, normalize_launch_command},
    ipc::ShellModel,
    panel::{PanelApp, PanelAppState, panel_app_matches_window},
    services::{
        notifications::NotificationSnapshot, system_status::SystemStatus, tray::TraySnapshot,
    },
    theme::ShellPalette,
};
use std::{
    env,
    path::{Path, PathBuf},
};
use time::{OffsetDateTime, macros::format_description};

pub struct WebShellSnapshotInput<'a> {
    pub model: &'a ShellModel,
    pub running_window_order: &'a [luft_ipc::WindowId],
    pub status: &'a SystemStatus,
    pub tray: &'a TraySnapshot,
    pub notifications: &'a NotificationSnapshot,
    pub panel_apps: &'a [PanelApp],
    pub panel_menu_command: Option<&'a str>,
    pub panel_menu_x: Option<i32>,
    pub applications: &'a [AppEntry],
    pub palette: ShellPalette,
    pub start_menu_open: bool,
    pub quick_settings_open: bool,
    pub date_center_open: bool,
}

impl WebShellSnapshot {
    pub fn from_shell(input: WebShellSnapshotInput<'_>) -> Self {
        let WebShellSnapshotInput {
            model,
            running_window_order,
            status,
            tray,
            notifications,
            panel_apps,
            panel_menu_command,
            panel_menu_x,
            applications,
            palette,
            start_menu_open,
            quick_settings_open,
            date_center_open,
        } = input;
        let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
        let windows: Vec<WebWindow> = model.windows.iter().map(WebWindow::from).collect();
        let mut web_panel_apps: Vec<WebPanelApp> = panel_apps
            .iter()
            .map(|app| {
                let state = PanelAppState::for_app(app, model);
                WebPanelApp {
                    label: app.label.clone(),
                    command: app.command.clone(),
                    icon_uri: app.icon_path.as_deref().and_then(icon_data_uri),
                    running: state.running,
                    active: state.active,
                    pinned: true,
                    window_id: primary_window_id(&state.window_ids, state.active_window_id),
                    window_ids: state.window_ids.iter().map(|id| id.0).collect(),
                    active_window_id: state.active_window_id.map(|id| id.0),
                }
            })
            .collect();

        for window_id in running_window_order {
            if let Some(window) = model.windows.iter().find(|window| window.id == *window_id) {
                append_running_window_app(&mut web_panel_apps, window, panel_apps, applications);
            }
        }
        for window in model
            .windows
            .iter()
            .filter(|window| !running_window_order.contains(&window.id))
        {
            append_running_window_app(&mut web_panel_apps, window, panel_apps, applications);
        }

        Self {
            surface: None,
            time: now
                .format(format_description!("[hour]:[minute]"))
                .unwrap_or_else(|_| "--:--".to_string()),
            date: now
                .format(format_description!(
                    "[weekday repr:long], [month repr:long] [day]"
                ))
                .unwrap_or_else(|_| "Today".to_string()),
            active_workspace: model.active_workspace.0.clone(),
            user_profile_icon_uri: user_profile_icon_uri(),
            palette: WebPalette::from(palette),
            workspaces: model
                .workspaces
                .iter()
                .map(|workspace| WebWorkspace {
                    id: workspace.id.0.clone(),
                    name: workspace.name.clone(),
                    active: workspace.id == model.active_workspace,
                })
                .collect(),
            windows,
            panel_apps: web_panel_apps,
            panel_menu_command: panel_menu_command.map(str::to_string),
            panel_menu_x,
            applications: applications
                .iter()
                .map(|app| WebApplication {
                    name: app.name.clone(),
                    command: app.command.clone(),
                    comment: app.comment.clone(),
                    icon: app.icon.clone(),
                    icon_uri: app.icon_path.as_deref().and_then(icon_data_uri),
                    pinned: panel_apps
                        .iter()
                        .any(|panel_app| commands_equal(&panel_app.command, &app.command)),
                })
                .collect(),
            status: status.into(),
            tray: tray
                .items
                .iter()
                .filter_map(|item| {
                    let icon_uri = item.icon_pixmap_uri.clone().or_else(|| {
                        item.icon_name
                            .as_deref()
                            .and_then(|name| crate::apps::resolve_icon_path(Some(name)))
                            .as_deref()
                            .and_then(icon_data_uri)
                    });
                    icon_uri.map(|icon_uri| WebTrayItem {
                        title: item.title.clone(),
                        icon_uri: Some(icon_uri),
                        status: WebTrayStatus::from(item.status),
                    })
                })
                .collect(),
            do_not_disturb: notifications.do_not_disturb,
            notifications: notifications.items.iter().map(Into::into).collect(),
            toast_notifications: notifications.toast_items.iter().map(Into::into).collect(),
            start_menu_open,
            quick_settings_open,
            date_center_open,
        }
    }
}

fn append_running_window_app(
    panel_apps: &mut Vec<WebPanelApp>,
    window: &luft_ipc::WindowSummary,
    pinned_apps: &[PanelApp],
    applications: &[AppEntry],
) {
    if !window_has_identity(window) {
        return;
    }
    if pinned_apps
        .iter()
        .any(|app| panel_app_matches_window(app, window))
    {
        return;
    }
    let matched_app = application_for_window(window, applications);
    let app_id_icon_uri = window
        .app_id
        .as_deref()
        .and_then(|app_id| crate::apps::resolve_icon_path(Some(app_id)))
        .as_deref()
        .and_then(icon_data_uri);
    let icon_uri = matched_app
        .and_then(|app| app.icon_path.as_deref())
        .and_then(icon_data_uri)
        .or(app_id_icon_uri);
    if matched_app.is_none() && icon_uri.is_none() {
        return;
    }
    let label = matched_app
        .map(|app| app.name.clone())
        .or_else(|| window.title.clone())
        .or_else(|| window.app_id.clone())
        .unwrap_or_else(|| "Window".to_string());
    let command = matched_app
        .map(|app| normalize_launch_command(&app.command))
        .or_else(|| window.app_id.as_deref().map(window_group_command))
        .unwrap_or_else(|| format!("window:{}", window.id.0));

    if let Some(app) = panel_apps
        .iter_mut()
        .find(|app| !app.pinned && app.command == command)
    {
        app.running = true;
        app.active |= window.is_active;
        app.window_ids.push(window.id.0);
        if window.is_active {
            app.active_window_id = Some(window.id.0);
            app.window_id = Some(window.id.0);
        } else if app.window_id.is_none() {
            app.window_id = Some(window.id.0);
        }
        if app.icon_uri.is_none() {
            app.icon_uri = icon_uri;
        }
        return;
    }

    panel_apps.push(WebPanelApp {
        label,
        command,
        icon_uri,
        running: true,
        active: window.is_active,
        pinned: false,
        window_id: Some(window.id.0),
        window_ids: vec![window.id.0],
        active_window_id: window.is_active.then_some(window.id.0),
    });
}

fn primary_window_id(
    window_ids: &[luft_ipc::WindowId],
    active_window_id: Option<luft_ipc::WindowId>,
) -> Option<u64> {
    active_window_id
        .or_else(|| window_ids.first().copied())
        .map(|id| id.0)
}

fn user_profile_icon_uri() -> Option<String> {
    let user = env::var("USER").ok()?;
    let home = env::var_os("HOME").map(PathBuf::from);
    [
        Some(PathBuf::from(format!(
            "/var/lib/AccountsService/icons/{user}"
        ))),
        home.as_ref().map(|path| path.join(".face")),
        home.as_ref().map(|path| path.join(".face.icon")),
    ]
    .into_iter()
    .flatten()
    .find_map(|path| icon_data_uri(path.as_path()))
}

fn commands_equal(left: &str, right: &str) -> bool {
    normalize_launch_command(left) == normalize_launch_command(right)
}

fn window_has_identity(window: &luft_ipc::WindowSummary) -> bool {
    window
        .title
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
        || window
            .app_id
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
}

fn application_for_window<'a>(
    window: &luft_ipc::WindowSummary,
    applications: &'a [AppEntry],
) -> Option<&'a AppEntry> {
    window
        .app_id
        .as_deref()
        .and_then(|app_id| {
            applications
                .iter()
                .find(|app| app_matches_app_id(app, app_id))
        })
        .or_else(|| {
            window.title.as_deref().and_then(|title| {
                applications
                    .iter()
                    .find(|app| app_matches_window_title(app, title))
            })
        })
}

fn app_matches_app_id(app: &AppEntry, identifier: &str) -> bool {
    let identifier = normalized_identifier(identifier);
    if identifier.is_empty() {
        return false;
    }

    app_identity_candidates(app).any(|candidate| {
        candidate == identifier
            || (candidate.len() >= 4 && identifier.contains(&candidate))
            || (identifier.len() >= 4 && candidate.contains(&identifier))
    })
}

fn app_matches_window_title(app: &AppEntry, title: &str) -> bool {
    let title = normalized_identifier(title);
    if title.is_empty() {
        return false;
    }

    app_identity_candidates(app)
        .filter(|candidate| candidate.len() >= 4)
        .any(|candidate| title.contains(&candidate))
}

fn app_identity_candidates(app: &AppEntry) -> impl Iterator<Item = String> + '_ {
    [
        app.desktop_id.as_deref(),
        app.startup_wm_class.as_deref(),
        app.icon.as_deref(),
        Some(app.name.as_str()),
        command_name(&app.command),
    ]
    .into_iter()
    .flatten()
    .map(normalized_identifier)
    .filter(|candidate| !candidate.is_empty())
}

fn normalized_identifier(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn window_group_command(value: &str) -> String {
    format!("window-group:{}", normalized_identifier(value))
}

fn command_name(command: &str) -> Option<&str> {
    let first = command
        .split_whitespace()
        .next()?
        .trim_matches('"')
        .trim_matches('\'');
    Path::new(first).file_name()?.to_str()
}
