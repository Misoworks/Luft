use super::{appearance::WebAppearance, icons::icon_data_uri, palette::WebPalette};
use crate::{
    apps::AppEntry,
    dock::{DockApp, DockAppState},
    ipc::ShellModel,
    services::{
        notifications::{NotificationItem, NotificationSnapshot, NotificationUrgency},
        system_status::{AudioInfo, BatteryInfo, BrightnessInfo, NetworkInfo, SystemStatus},
        tray::{TrayItemStatus, TraySnapshot},
    },
    theme::ShellPalette,
};
use asher_config::AsherConfig;
use serde::Serialize;
use std::{env, path::PathBuf};
use time::{OffsetDateTime, macros::format_description};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebShellSnapshot {
    pub surface: Option<WebShellSurface>,
    pub time: String,
    pub date: String,
    pub active_workspace: String,
    pub active_profile: String,
    pub active_mode: String,
    pub panel_taskbar: bool,
    pub blur_enabled: bool,
    pub debug_overlay: bool,
    pub safe_mode: bool,
    pub wallpaper_uri: Option<String>,
    pub glass_blur_wallpaper_uri: Option<String>,
    pub user_profile_icon_uri: Option<String>,
    pub palette: WebPalette,
    pub appearance: WebAppearance,
    pub profiles: Vec<WebProfile>,
    pub workspaces: Vec<WebWorkspace>,
    pub windows: Vec<WebWindow>,
    pub dock_apps: Vec<WebDockApp>,
    pub dock_menu_command: Option<String>,
    pub dock_menu_x: Option<i32>,
    pub applications: Vec<WebApplication>,
    pub status: WebSystemStatus,
    pub tray: Vec<WebTrayItem>,
    pub do_not_disturb: bool,
    pub notifications: Vec<WebNotification>,
    pub toast_notifications: Vec<WebNotification>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WebShellSurface {
    Panel,
    Dock,
    DockMenu,
    Sidebar,
    QuickSettings,
    DateCenter,
    NotificationToast,
    StartMenu,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebWorkspace {
    pub id: String,
    pub name: String,
    pub profile: String,
    pub mode: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebProfile {
    pub id: String,
    pub name: String,
    pub mode: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebWindow {
    pub id: u64,
    pub title: String,
    pub app_id: Option<String>,
    pub icon_uri: Option<String>,
    pub workspace: String,
    pub geometry: WebGeometry,
    pub active: bool,
    pub visible: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebDockApp {
    pub label: String,
    pub command: String,
    pub icon_uri: Option<String>,
    pub running: bool,
    pub active: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebApplication {
    pub name: String,
    pub command: String,
    pub comment: Option<String>,
    pub icon: Option<String>,
    pub icon_uri: Option<String>,
    pub pinned: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebGeometry {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSystemStatus {
    pub battery: Option<WebBattery>,
    pub network: Option<WebNetwork>,
    pub audio: Option<WebAudio>,
    pub brightness: Option<WebBrightness>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebBattery {
    pub percent: u8,
    pub state: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebNetwork {
    pub name: String,
    pub wireless: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebAudio {
    pub percent: u8,
    pub muted: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebBrightness {
    pub percent: u8,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebTrayItem {
    pub title: String,
    pub icon_uri: Option<String>,
    pub status: WebTrayStatus,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WebTrayStatus {
    Passive,
    Active,
    NeedsAttention,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebNotification {
    pub id: u32,
    pub app_name: String,
    pub icon_uri: Option<String>,
    pub received_at: u64,
    pub summary: String,
    pub body: String,
    pub urgency: WebNotificationUrgency,
    pub actions: Vec<WebNotificationAction>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebNotificationAction {
    pub key: String,
    pub label: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WebNotificationUrgency {
    Low,
    Normal,
    Critical,
}

impl WebShellSnapshot {
    pub fn from_shell(
        model: &ShellModel,
        status: &SystemStatus,
        tray: &TraySnapshot,
        notifications: &NotificationSnapshot,
        dock_apps: &[DockApp],
        dock_menu_command: Option<&str>,
        dock_menu_x: Option<i32>,
        applications: &[AppEntry],
        wallpaper_uri: Option<String>,
        glass_blur_wallpaper_uri: Option<String>,
        palette: ShellPalette,
        config: &AsherConfig,
        safe_mode: bool,
    ) -> Self {
        let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
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
            windows: model
                .windows
                .iter()
                .map(|window| WebWindow {
                    id: window.id.0,
                    title: window
                        .title
                        .clone()
                        .or_else(|| window.app_id.clone())
                        .unwrap_or_else(|| "Window".to_string()),
                    app_id: window.app_id.clone(),
                    icon_uri: window
                        .app_id
                        .as_deref()
                        .and_then(|app_id| crate::apps::resolve_icon_path(Some(app_id)))
                        .as_deref()
                        .and_then(icon_data_uri),
                    workspace: window.workspace.0.clone(),
                    geometry: WebGeometry {
                        x: window.geometry.x,
                        y: window.geometry.y,
                        width: window.geometry.width,
                        height: window.geometry.height,
                    },
                    active: window.is_active,
                    visible: window.is_visible,
                })
                .collect(),
            dock_apps: dock_apps
                .iter()
                .map(|app| {
                    let state = DockAppState::for_app(app, model);
                    WebDockApp {
                        label: app.label.clone(),
                        command: app.command.clone(),
                        icon_uri: app.icon_path.as_deref().and_then(icon_data_uri),
                        running: state.running,
                        active: state.active,
                    }
                })
                .collect(),
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
            status: WebSystemStatus::from(status),
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
            notifications: notifications
                .items
                .iter()
                .map(WebNotification::from)
                .collect(),
            toast_notifications: notifications
                .toast_items
                .iter()
                .map(WebNotification::from)
                .collect(),
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

impl From<&NotificationItem> for WebNotification {
    fn from(item: &NotificationItem) -> Self {
        Self {
            id: item.id,
            app_name: item.app_name.clone(),
            icon_uri: notification_icon_uri(item),
            received_at: item.received_at,
            summary: item.summary.clone(),
            body: item.body.clone(),
            urgency: WebNotificationUrgency::from(item.urgency),
            actions: item
                .actions
                .iter()
                .map(|action| WebNotificationAction {
                    key: action.key.clone(),
                    label: action.label.clone(),
                })
                .collect(),
        }
    }
}

impl From<&SystemStatus> for WebSystemStatus {
    fn from(value: &SystemStatus) -> Self {
        Self {
            battery: value.battery.as_ref().map(WebBattery::from),
            network: value.network.as_ref().map(WebNetwork::from),
            audio: value.audio.as_ref().map(WebAudio::from),
            brightness: value.brightness.as_ref().map(WebBrightness::from),
        }
    }
}

impl From<&BatteryInfo> for WebBattery {
    fn from(value: &BatteryInfo) -> Self {
        Self {
            percent: value.percent,
            state: value.state.clone(),
        }
    }
}

impl From<&NetworkInfo> for WebNetwork {
    fn from(value: &NetworkInfo) -> Self {
        Self {
            name: value.name.clone(),
            wireless: value.wireless,
        }
    }
}

impl From<&AudioInfo> for WebAudio {
    fn from(value: &AudioInfo) -> Self {
        Self {
            percent: value.percent,
            muted: value.muted,
        }
    }
}

impl From<&BrightnessInfo> for WebBrightness {
    fn from(value: &BrightnessInfo) -> Self {
        Self {
            percent: value.percent,
        }
    }
}

impl From<TrayItemStatus> for WebTrayStatus {
    fn from(value: TrayItemStatus) -> Self {
        match value {
            TrayItemStatus::Passive => Self::Passive,
            TrayItemStatus::Active => Self::Active,
            TrayItemStatus::NeedsAttention => Self::NeedsAttention,
        }
    }
}

impl From<NotificationUrgency> for WebNotificationUrgency {
    fn from(value: NotificationUrgency) -> Self {
        match value {
            NotificationUrgency::Low => Self::Low,
            NotificationUrgency::Normal => Self::Normal,
            NotificationUrgency::Critical => Self::Critical,
        }
    }
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
    left.trim() == right.trim()
}

fn notification_icon_uri(item: &NotificationItem) -> Option<String> {
    item.app_icon
        .as_deref()
        .and_then(|icon| crate::apps::resolve_icon_path(Some(icon)))
        .or_else(|| crate::apps::resolve_icon_path(Some(item.app_name.as_str())))
        .as_deref()
        .and_then(icon_data_uri)
}
