use super::BatonState;
use crate::{layers, output::configure_output};
use smithay::{
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Physical, Size},
};
use staccato_layout::Rect;

impl BatonState {
    pub fn set_output_size(&mut self, size: Size<i32, Physical>) {
        self.output_size = size;
        configure_output(&self.output, size, self.output_refresh_millihertz);
        self.layout.set_bounds(Rect::new(0, 0, size.w, size.h));
        layers::arrange(&self.output);
        self.apply_active_arrangement();
        self.mark_scene_dirty();
    }

    pub fn set_output_refresh(&mut self, refresh_millihertz: i32) {
        if self.output_refresh_millihertz == refresh_millihertz {
            return;
        }

        self.output_refresh_millihertz = refresh_millihertz;
        configure_output(&self.output, self.output_size, refresh_millihertz);
        self.mark_scene_dirty();
    }

    pub fn enter_output(&self, surface: &WlSurface) {
        self.output.enter(surface);
        self.update_surface_scale(surface);
    }

    pub fn leave_output(&self, surface: &WlSurface) {
        self.output.leave(surface);
    }

    pub fn cleanup_output(&self) {
        self.output.cleanup();
    }
}
