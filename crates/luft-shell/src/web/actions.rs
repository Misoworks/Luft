use luft_ipc::{WindowId, WorkspaceId};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum WebShellAction {
    OpenLauncher,
    LaunchDefaultApp {
        app: luft_ipc::DefaultAppKind,
    },
    ToggleStartMenu,
    CloseStartMenu,
    ToggleQuickSettings,
    CloseQuickSettings,
    ToggleDateCenter,
    CloseDateCenter,
    WorkspaceSwitch {
        workspace: String,
    },
    WorkspaceRelative {
        offset: i32,
    },
    WorkspaceNew,
    WindowActivate {
        window: u64,
    },
    WindowClose {
        window: u64,
    },
    WindowMinimize {
        window: u64,
    },
    WindowMove {
        window: u64,
        workspace: String,
    },
    PanelLaunch {
        command: String,
    },
    PanelMenuOpen {
        command: String,
        x: Option<i32>,
    },
    PanelMenuClose,
    PanelPin {
        label: String,
        command: String,
        icon: Option<String>,
    },
    PanelUnpin {
        command: String,
    },
    PanelForceQuit {
        command: String,
    },
    PanelReorder {
        commands: Vec<String>,
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
    QuickSetVolume {
        percent: u8,
    },
    QuickToggleMute,
    QuickSetBrightness {
        percent: u8,
    },
    SessionCommand {
        command: SessionCommand,
    },
    SessionMenuOpen,
    SessionMenuClose,
    ToggleSessionMenu,
    ReloadConfig,
    OpenLogsFolder,
    NotificationClose {
        notification: u32,
    },
    NotificationClearAll,
    NotificationDoNotDisturb {
        enabled: bool,
    },
    NotificationAction {
        notification: u32,
        action: String,
    },
}

impl WebShellAction {
    pub fn affects_popover(&self) -> bool {
        matches!(
            self,
            Self::ToggleStartMenu
                | Self::CloseStartMenu
                | Self::ToggleQuickSettings
                | Self::CloseQuickSettings
                | Self::ToggleDateCenter
                | Self::CloseDateCenter
        )
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionCommand {
    Lock,
    Suspend,
    Reboot,
    PowerOff,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum QuickSettingsPage {
    Appearance,
    Network,
    Audio,
    Power,
}

impl QuickSettingsPage {
    pub fn as_settings_arg(self) -> &'static str {
        match self {
            Self::Appearance => "appearance",
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
