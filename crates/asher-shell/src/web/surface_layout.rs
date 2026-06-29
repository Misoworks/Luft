use super::model::WebShellSurface;
use fenestra_cef::{
    ShellSurfaceAnchor, ShellSurfaceKeyboardInteractivity, ShellSurfaceLayer, ShellSurfaceMargin,
    ShellSurfaceOptions,
};

pub(crate) const PANEL_WIDTH_HINT: i32 = 1;
pub(crate) const PANEL_HEIGHT: i32 = 34;

const TASKBAR_HEIGHT: i32 = 48;
const TASKBAR_SURFACE_HEIGHT: i32 = 62;
const DOCK_HEIGHT: i32 = 50;
const DOCK_ITEM: i32 = 40;
const DOCK_GAP: i32 = 10;
const DOCK_PADDING: i32 = 22;
const DOCK_MENU_EDGE_MARGIN: i32 = 6;
const DOCK_MENU_PANEL_GAP: i32 = 6;
const LAYER_SURFACE_ZONE_IGNORE: i32 = -1;

impl WebShellSurface {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Panel => "panel",
            Self::Dock => "dock",
            Self::DockMenu => "dock-menu",
            Self::Sidebar => "sidebar",
            Self::QuickSettings => "quick-settings",
            Self::DateCenter => "date-center",
            Self::NotificationToast => "notification-toast",
            Self::StartMenu => "start-menu",
        }
    }
}

impl WebShellSurface {
    pub(crate) fn namespace(self) -> &'static str {
        match self {
            Self::Panel => "asher-panel",
            Self::Dock => "asher-dock",
            Self::DockMenu => "asher-dock-menu",
            Self::Sidebar => "asher-sidebar",
            Self::QuickSettings => "asher-quick-settings",
            Self::DateCenter => "asher-date-center",
            Self::NotificationToast => "asher-notifications",
            Self::StartMenu => "asher-start-menu",
        }
    }
}

pub(crate) fn panel_size(taskbar: bool) -> (i32, i32) {
    (
        PANEL_WIDTH_HINT,
        if taskbar {
            TASKBAR_SURFACE_HEIGHT
        } else {
            PANEL_HEIGHT
        },
    )
}

pub(crate) fn dock_size(apps: &[crate::dock::DockApp], icon_size: u16) -> (i32, i32) {
    let item = dock_item_size(icon_size);
    let count = apps.len() as i32;
    let gaps = apps.len().saturating_sub(1) as i32 * DOCK_GAP;
    let width = count * item + gaps + DOCK_PADDING;
    (width.max(DOCK_HEIGHT), (item + 28).max(64))
}

pub(crate) fn dock_item_size(icon_size: u16) -> i32 {
    i32::from(icon_size.clamp(32, 64)).max(DOCK_ITEM)
}

pub(crate) fn shell_surface(
    kind: WebShellSurface,
    size: (i32, i32),
    panel_taskbar: bool,
    dock_menu_x: Option<i32>,
) -> ShellSurfaceOptions {
    let mut shell_surface = ShellSurfaceOptions::new(kind.namespace())
        .layer(layer(kind))
        .anchor(anchor(kind, panel_taskbar, dock_menu_x))
        .margin(margin(kind, size, panel_taskbar, dock_menu_x))
        .keyboard_interactivity(keyboard_interactivity(kind));
    let (width, height) = shell_size(kind, size, panel_taskbar);
    shell_surface = shell_surface.size(width, height);
    if let Some(exclusive_zone) = exclusive_zone(kind, panel_taskbar) {
        shell_surface = shell_surface.exclusive_zone(exclusive_zone);
    }
    shell_surface
}

fn shell_size(kind: WebShellSurface, size: (i32, i32), panel_taskbar: bool) -> (u32, u32) {
    let size = match kind {
        WebShellSurface::Panel => (0, panel_size(panel_taskbar).1),
        WebShellSurface::Sidebar => (size.0, 0),
        _ => size,
    };
    (size.0.max(0) as u32, size.1.max(0) as u32)
}

fn layer(kind: WebShellSurface) -> ShellSurfaceLayer {
    match kind {
        WebShellSurface::DockMenu | WebShellSurface::NotificationToast | WebShellSurface::Panel => {
            ShellSurfaceLayer::Overlay
        }
        _ => ShellSurfaceLayer::Top,
    }
}

