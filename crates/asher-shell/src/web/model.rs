use super::{appearance::WebAppearance, icons::icon_data_uri, palette::WebPalette};
use crate::services::{
    notifications::{NotificationItem, NotificationUrgency},
    system_status::{AudioInfo, BatteryInfo, BrightnessInfo, NetworkInfo, SystemStatus},
    tray::TrayItemStatus,
};
use asher_ipc::WindowSummary;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebShellSnapshot {
    pub surface: Option<WebShellSurface>,
    pub time: String,
    pub date: String,
    pub active_workspace: String,
    pub active_profile: String,
    pub active_mode: String,
    pub blur_enabled: bool,
    pub debug_overlay: bool,
    pub user_profile_icon_uri: Option<String>,
    pub palette: WebPalette,
    pub appearance: WebAppearance,
    pub profiles: Vec<WebProfile>,
    pub workspaces: Vec<WebWorkspace>,
    pub windows: Vec<WebWindow>,
    pub panel_apps: Vec<WebPanelApp>,
    pub panel_menu_command: Option<String>,
    pub panel_menu_x: Option<i32>,
    pub applications: Vec<WebApplication>,
    pub status: WebSystemStatus,
    pub tray: Vec<WebTrayItem>,
    pub do_not_disturb: bool,
    pub notifications: Vec<WebNotification>,
    pub toast_notifications: Vec<WebNotification>,
    pub start_menu_open: bool,
    pub quick_settings_open: bool,
    pub date_center_open: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WebShellSurface {
    Panel,
    PanelMenu,
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

impl From<&WindowSummary> for WebWindow {
    fn from(window: &WindowSummary) -> Self {
        Self {
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
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebPanelApp {
    pub label: String,
    pub command: String,
    pub icon_uri: Option<String>,
    pub running: bool,
    pub active: bool,
    pub pinned: bool,
    pub window_id: Option<u64>,
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

fn notification_icon_uri(item: &NotificationItem) -> Option<String> {
    item.app_icon
        .as_deref()
        .and_then(|icon| crate::apps::resolve_icon_path(Some(icon)))
        .or_else(|| crate::apps::resolve_icon_path(Some(item.app_name.as_str())))
        .as_deref()
        .and_then(icon_data_uri)
}
