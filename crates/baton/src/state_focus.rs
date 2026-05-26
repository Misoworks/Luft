use crate::{
    layers,
    state::BatonState,
    window::{ResizeEdge, WindowFrameHit},
};
use smithay::{
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point},
    wayland::compositor::{TraversalAction, with_surface_tree_downward},
    wayland::shell::{wlr_layer::LayerSurface, xdg::ToplevelSurface},
};

impl BatonState {
    pub fn pointer_focus(
        &self,
        location: Point<f64, Logical>,
    ) -> Option<(WlSurface, Point<f64, Logical>)> {
        let fullscreen = self
            .windows
            .fullscreen_on_workspace(self.layout.active_workspace())
            .is_some();
        if !fullscreen {
            if let Some(focus) = layers::pointer_focus(&self.output, location) {
                return Some((focus.surface, focus.location));
            }
        }

        self.windows
            .pointer_focus(location, self.layout.active_workspace())
    }

    pub fn keyboard_focus(&self, location: Point<f64, Logical>) -> Option<WlSurface> {
        let fullscreen = self
            .windows
            .fullscreen_on_workspace(self.layout.active_workspace())
            .is_some();
        if !fullscreen {
            if let Some(surface) = layers::keyboard_focus(&self.output, location) {
                return Some(surface);
            }
        }

        let surface = self
            .windows
            .window_at(location, self.layout.active_workspace())?;
        Some(surface.wl_surface().clone())
    }

    pub fn window_at_for_shell_interaction(
        &self,
        location: Point<f64, Logical>,
    ) -> Option<ToplevelSurface> {
        let fullscreen = self
            .windows
            .fullscreen_on_workspace(self.layout.active_workspace())
            .is_some();
        if !fullscreen && layers::has_layer_above_windows(&self.output, location) {
            return None;
        }

        self.windows
            .window_at(location, self.layout.active_workspace())
    }

    pub fn window_frame_hit(&self, location: Point<f64, Logical>) -> Option<WindowFrameHit> {
        let fullscreen = self
            .windows
            .fullscreen_on_workspace(self.layout.active_workspace())
            .is_some();
        if !fullscreen && layers::has_layer_above_windows(&self.output, location) {
            return None;
        }

        self.windows
            .frame_hit(location, self.layout.active_workspace())
    }

    pub fn modifier_resize_at(
        &self,
        location: Point<f64, Logical>,
    ) -> Option<(ToplevelSurface, ResizeEdge)> {
        let fullscreen = self
            .windows
            .fullscreen_on_workspace(self.layout.active_workspace())
            .is_some();
        if !fullscreen && layers::has_layer_above_windows(&self.output, location) {
            return None;
        }

        self.windows
            .modifier_resize_at(location, self.layout.active_workspace())
    }

    pub fn layer_surface_for_commit(&self, surface: &WlSurface) -> Option<LayerSurface> {
        self.layer_shell_state
            .layer_surfaces()
            .find(|layer| layer.wl_surface() == surface)
    }

    pub fn commit_surface_needs_render(&self, surface: &WlSurface) -> bool {
        if self.visible_window_contains_surface(surface) {
            return true;
        }

        layers::surfaces(&self.output)
            .iter()
            .any(|root| surface_tree_contains(root, surface))
    }

    fn visible_window_contains_surface(&self, surface: &WlSurface) -> bool {
        if let Some(transition) = self.workspace_transition() {
            return self
                .windows
                .render_windows_on_workspace(&transition.from)
                .chain(self.windows.render_windows_on_workspace(&transition.to))
                .any(|window| surface_tree_contains(window.surface.wl_surface(), surface));
        }

        self.windows
            .render_windows_on_workspace(self.layout.active_workspace())
            .any(|window| surface_tree_contains(window.surface.wl_surface(), surface))
    }
}

fn surface_tree_contains(root: &WlSurface, needle: &WlSurface) -> bool {
    let mut found = false;
    with_surface_tree_downward(
        root,
        (),
        |surface, _, &()| {
            if surface == needle {
                found = true;
                TraversalAction::Break
            } else {
                TraversalAction::DoChildren(())
            }
        },
        |_, _, &()| {},
        |_, _, &()| true,
    );
    found
}
