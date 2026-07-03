use super::model::WebShellSurface;
use fenestra_cef::{
    ShellSurfaceAnchor, ShellSurfaceKeyboardInteractivity, ShellSurfaceLayer, ShellSurfaceMargin,
    ShellSurfaceOptions,
};

pub(crate) const PANEL_WIDTH_HINT: i32 = 1;
const PANEL_BAR_HEIGHT: i32 = 48;
pub(crate) const PANEL_HEIGHT: i32 = PANEL_BAR_HEIGHT;
const PANEL_MENU_EDGE_MARGIN: i32 = 6;
const PANEL_MENU_GAP: i32 = 6;
const LAYER_SURFACE_ZONE_IGNORE: i32 = -1;

impl WebShellSurface {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Panel => "panel",
            Self::PanelMenu => "panel-menu",
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
            Self::Panel => "luft-panel",
            Self::PanelMenu => "luft-panel-menu",
            Self::QuickSettings => "luft-quick-settings",
            Self::DateCenter => "luft-date-center",
            Self::NotificationToast => "luft-notifications",
            Self::StartMenu => "luft-start-menu",
        }
    }
}

pub(crate) fn panel_size() -> (i32, i32) {
    (PANEL_WIDTH_HINT, PANEL_HEIGHT)
}

pub(crate) fn shell_surface(
    kind: WebShellSurface,
    size: (i32, i32),
    panel_menu_x: Option<i32>,
) -> ShellSurfaceOptions {
    let mut shell_surface = ShellSurfaceOptions::new(kind.namespace())
        .layer(layer(kind))
        .anchor(anchor(kind, panel_menu_x))
        .margin(margin(kind, size, panel_menu_x))
        .keyboard_interactivity(keyboard_interactivity(kind));
    let (width, height) = shell_size(kind, size);
    shell_surface = shell_surface.size(width, height);
    if let Some(exclusive_zone) = exclusive_zone(kind) {
        shell_surface = shell_surface.exclusive_zone(exclusive_zone);
    }
    shell_surface
}

fn shell_size(kind: WebShellSurface, size: (i32, i32)) -> (u32, u32) {
    let size = match kind {
        WebShellSurface::Panel => (0, panel_size().1),
        _ => size,
    };
    (size.0.max(0) as u32, size.1.max(0) as u32)
}

fn layer(kind: WebShellSurface) -> ShellSurfaceLayer {
    match kind {
        WebShellSurface::PanelMenu
        | WebShellSurface::NotificationToast
        | WebShellSurface::Panel => ShellSurfaceLayer::Overlay,
        _ => ShellSurfaceLayer::Top,
    }
}

fn anchor(kind: WebShellSurface, panel_menu_x: Option<i32>) -> ShellSurfaceAnchor {
    match kind {
        WebShellSurface::Panel => ShellSurfaceAnchor::BOTTOM | horizontal_anchor(),
        WebShellSurface::PanelMenu if panel_menu_x.is_some() => {
            ShellSurfaceAnchor::BOTTOM | ShellSurfaceAnchor::LEFT
        }
        WebShellSurface::PanelMenu => ShellSurfaceAnchor::BOTTOM,
        WebShellSurface::QuickSettings
        | WebShellSurface::DateCenter
        | WebShellSurface::NotificationToast => {
            ShellSurfaceAnchor::BOTTOM | ShellSurfaceAnchor::RIGHT
        }
        WebShellSurface::StartMenu => ShellSurfaceAnchor::BOTTOM,
    }
}

fn margin(
    kind: WebShellSurface,
    size: (i32, i32),
    panel_menu_x: Option<i32>,
) -> ShellSurfaceMargin {
    match kind {
        WebShellSurface::PanelMenu => ShellSurfaceMargin::new(
            0,
            0,
            PANEL_BAR_HEIGHT + PANEL_MENU_GAP,
            panel_menu_left_margin(size.0, panel_menu_x),
        ),
        WebShellSurface::StartMenu => ShellSurfaceMargin::new(0, 0, PANEL_BAR_HEIGHT + 10, 0),
        WebShellSurface::QuickSettings => ShellSurfaceMargin::new(0, 0, PANEL_BAR_HEIGHT + 8, 0),
        WebShellSurface::DateCenter => ShellSurfaceMargin::new(0, 0, PANEL_BAR_HEIGHT + 8, 0),
        WebShellSurface::NotificationToast => {
            ShellSurfaceMargin::new(0, 12, PANEL_BAR_HEIGHT + 12, 0)
        }
        _ => zero_margin(),
    }
}

fn horizontal_anchor() -> ShellSurfaceAnchor {
    ShellSurfaceAnchor::LEFT | ShellSurfaceAnchor::RIGHT
}

fn zero_margin() -> ShellSurfaceMargin {
    ShellSurfaceMargin::new(0, 0, 0, 0)
}

fn panel_menu_left_margin(width: i32, x: Option<i32>) -> i32 {
    let Some(x) = x else {
        return 0;
    };
    (x - width.max(1) / 2).max(PANEL_MENU_EDGE_MARGIN)
}

fn exclusive_zone(kind: WebShellSurface) -> Option<i32> {
    match kind {
        WebShellSurface::Panel | WebShellSurface::StartMenu | WebShellSurface::PanelMenu => {
            Some(LAYER_SURFACE_ZONE_IGNORE)
        }
        _ => None,
    }
}

fn keyboard_interactivity(kind: WebShellSurface) -> ShellSurfaceKeyboardInteractivity {
    match kind {
        WebShellSurface::Panel | WebShellSurface::NotificationToast => {
            ShellSurfaceKeyboardInteractivity::None
        }
        WebShellSurface::StartMenu => ShellSurfaceKeyboardInteractivity::OnDemand,
        WebShellSurface::PanelMenu
        | WebShellSurface::QuickSettings
        | WebShellSurface::DateCenter => ShellSurfaceKeyboardInteractivity::OnDemand,
    }
}
