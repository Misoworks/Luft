use super::{
    MIN_WINDOW_HEIGHT, MIN_WINDOW_WIDTH, ManagedWindow, ResizeEdge, WindowFrameHit,
    WindowRestoreState,
    hit::{
        content_contains, modifier_resize_edge_at, resize_edge_at, titlebar_contains,
        titlebar_control_at,
    },
    surface_has_client_frame_extents,
};
use crate::window_animation::WindowAnimation;
use asher_ipc::{Rect, WindowId, WorkspaceId};
use smithay::{
    desktop::{WindowSurfaceType, utils::under_from_surface_tree},
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Size},
    wayland::shell::xdg::ToplevelSurface,
};

#[derive(Debug, Default)]
pub struct WindowStack {
    windows: Vec<ManagedWindow>,
}

#[derive(Debug, Clone, Copy)]
pub struct WindowDecorationChange {
    pub id: WindowId,
    pub geometry: Rect,
    pub server_decorated: bool,
}

impl WindowStack {
    pub fn add(
        &mut self,
        id: WindowId,
        workspace: WorkspaceId,
        surface: ToplevelSurface,
        geometry: Rect,
        requested_server_decoration: bool,
        animate: bool,
    ) {
        if self.windows.iter().any(|window| window.surface == surface) {
            return;
        }
        let server_decorated =
            requested_server_decoration && !surface_has_client_frame_extents(&surface);

        self.windows.push(ManagedWindow {
            id,
            workspace,
            surface,
            location: (geometry.x, geometry.y).into(),
            size: (geometry.width, geometry.height).into(),
            requested_server_decoration,
            server_decorated,
            initial_size_pending: true,
            hidden: false,
            closing: false,
            close_sent: false,
            maximized: false,
            fullscreen: false,
            restore_geometry: None,
            fullscreen_restore: None,
            animation: WindowAnimation::open(animate),
        });
    }

    pub fn remove(&mut self, surface: &ToplevelSurface) -> Option<ManagedWindow> {
        let index = self
            .windows
            .iter()
            .position(|window| &window.surface == surface)?;
        Some(self.windows.remove(index))
    }

    pub fn retain_alive(&mut self) -> Vec<WindowId> {
        let mut removed = Vec::new();
        self.windows.retain(|window| {
            let alive = window.surface.alive();
            if !alive {
                removed.push(window.id);
            }
            alive
        });
        removed
    }

    pub fn raise_by_id(&mut self, id: WindowId) -> Option<ToplevelSurface> {
        let index = self.windows.iter().position(|window| window.id == id)?;
        let window = self.windows.remove(index);
        let surface = window.surface.clone();
        self.windows.push(window);
        Some(surface)
    }

    pub fn cycle_on_workspace(
        &mut self,
        workspace: &WorkspaceId,
        previous: bool,
    ) -> Option<(WindowId, ToplevelSurface)> {
        let visible = self
            .windows
            .iter()
            .enumerate()
            .filter(|(_, window)| &window.workspace == workspace && !window.hidden)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let target = if previous {
            *visible.first()?
        } else {
            *visible.last()?
        };
        let insert = if previous {
            *visible.last()?
        } else {
            *visible.first()?
        };

        let window = self.windows.remove(target);
        self.windows.insert(insert, window);
        let topmost = self
            .windows
            .iter()
            .rev()
            .find(|window| &window.workspace == workspace)?;

        Some((topmost.id, topmost.surface.clone()))
    }

    pub fn set_workspace(&mut self, id: WindowId, workspace: WorkspaceId) -> bool {
        let Some(window) = self.windows.iter_mut().find(|window| window.id == id) else {
            return false;
        };

        window.workspace = workspace;
        true
    }

    pub fn window(&self, id: WindowId) -> Option<&ManagedWindow> {
        self.windows.iter().find(|window| window.id == id)
    }

    pub fn initial_size_pending(&self, id: WindowId) -> bool {
        self.window(id)
            .is_some_and(|window| window.initial_size_pending)
    }

    pub fn set_initial_size_pending(&mut self, id: WindowId, pending: bool) -> bool {
        let Some(window) = self.windows.iter_mut().find(|window| window.id == id) else {
            return false;
        };

        window.initial_size_pending = pending;
        true
    }

