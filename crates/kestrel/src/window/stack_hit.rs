use super::{
    ManagedWindow, ResizeEdge, WindowFrameHit, WindowStack,
    hit::{
        content_contains, modifier_resize_edge_at, resize_edge_at, titlebar_contains,
        titlebar_control_at,
    },
};
use luft_ipc::WorkspaceId;
use smithay::{
    desktop::{PopupKind, PopupManager, WindowSurfaceType, utils::under_from_surface_tree},
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point},
    wayland::shell::xdg::ToplevelSurface,
};

impl WindowStack {
    pub fn window_at(
        &self,
        point: Point<f64, Logical>,
        active_workspace: &WorkspaceId,
    ) -> Option<ToplevelSurface> {
        self.visible_windows_for_workspace(active_workspace)
            .iter()
            .copied()
            .find(|window| interactive_contains(window, point))
            .map(|window| window.surface.clone())
    }

    pub fn pointer_focus(
        &self,
        point: Point<f64, Logical>,
        active_workspace: &WorkspaceId,
    ) -> Option<(WlSurface, Point<f64, Logical>)> {
        for window in self.visible_windows_for_workspace(active_workspace) {
            if let Some(focus) = popup_pointer_focus(window, point) {
                return Some(focus);
            }
            if !content_contains(window, point) {
                continue;
            }

            if let Some((surface, location)) = under_from_surface_tree(
                window.surface.wl_surface(),
                point,
                window.surface_location(),
                WindowSurfaceType::ALL,
            ) {
                return Some((surface, location.to_f64()));
            }
        }

        None
    }

    pub fn frame_hit(
        &self,
        point: Point<f64, Logical>,
        active_workspace: &WorkspaceId,
    ) -> Option<WindowFrameHit> {
        for window in self.visible_windows_for_workspace(active_workspace) {
            if titlebar_contains(window, point) {
                if let Some(control) = titlebar_control_at(window, point) {
                    return Some(WindowFrameHit::Control {
                        id: window.id,
                        control,
                    });
                }

                return Some(WindowFrameHit::Titlebar {
                    surface: window.surface.clone(),
                });
            }

            if let Some(edge) = resize_edge_at(window, point) {
                return Some(WindowFrameHit::Resize {
                    surface: window.surface.clone(),
                    edge,
                });
            }

            if surface_contains(window, point) {
                return None;
            }
        }

        None
    }

    pub fn modifier_resize_at(
        &self,
        point: Point<f64, Logical>,
        active_workspace: &WorkspaceId,
    ) -> Option<(ToplevelSurface, ResizeEdge)> {
        let window = self
            .visible_windows_for_workspace(active_workspace)
            .iter()
            .copied()
            .find(|window| interactive_contains(window, point))?;
        let edge = modifier_resize_edge_at(window, point)?;
        Some((window.surface.clone(), edge))
    }

    pub fn client_drag_surface_at(
        &self,
        point: Point<f64, Logical>,
        active_workspace: &WorkspaceId,
    ) -> Option<ToplevelSurface> {
        self.visible_windows_for_workspace(active_workspace)
            .iter()
            .copied()
            .find(|window| client_drag_region_contains(window, point))
            .map(|window| window.surface.clone())
    }
}

fn interactive_contains(window: &ManagedWindow, point: Point<f64, Logical>) -> bool {
    titlebar_contains(window, point)
        || resize_edge_at(window, point).is_some()
        || surface_contains(window, point)
        || popup_contains(window, point)
}

fn surface_contains(window: &ManagedWindow, point: Point<f64, Logical>) -> bool {
    under_from_surface_tree(
        window.surface.wl_surface(),
        point,
        window.surface_location(),
        WindowSurfaceType::ALL,
    )
    .is_some()
}

fn popup_pointer_focus(
    window: &ManagedWindow,
    point: Point<f64, Logical>,
) -> Option<(WlSurface, Point<f64, Logical>)> {
    for (popup, popup_offset) in PopupManager::popups_for_surface(window.surface.wl_surface()) {
        let location = popup_location(window, &popup, popup_offset);
        if let Some((surface, location)) =
            under_from_surface_tree(popup.wl_surface(), point, location, WindowSurfaceType::ALL)
        {
            return Some((surface, location.to_f64()));
        }
    }

    None
}

fn popup_contains(window: &ManagedWindow, point: Point<f64, Logical>) -> bool {
    popup_pointer_focus(window, point).is_some()
}

fn popup_location(
    window: &ManagedWindow,
    popup: &PopupKind,
    popup_offset: Point<i32, Logical>,
) -> Point<i32, Logical> {
    window.surface_location() + popup_offset - popup.geometry().loc
}

fn client_drag_region_contains(window: &ManagedWindow, point: Point<f64, Logical>) -> bool {
    const CLIENT_DRAG_HEIGHT: f64 = 44.0;

    if window.server_decorated || window.fullscreen || !surface_contains(window, point) {
        return false;
    }

    let content = window.content_location().to_f64();
    let size = window.size.to_f64();
    point.x >= content.x
        && point.x < content.x + size.w
        && point.y >= content.y
        && point.y < content.y + CLIENT_DRAG_HEIGHT.min(size.h)
}