fn anchor(
    kind: WebShellSurface,
    panel_taskbar: bool,
    dock_menu_x: Option<i32>,
) -> ShellSurfaceAnchor {
    match kind {
        WebShellSurface::Panel if panel_taskbar => ShellSurfaceAnchor::BOTTOM | horizontal_anchor(),
        WebShellSurface::Panel => ShellSurfaceAnchor::TOP | horizontal_anchor(),
        WebShellSurface::Dock => ShellSurfaceAnchor::BOTTOM,
        WebShellSurface::DockMenu if panel_taskbar && dock_menu_x.is_some() => {
            ShellSurfaceAnchor::BOTTOM | ShellSurfaceAnchor::LEFT
        }
        WebShellSurface::DockMenu if panel_taskbar => ShellSurfaceAnchor::BOTTOM,
        WebShellSurface::DockMenu => ShellSurfaceAnchor::BOTTOM,
        WebShellSurface::Sidebar => ShellSurfaceAnchor::LEFT | vertical_anchor(),
        WebShellSurface::QuickSettings | WebShellSurface::DateCenter if panel_taskbar => {
            ShellSurfaceAnchor::BOTTOM | ShellSurfaceAnchor::RIGHT
        }
        WebShellSurface::QuickSettings | WebShellSurface::DateCenter => {
            ShellSurfaceAnchor::TOP | ShellSurfaceAnchor::RIGHT
        }
        WebShellSurface::NotificationToast if panel_taskbar => {
            ShellSurfaceAnchor::BOTTOM | ShellSurfaceAnchor::RIGHT
        }
        WebShellSurface::NotificationToast => ShellSurfaceAnchor::TOP | ShellSurfaceAnchor::RIGHT,
        WebShellSurface::StartMenu => ShellSurfaceAnchor::BOTTOM,
    }
}

fn margin(
    kind: WebShellSurface,
    size: (i32, i32),
    panel_taskbar: bool,
    dock_menu_x: Option<i32>,
) -> ShellSurfaceMargin {
    match kind {
        WebShellSurface::Dock => ShellSurfaceMargin::new(0, 0, 12, 0),
        WebShellSurface::DockMenu if panel_taskbar => ShellSurfaceMargin::new(
            0,
            0,
            TASKBAR_HEIGHT + DOCK_MENU_PANEL_GAP,
            dock_menu_left_margin(size.0, dock_menu_x),
        ),
        WebShellSurface::DockMenu => ShellSurfaceMargin::new(0, 0, 84, 0),
        WebShellSurface::StartMenu if panel_taskbar => {
            ShellSurfaceMargin::new(0, 0, TASKBAR_HEIGHT + 10, 0)
        }
        WebShellSurface::StartMenu => ShellSurfaceMargin::new(0, 0, 84, 0),
        WebShellSurface::QuickSettings if panel_taskbar => {
            ShellSurfaceMargin::new(0, 0, TASKBAR_HEIGHT + 8, 0)
        }
        WebShellSurface::DateCenter if panel_taskbar => {
            ShellSurfaceMargin::new(0, 0, TASKBAR_HEIGHT + 8, 0)
        }
        WebShellSurface::QuickSettings | WebShellSurface::DateCenter => {
            ShellSurfaceMargin::new(PANEL_HEIGHT + 8, 0, 0, 0)
        }
        WebShellSurface::NotificationToast if panel_taskbar => {
            ShellSurfaceMargin::new(0, 12, TASKBAR_HEIGHT + 12, 0)
        }
        WebShellSurface::NotificationToast => ShellSurfaceMargin::new(PANEL_HEIGHT + 12, 12, 0, 0),
        _ => zero_margin(),
    }
}

fn horizontal_anchor() -> ShellSurfaceAnchor {
    ShellSurfaceAnchor::LEFT | ShellSurfaceAnchor::RIGHT
}

fn vertical_anchor() -> ShellSurfaceAnchor {
    ShellSurfaceAnchor::TOP | ShellSurfaceAnchor::BOTTOM
}

fn zero_margin() -> ShellSurfaceMargin {
    ShellSurfaceMargin::new(0, 0, 0, 0)
}

fn dock_menu_left_margin(width: i32, x: Option<i32>) -> i32 {
    let Some(x) = x else {
        return 0;
    };
    (x - width.max(1) / 2).max(DOCK_MENU_EDGE_MARGIN)
}

fn exclusive_zone(kind: WebShellSurface, _panel_taskbar: bool) -> Option<i32> {
    match kind {
        WebShellSurface::Panel
        | WebShellSurface::StartMenu
        | WebShellSurface::Dock
        | WebShellSurface::DockMenu => Some(LAYER_SURFACE_ZONE_IGNORE),
        WebShellSurface::Sidebar => Some(108),
        _ => None,
    }
}

fn keyboard_interactivity(kind: WebShellSurface) -> ShellSurfaceKeyboardInteractivity {
    match kind {
        WebShellSurface::Panel
        | WebShellSurface::Dock
        | WebShellSurface::Sidebar
        | WebShellSurface::NotificationToast => ShellSurfaceKeyboardInteractivity::None,
        WebShellSurface::StartMenu => ShellSurfaceKeyboardInteractivity::OnDemand,
        WebShellSurface::DockMenu
        | WebShellSurface::QuickSettings
        | WebShellSurface::DateCenter => ShellSurfaceKeyboardInteractivity::OnDemand,
    }
}
