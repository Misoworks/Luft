use super::{
    appearance::WebAppearance,
    icons::icon_data_uri,
    model::{
        WebApplication, WebDockApp, WebProfile, WebShellSnapshot, WebTrayItem, WebTrayStatus,
        WebWindow, WebWorkspace,
    },
    palette::WebPalette,
};
use crate::{
    apps::{AppEntry, normalize_launch_command},
    dock::{DockApp, DockAppState, dock_app_matches_window},
    ipc::ShellModel,
    services::{
        notifications::NotificationSnapshot, system_status::SystemStatus, tray::TraySnapshot,
    },
    theme::ShellPalette,
};
use asher_config::AsherConfig;
use std::{env, path::PathBuf};
use time::{OffsetDateTime, macros::format_description};

pub struct WebShellSnapshotInput<'a> {
    pub model: &'a ShellModel,
    pub status: &'a SystemStatus,
    pub tray: &'a TraySnapshot,
    pub notifications: &'a NotificationSnapshot,
    pub dock_apps: &'a [DockApp],
    pub dock_menu_command: Option<&'a str>,
    pub dock_menu_x: Option<i32>,
    pub applications: &'a [AppEntry],
    pub wallpaper_uri: Option<String>,
    pub glass_blur_wallpaper_uri: Option<String>,
    pub palette: ShellPalette,
    pub config: &'a AsherConfig,
    pub safe_mode: bool,
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
            dock_apps,
            dock_menu_command,
            dock_menu_x,
            applications,
            wallpaper_uri,
            glass_blur_wallpaper_uri,
            palette,
            config,
            safe_mode,
            start_menu_open,
            quick_settings_open,
            date_center_open,
        } = input;
        let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
        let windows: Vec<WebWindow> = model.windows.iter().map(WebWindow::from).collect();
        let mut web_dock_apps: Vec<WebDockApp> = dock_apps
            .iter()
            .map(|app| {
                let state = DockAppState::for_app(app, model);
                WebDockApp {
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
            if dock_apps
                .iter()
                .any(|app| dock_app_matches_window(app, window))
            {
                continue;
            }
            let label = window
                .title
                .clone()
                .or_else(|| window.app_id.clone())
                .unwrap_or_else(|| "Window".to_string());
            let icon_uri = window
                .app_id
                .as_deref()
                .and_then(|app_id| crate::apps::resolve_icon_path(Some(app_id)))
                .as_deref()
                .and_then(icon_data_uri);
            web_dock_apps.push(WebDockApp {
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
            panel_taskbar: model.active_mode == asher_layout::ModeId::Panel,
            blur_enabled: model.blur_enabled,
            debug_overlay: model.debug_overlay,
            safe_mode,
            wallpaper_uri,
            glass_blur_wallpaper_uri,
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
            dock_apps: web_dock_apps,
            dock_menu_command: dock_menu_command.map(str::to_string),
            dock_menu_x,
            applications: applications
                .iter()
                .map(|app| WebApplication {
                    name: app.name.clone(),
                    command: app.command.clone(),
                    comment: app.comment.clone(),
                    icon: app.icon.clone(),
                    icon_uri: app.icon_path.as_deref().and_then(icon_data_uri),
                    pinned: dock_apps
                        .iter()
                        .any(|dock_app| commands_equal(&dock_app.command, &app.command)),
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

fn mode_name(mode: asher_layout::ModeId) -> String {
    match mode {
        asher_layout::ModeId::Classic => "classic",
        asher_layout::ModeId::Dock => "dock",
        asher_layout::ModeId::Panel => "panel",
        asher_layout::ModeId::Tiling => "tiling",
        asher_layout::ModeId::Browser => "browser",
        asher_layout::ModeId::Focus => "focus",
        asher_layout::ModeId::Tablet => "tablet",
    }
    .to_string()
}

fn commands_equal(left: &str, right: &str) -> bool {
    normalize_launch_command(left) == normalize_launch_command(right)
}
