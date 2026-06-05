use super::model::WebShellSurface;
use fenestra_cef::{
    ShellSurfaceAnchor, ShellSurfaceKeyboardInteractivity, ShellSurfaceLayer, ShellSurfaceMargin,
    ShellSurfaceOptions,
};

pub(crate) const PANEL_WIDTH_HINT: i32 = 1;
pub(crate) const PANEL_HEIGHT: i32 = 34;

const TASKBAR_HEIGHT: i32 = 48;
const TASKBAR_SURFACE_HEIGHT: i32 = 96;
const DOCK_SURFACE_HEIGHT: i32 = 64;
const DOCK_HEIGHT: i32 = 50;
const DOCK_ITEM: i32 = 40;
const DOCK_GAP: i32 = 10;
const DOCK_PADDING: i32 = 22;
const DOCK_MENU_EDGE_MARGIN: i32 = 6;
const DOCK_MENU_PANEL_GAP: i32 = 6;

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
            Self::Overview => "overview",
        }
    }
}

impl WebShellSurface {
    pub(crate) fn namespace(self) -> &'static str {
        match self {
            Self::Panel => "staccato-panel",
            Self::Dock => "staccato-dock",
            Self::DockMenu => "staccato-dock-menu",
            Self::Sidebar => "staccato-sidebar",
            Self::QuickSettings => "staccato-quick-settings",
            Self::DateCenter => "staccato-date-center",
            Self::NotificationToast => "staccato-notifications",
            Self::Overview => "staccato-overview",
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

pub(crate) fn dock_size(apps: &[crate::dock::DockApp]) -> (i32, i32) {
    let count = apps.len() as i32;
    let gaps = apps.len().saturating_sub(1) as i32 * DOCK_GAP;
    let width = count * DOCK_ITEM + gaps + DOCK_PADDING;
    (width.max(DOCK_HEIGHT), DOCK_SURFACE_HEIGHT)
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
    if !matches!(
        kind,
        WebShellSurface::Overview | WebShellSurface::QuickSettings | WebShellSurface::DateCenter
    ) {
        let (width, height) = shell_size(kind, size, panel_taskbar);
        shell_surface = shell_surface.size(width, height);
    }
    if let Some(exclusive_zone) = exclusive_zone(kind, panel_taskbar) {
        shell_surface = shell_surface.exclusive_zone(exclusive_zone);
    }
    shell_surface
}

fn shell_size(kind: WebShellSurface, size: (i32, i32), panel_taskbar: bool) -> (u32, u32) {
    let size = match kind {
        WebShellSurface::Panel => (0, panel_size(panel_taskbar).1),
        WebShellSurface::Sidebar => (size.0, 0),
        WebShellSurface::Overview => (0, 0),
        _ => size,
    };
    (size.0.max(0) as u32, size.1.max(0) as u32)
}

fn layer(kind: WebShellSurface) -> ShellSurfaceLayer {
    match kind {
        WebShellSurface::Overview
        | WebShellSurface::DockMenu
        | WebShellSurface::QuickSettings
        | WebShellSurface::DateCenter
        | WebShellSurface::NotificationToast => ShellSurfaceLayer::Overlay,
        _ => ShellSurfaceLayer::Top,
    }
}

fn anchor(
    kind: WebShellSurface,
    panel_taskbar: bool,
    dock_menu_x: Option<i32>,
) -> ShellSurfaceAnchor {
    match kind {
        WebShellSurface::Panel if panel_taskbar => {
            ShellSurfaceAnchor::BOTTOM | ShellSurfaceAnchor::horizontal()
        }
        WebShellSurface::Panel => ShellSurfaceAnchor::TOP | ShellSurfaceAnchor::horizontal(),
        WebShellSurface::Dock => ShellSurfaceAnchor::BOTTOM,
        WebShellSurface::DockMenu if panel_taskbar && dock_menu_x.is_some() => {
            ShellSurfaceAnchor::BOTTOM | ShellSurfaceAnchor::LEFT
        }
        WebShellSurface::DockMenu if panel_taskbar => ShellSurfaceAnchor::BOTTOM,
        WebShellSurface::DockMenu => ShellSurfaceAnchor::BOTTOM,
        WebShellSurface::Sidebar => ShellSurfaceAnchor::LEFT | ShellSurfaceAnchor::vertical(),
        WebShellSurface::QuickSettings | WebShellSurface::DateCenter => ShellSurfaceAnchor::ALL,
        WebShellSurface::NotificationToast if panel_taskbar => {
            ShellSurfaceAnchor::BOTTOM | ShellSurfaceAnchor::RIGHT
        }
        WebShellSurface::NotificationToast => ShellSurfaceAnchor::TOP | ShellSurfaceAnchor::RIGHT,
        WebShellSurface::Overview => ShellSurfaceAnchor::ALL,
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
        WebShellSurface::QuickSettings | WebShellSurface::DateCenter => ShellSurfaceMargin::ZERO,
        WebShellSurface::NotificationToast if panel_taskbar => {
            ShellSurfaceMargin::new(0, 12, TASKBAR_HEIGHT + 12, 0)
        }
        WebShellSurface::NotificationToast => ShellSurfaceMargin::new(PANEL_HEIGHT + 12, 12, 0, 0),
        _ => ShellSurfaceMargin::ZERO,
    }
}

fn dock_menu_left_margin(width: i32, x: Option<i32>) -> i32 {
    let Some(x) = x else {
        return 0;
    };
    (x - width.max(1) / 2).max(DOCK_MENU_EDGE_MARGIN)
}

fn exclusive_zone(kind: WebShellSurface, panel_taskbar: bool) -> Option<i32> {
    match kind {
        WebShellSurface::Panel if panel_taskbar => None,
        WebShellSurface::Panel => None,
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
        WebShellSurface::Overview => ShellSurfaceKeyboardInteractivity::Exclusive,
        WebShellSurface::DockMenu
        | WebShellSurface::QuickSettings
        | WebShellSurface::DateCenter => ShellSurfaceKeyboardInteractivity::OnDemand,
    }
}
