use serde::Deserialize;
use staccato_layout::{WindowId, WorkspaceId};

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum WebShellAction {
    OpenLauncher,
    ToggleOverview,
    ToggleQuickSettings,
    ToggleDateCenter,
    ToggleShellStyle,
    WorkspaceSwitch {
        workspace: String,
    },
    WorkspaceRelative {
        offset: i32,
    },
    WindowActivate {
        window: u64,
    },
    WindowMove {
        window: u64,
        workspace: String,
    },
    DockLaunch {
        command: String,
    },
    DockMenuOpen {
        command: String,
    },
    DockMenuClose,
    DockPin {
        label: String,
        command: String,
        icon: Option<String>,
    },
    DockUnpin {
        command: String,
    },
    AppLaunch {
        command: String,
    },
    TrayActivate {
        index: usize,
    },
    TrayMenu {
        index: usize,
    },
    QuickOpenSettings {
        page: QuickSettingsPage,
    },
    QuickToggleBlur,
    QuickToggleDebugOverlay,
    QuickNextProfile,
    QuickReloadConfig,
    NotificationClose {
        notification: u32,
    },
    NotificationAction {
        notification: u32,
        action: String,
    },
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum QuickSettingsPage {
    Network,
    Audio,
    Power,
}

impl QuickSettingsPage {
    pub fn gnome_panel(self) -> &'static str {
        match self {
            Self::Network => "network",
            Self::Audio => "sound",
            Self::Power => "power",
        }
    }
}

pub fn workspace_id(value: String) -> WorkspaceId {
    WorkspaceId(value)
}

pub fn window_id(value: u64) -> WindowId {
    WindowId(value)
}
