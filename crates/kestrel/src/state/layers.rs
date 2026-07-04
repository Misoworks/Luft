use super::KestrelState;
use crate::layers;
use smithay::{
    desktop::PopupManager, reexports::wayland_server::protocol::wl_surface::WlSurface,
    wayland::shell::wlr_layer::LayerSurface,
};

impl KestrelState {
    pub fn map_layer_surface(&mut self, surface: LayerSurface, namespace: String) {
        self.enter_output(surface.wl_surface());
        layers::map(self.output(), surface, namespace);
    }

    pub fn unmap_layer_surface(&mut self, surface: &LayerSurface) {
        self.dismiss_popups_for_surface(surface.wl_surface());
        self.leave_output(surface.wl_surface());
        layers::unmap(self.output(), surface);
    }

    pub fn arrange_layers(&self) {
        layers::arrange(self.output());
    }

    pub fn cleanup_layers(&mut self) {
        layers::cleanup(self.output());
        self.popup_manager.cleanup();
    }

    pub fn layer_surfaces(&self) -> Vec<WlSurface> {
        let mut surfaces = layers::surfaces(self.output());
        let roots = surfaces.clone();
        for root in roots {
            surfaces.extend(
                PopupManager::popups_for_surface(&root)
                    .map(|(popup, _)| popup.wl_surface().clone()),
            );
        }
        surfaces
    }

    pub fn dismiss_popups_for_surface(&mut self, surface: &WlSurface) {
        let popups = PopupManager::popups_for_surface(surface)
            .map(|(popup, _)| popup)
            .collect::<Vec<_>>();
        for popup in popups {
            let _ = PopupManager::dismiss_popup(surface, &popup);
        }
        self.popup_manager.cleanup();
    }

    pub fn has_visible_popups(&self) -> bool {
        self.windows.iter().any(|window| {
            PopupManager::popups_for_surface(window.surface.wl_surface())
                .next()
                .is_some()
        }) || layers::surfaces(self.output())
            .iter()
            .any(|surface| PopupManager::popups_for_surface(surface).next().is_some())
    }
}
