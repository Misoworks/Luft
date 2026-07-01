use super::{
    appearance::WebAppearance,
    icons::icon_data_uri,
    model::{
        WebApplication, WebPanelApp, WebProfile, WebShellSnapshot, WebTrayItem, WebTrayStatus,
        WebWindow, WebWorkspace,
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
use asher_config::AsherConfig;
use std::{
    env,
    path::{Path, PathBuf},
};
use time::{OffsetDateTime, macros::format_description};

pub struct WebShellSnapshotInput<'a> {
    pub model: &'a ShellModel,
    pub status: &'a SystemStatus,
    pub tray: &'a TraySnapshot,
    pub notifications: &'a NotificationSnapshot,
    pub panel_apps: &'a [PanelApp],
    pub panel_menu_command: Option<&'a str>,
    pub panel_menu_x: Option<i32>,
    pub applications: &'a [AppEntry],
    pub palette: ShellPalette,
    pub config: &'a AsherConfig,
    pub start_menu_open: bool,
    pub quick_settings_open: bool,
    pub date_center_open: bool,
}

impl WebShellSnapshot {
    pub fn from_shell(input: WebShellSnapshotInput<'_>) -> Self {
        let WebShellSnapshotInput {
            model,
            status,
            tray,
            notifications,
            panel_apps,
            panel_menu_command,
            panel_menu_x,
            applications,
            palette,
            config,
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
                    window_id: None,
                }
            })
            .collect();

        for window in &model.windows {
            if !window_has_identity(window) {
                continue;
            }
            if panel_apps
                .iter()
                .any(|app| panel_app_matches_window(app, window))
            {
                continue;
            }
            let matched_app = application_for_window(window, applications);
            let app_id_icon_uri = window
                .app_id
                .as_deref()
                .and_then(|app_id| crate::apps::resolve_icon_path(Some(app_id)))
                .as_deref()
                .and_then(icon_data_uri);
            let label = window
                .title
                .clone()
                .or_else(|| matched_app.map(|app| app.name.clone()))
                .or_else(|| window.app_id.clone())
                .unwrap_or_else(|| "Window".to_string());
            let icon_uri = matched_app
                .and_then(|app| app.icon_path.as_deref())
                .and_then(icon_data_uri)
                .or(app_id_icon_uri);
            if matched_app.is_none() && icon_uri.is_none() {
                continue;
            }
            web_panel_apps.push(WebPanelApp {
                label,
                command: format!("window:{}", window.id.0),
                icon_uri,
                running: true,
                active: window.is_active,
                pinned: false,
                window_id: Some(window.id.0),
            });
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
            active_profile: model.active_profile.0.clone(),
            active_mode: mode_name(model.active_mode),
            blur_enabled: model.blur_enabled,
            debug_overlay: model.debug_overlay,
            user_profile_icon_uri: user_profile_icon_uri(),
            palette: WebPalette::from(palette),
            appearance: WebAppearance::from_config(config),
            profiles: model
                .profiles
                .iter()
                .map(|profile| WebProfile {
                    id: profile.id.0.clone(),
                    name: profile.name.clone(),
                    mode: mode_name(profile.mode),
                    active: profile.id == model.active_profile,
                })
                .collect(),
            workspaces: model
                .workspaces
                .iter()
                .map(|workspace| WebWorkspace {
                    id: workspace.id.0.clone(),
                    name: workspace.name.clone(),
                    profile: workspace.profile.0.clone(),
                    mode: mode_name(workspace.mode),
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
                .map(|item| WebTrayItem {
                    title: item.title.clone(),
                    icon_uri: item
                        .icon_name
                        .as_deref()
                        .and_then(|name| crate::apps::resolve_icon_path(Some(name)))
                        .as_deref()
                        .and_then(icon_data_uri),
                    status: WebTrayStatus::from(item.status),
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

fn mode_name(mode: asher_ipc::ModeId) -> String {
    match mode {
        asher_ipc::ModeId::Panel => "panel",
    }
    .to_string()
}

fn commands_equal(left: &str, right: &str) -> bool {
    normalize_launch_command(left) == normalize_launch_command(right)
}

fn window_has_identity(window: &asher_ipc::WindowSummary) -> bool {
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
    window: &asher_ipc::WindowSummary,
    applications: &'a [AppEntry],
) -> Option<&'a AppEntry> {
    applications.iter().find(|app| {
        [window.app_id.as_deref(), window.title.as_deref()]
            .into_iter()
            .flatten()
            .any(|identifier| app_matches_window_identifier(app, identifier))
    })
}

fn app_matches_window_identifier(app: &AppEntry, identifier: &str) -> bool {
    let identifier = normalized_identifier(identifier);
    if identifier.is_empty() {
        return false;
    }

    [
        app.startup_wm_class.as_deref(),
        app.icon.as_deref(),
        Some(app.name.as_str()),
        command_name(&app.command),
    ]
    .into_iter()
    .flatten()
    .map(normalized_identifier)
    .filter(|candidate| !candidate.is_empty())
    .any(|candidate| identifier.contains(&candidate) || candidate.contains(&identifier))
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
