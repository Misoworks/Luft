use super::model::WebShellSurface;
use gtk::prelude::*;
use gtk_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

pub(crate) const PANEL_WIDTH_HINT: i32 = 1;
pub(crate) const PANEL_HEIGHT: i32 = 34;
pub(crate) const TASKBAR_HEIGHT: i32 = 48;

const TASKBAR_SURFACE_HEIGHT: i32 = 96;
const DOCK_SURFACE_HEIGHT: i32 = 64;
const DOCK_HEIGHT: i32 = 50;
const DOCK_ITEM: i32 = 40;
const DOCK_GAP: i32 = 10;
const DOCK_PADDING: i32 = 22;
const DOCK_MENU_BOTTOM_MARGIN: i32 = 84;
const TASKBAR_POPOVER_GAP: i32 = 6;

impl WebShellSurface {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Panel => "panel",
            Self::Dock => "dock",
            Self::DockMenu => "dock-menu",
            Self::Sidebar => "sidebar",
            Self::QuickSettings => "quick-settings",
            Self::DateCenter => "date-center",
            Self::Overview => "overview",
        }
    }

    fn namespace(self) -> &'static str {
        match self {
            Self::Panel => "staccato-panel",
            Self::Dock => "staccato-dock",
            Self::DockMenu => "staccato-dock-menu",
            Self::Sidebar => "staccato-sidebar",
            Self::QuickSettings => "staccato-quick-settings",
            Self::DateCenter => "staccato-date-center",
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

pub(crate) fn configure_window(window: &gtk::Window, kind: WebShellSurface, size: (i32, i32)) {
    window.set_decorated(false);
    window.set_resizable(false);
    window.set_app_paintable(true);
    window.set_default_size(size.0, size.1);
    if fixed_size(kind) || kind == WebShellSurface::Panel {
        window.set_size_request(size.0, size.1);
    }
    apply_rgba_visual(window);
    window.init_layer_shell();
    window.set_namespace(kind.namespace());
    window.set_keyboard_mode(match kind {
        WebShellSurface::Overview => KeyboardMode::OnDemand,
        _ => KeyboardMode::None,
    });

    match kind {
        WebShellSurface::Panel => configure_panel_window(window, false),
        WebShellSurface::Dock => configure_dock_window(window),
        WebShellSurface::DockMenu => configure_dock_menu_window(window, false),
        WebShellSurface::Sidebar => configure_sidebar_window(window),
        WebShellSurface::QuickSettings | WebShellSurface::DateCenter => {
            configure_popover_window(window, kind, false);
        }
        WebShellSurface::Overview => configure_overview_window(window),
    }
}

pub(crate) fn configure_panel_window(window: &gtk::Window, taskbar: bool) {
    window.set_layer(Layer::Top);
    window.set_exclusive_zone(if taskbar {
        TASKBAR_HEIGHT
    } else {
        PANEL_HEIGHT
    });
    clear_margins(window);
    if taskbar {
        anchor(window, &[Edge::Bottom, Edge::Left, Edge::Right]);
    } else {
        anchor(window, &[Edge::Top, Edge::Left, Edge::Right]);
    }
}

pub(crate) fn configure_popover_window(window: &gtk::Window, kind: WebShellSurface, taskbar: bool) {
    window.set_layer(Layer::Overlay);
    window.set_exclusive_zone(0);
    clear_margins(window);
    match (kind, taskbar) {
        (WebShellSurface::DockMenu, false) => configure_dock_menu_window(window, false),
        (WebShellSurface::DockMenu, true) => configure_dock_menu_window(window, true),
        (WebShellSurface::QuickSettings, false) => {
            anchor(window, &[Edge::Top, Edge::Right]);
            window.set_layer_shell_margin(Edge::Top, PANEL_HEIGHT + 8);
            window.set_layer_shell_margin(Edge::Right, 12);
        }
        (WebShellSurface::QuickSettings, true) => {
            anchor(window, &[Edge::Bottom, Edge::Right]);
            window.set_layer_shell_margin(Edge::Bottom, TASKBAR_POPOVER_GAP);
            window.set_layer_shell_margin(Edge::Right, 12);
        }
        (WebShellSurface::DateCenter, false) => {
            anchor(window, &[Edge::Top]);
            window.set_layer_shell_margin(Edge::Top, PANEL_HEIGHT + 8);
        }
        (WebShellSurface::DateCenter, true) => {
            anchor(window, &[Edge::Bottom, Edge::Right]);
            window.set_layer_shell_margin(Edge::Bottom, TASKBAR_POPOVER_GAP);
            window.set_layer_shell_margin(Edge::Right, 12);
        }
        _ => {}
    }
}

pub(crate) fn configure_content_size(
    container: &gtk::Box,
    kind: WebShellSurface,
    size: (i32, i32),
) {
    if kind == WebShellSurface::Panel {
        container.set_size_request(-1, size.1);
        container.set_hexpand(true);
        container.set_vexpand(false);
        return;
    }
    if kind == WebShellSurface::Sidebar {
        container.set_size_request(size.0, -1);
        container.set_hexpand(false);
        container.set_vexpand(true);
        return;
    }
    if !fixed_size(kind) {
        return;
    }
    container.set_size_request(size.0, size.1);
    container.set_hexpand(false);
    container.set_vexpand(false);
}

pub(crate) fn fixed_size(kind: WebShellSurface) -> bool {
    matches!(
        kind,
        WebShellSurface::Dock
            | WebShellSurface::DockMenu
            | WebShellSurface::QuickSettings
            | WebShellSurface::DateCenter
    )
}

fn configure_dock_window(window: &gtk::Window) {
    window.set_layer(Layer::Top);
    anchor(window, &[Edge::Bottom]);
    clear_margins(window);
    window.set_layer_shell_margin(Edge::Bottom, 12);
    window.set_exclusive_zone(0);
}

fn configure_dock_menu_window(window: &gtk::Window, taskbar: bool) {
    window.set_layer(Layer::Overlay);
    anchor(window, &[Edge::Bottom]);
    clear_margins(window);
    let margin = if taskbar {
        TASKBAR_POPOVER_GAP
    } else {
        DOCK_MENU_BOTTOM_MARGIN
    };
    window.set_layer_shell_margin(Edge::Bottom, margin);
    window.set_exclusive_zone(0);
}

fn configure_sidebar_window(window: &gtk::Window) {
    window.set_layer(Layer::Top);
    anchor(window, &[Edge::Top, Edge::Bottom, Edge::Left]);
    clear_margins(window);
    window.set_exclusive_zone(108);
}

fn configure_overview_window(window: &gtk::Window) {
    window.set_layer(Layer::Overlay);
    anchor(window, &[Edge::Top, Edge::Right, Edge::Bottom, Edge::Left]);
    clear_margins(window);
    window.set_exclusive_zone(0);
}

fn anchor(window: &gtk::Window, edges: &[Edge]) {
    for edge in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
        window.set_anchor(edge, edges.contains(&edge));
    }
}

fn clear_margins(window: &gtk::Window) {
    for edge in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
        window.set_layer_shell_margin(edge, 0);
    }
}

fn apply_rgba_visual(window: &gtk::Window) {
    let Some(screen) = gtk::prelude::WidgetExt::screen(window) else {
        return;
    };
    if let Some(visual) = screen.rgba_visual() {
        window.set_visual(Some(&visual));
    }
}