    pub fn committed_surface_size(&self, id: WindowId) -> Option<Size<i32, Logical>> {
        let geometry = self.window(id)?.committed_surface_geometry()?;
        (geometry.size.w > 0 && geometry.size.h > 0).then_some(geometry.size)
    }

    pub fn id_for_surface(&self, surface: &ToplevelSurface) -> Option<WindowId> {
        self.windows
            .iter()
            .find(|window| &window.surface == surface)
            .map(|window| window.id)
    }

    pub fn id_for_wl_surface(&self, surface: &WlSurface) -> Option<WindowId> {
        self.windows
            .iter()
            .find(|window| window.surface.wl_surface() == surface)
            .map(|window| window.id)
    }

    pub fn set_geometry(
        &mut self,
        id: WindowId,
        geometry: Rect,
    ) -> Option<(ToplevelSurface, Rect)> {
        let window = self.windows.iter_mut().find(|window| window.id == id)?;
        let geometry = Rect::new(
            geometry.x,
            geometry.y,
            geometry.width.max(MIN_WINDOW_WIDTH),
            geometry.height.max(MIN_WINDOW_HEIGHT),
        );
        window.location = (geometry.x, geometry.y).into();
        window.size = (geometry.width, geometry.height).into();
        Some((window.surface.clone(), geometry))
    }

    pub fn animate_geometry(&mut self, id: WindowId, from: Rect, enabled: bool) {
        let Some(window) = self.windows.iter_mut().find(|window| window.id == id) else {
            return;
        };
        if window.geometry() != from {
            window.animation.geometry(from, enabled);
        }
    }

    pub fn set_requested_server_decoration(
        &mut self,
        surface: &ToplevelSurface,
        requested_server_decoration: bool,
    ) -> Option<WindowDecorationChange> {
        let window = self
            .windows
            .iter_mut()
            .find(|window| &window.surface == surface)?;

        window.requested_server_decoration = requested_server_decoration;
        update_effective_decoration(window)
    }

    pub fn refresh_decoration_for_root_surface(
        &mut self,
        surface: &WlSurface,
    ) -> Option<(ToplevelSurface, WindowDecorationChange)> {
        let window = self
            .windows
            .iter_mut()
            .find(|window| window.surface.wl_surface() == surface)?;
        let surface = window.surface.clone();
        update_effective_decoration(window).map(|change| (surface, change))
    }

    pub fn geometry_for_surface(&self, surface: &ToplevelSurface) -> Option<(WindowId, Rect)> {
        self.windows
            .iter()
            .find(|window| &window.surface == surface)
            .map(|window| (window.id, window.geometry()))
    }

    pub fn set_hidden(&mut self, id: WindowId, hidden: bool, animate: bool) -> bool {
        let Some(window) = self.windows.iter_mut().find(|window| window.id == id) else {
            return false;
        };

        if window.closing {
            return true;
        }
        if window.hidden != hidden {
            if hidden {
                window.animation.hide(animate);
            } else {
                window.animation.show(animate);
            }
        }
        window.hidden = hidden;
        true
    }

    pub fn start_close(&mut self, id: WindowId, animate: bool) -> Option<ToplevelSurface> {
        let window = self.windows.iter_mut().find(|window| window.id == id)?;
        if !animate {
            window.close_sent = true;
            return Some(window.surface.clone());
        }
        if !window.closing {
            window.closing = true;
            window.close_sent = false;
            window.animation.close(true);
        }
        None
    }

    pub fn drain_close_requests(&mut self) -> Vec<ToplevelSurface> {
        self.windows
            .iter_mut()
            .filter_map(|window| {
                if !window.closing || window.close_sent || !window.animation.close_finished() {
                    return None;
                }
                window.close_sent = true;
                Some(window.surface.clone())
            })
            .collect()
    }

    pub fn set_fullscreen(
        &mut self,
        id: WindowId,
        fullscreen: bool,
    ) -> Option<(ToplevelSurface, Rect)> {
        let window = self.windows.iter_mut().find(|window| window.id == id)?;
        window.fullscreen = fullscreen;
        Some((window.surface.clone(), window.geometry()))
    }

    pub fn set_maximized(&mut self, id: WindowId, maximized: bool) -> bool {
        let Some(window) = self.windows.iter_mut().find(|window| window.id == id) else {
            return false;
        };

        window.maximized = maximized;
        true
    }

    pub fn fullscreen_restore(&self, id: WindowId) -> Option<WindowRestoreState> {
        self.window(id)?.fullscreen_restore.clone()
    }

    pub fn set_fullscreen_restore(
        &mut self,
        id: WindowId,
        restore: Option<WindowRestoreState>,
    ) -> bool {
        let Some(window) = self.windows.iter_mut().find(|window| window.id == id) else {
            return false;
        };

        window.fullscreen_restore = restore;
        true
    }

    pub fn restore_geometry(&self, id: WindowId) -> Option<Rect> {
        self.window(id)?.restore_geometry
    }

    pub fn set_restore_geometry(&mut self, id: WindowId, geometry: Option<Rect>) -> bool {
        let Some(window) = self.windows.iter_mut().find(|window| window.id == id) else {
            return false;
        };

        window.restore_geometry = geometry;
        true
    }

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

    pub fn render_windows_on_workspace<'a>(
        &'a self,
        active_workspace: &WorkspaceId,
    ) -> impl Iterator<Item = &'a ManagedWindow> {
        let fullscreen = self.fullscreen_on_workspace(active_workspace);
        let active_workspace = active_workspace.clone();
        self.windows.iter().rev().filter(move |window| {
            fullscreen.map_or_else(
                || {
                    window.workspace == active_workspace
                        && ((!window.hidden && !window.close_sent)
                            || window.animation.renders_while_hidden()
                            || (window.closing && !window.close_sent))
                },
                |fullscreen_id| window.id == fullscreen_id && !window.close_sent,
            )
        })
    }

    pub fn topmost_on_workspace(&self, workspace: &WorkspaceId) -> Option<WindowId> {
        if let Some(id) = self.fullscreen_on_workspace(workspace) {
            return Some(id);
        }

        self.windows
            .iter()
            .rev()
            .find(|window| &window.workspace == workspace && !window.hidden && !window.closing)
            .map(|window| window.id)
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

    pub fn iter(&self) -> impl Iterator<Item = &ManagedWindow> {
        self.windows.iter()
    }

    pub fn surfaces(&self) -> Vec<WlSurface> {
        self.windows
            .iter()
            .map(|window| window.surface.wl_surface().clone())
            .collect()
    }

    pub fn animations_active(&self) -> bool {
        self.windows
            .iter()
            .any(|window| window.animation.is_active())
    }

    pub fn fullscreen_on_workspace(&self, workspace: &WorkspaceId) -> Option<WindowId> {
        self.windows
            .iter()
            .rev()
            .find(|window| {
                &window.workspace == workspace
                    && window.fullscreen
                    && !window.hidden
                    && !window.closing
            })
            .map(|window| window.id)
    }

    #[cfg(feature = "session-backend")]
    pub fn fullscreen_window(&self, workspace: &WorkspaceId) -> Option<&ManagedWindow> {
        let id = self.fullscreen_on_workspace(workspace)?;
        self.windows.iter().find(|window| window.id == id)
    }

    fn visible_windows_for_workspace(&self, workspace: &WorkspaceId) -> Vec<&ManagedWindow> {
        if let Some(fullscreen) = self.fullscreen_on_workspace(workspace) {
            return self
                .windows
                .iter()
                .find(|window| window.id == fullscreen)
                .into_iter()
                .collect();
        }

        self.windows
            .iter()
            .rev()
            .filter(|window| &window.workspace == workspace && !window.hidden && !window.closing)
            .collect()
    }
}

fn update_effective_decoration(window: &mut ManagedWindow) -> Option<WindowDecorationChange> {
    let server_decorated = window.requested_server_decoration && !window.client_frame_extents();
    if window.server_decorated == server_decorated {
        return None;
    }

    let old_titlebar = window.titlebar_height();
    let content_y = window.location.y + old_titlebar;
    window.server_decorated = server_decorated;
    window.location.y = content_y - window.titlebar_height();

    Some(WindowDecorationChange {
        id: window.id,
        geometry: window.geometry(),
        server_decorated,
    })
}

fn interactive_contains(window: &ManagedWindow, point: Point<f64, Logical>) -> bool {
    titlebar_contains(window, point)
        || resize_edge_at(window, point).is_some()
        || surface_contains(window, point)
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
