use super::KestrelState;
use crate::layers;
use asher_layout::Rect;
use smithay::{
    output::Output,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Physical, Size},
};

impl KestrelState {
    pub fn output(&self) -> &Output {
        self.outputs.primary_output()
    }

    pub fn output_size(&self) -> Size<i32, Physical> {
        self.outputs.primary_size()
    }

    #[cfg(feature = "session-backend")]
    pub fn output_refresh_millihertz(&self) -> i32 {
        self.outputs.primary_refresh_millihertz()
    }

    pub fn output_scale(&self) -> f64 {
        self.outputs.primary_scale()
    }

    pub fn output_transform(&self) -> smithay::utils::Transform {
        self.outputs.primary_transform()
    }

    #[cfg(feature = "session-backend")]
    pub fn set_output_descriptors(&mut self, descriptors: Vec<crate::output::OutputDescriptor>) {
        self.outputs
            .replace(&self.display_handle, &self.config.display, descriptors);
        self.resize_primary_layout();
    }

    #[cfg(feature = "session-backend")]
    fn resize_primary_layout(&mut self) {
        let size = self.output_size();
        self.layout.set_bounds(Rect::new(0, 0, size.w, size.h));
        layers::arrange(self.output());
        self.apply_active_arrangement();
        self.mark_scene_dirty();
    }

    pub fn set_output_size(&mut self, size: Size<i32, Physical>) {
        self.outputs.set_primary_size(size);
        self.layout.set_bounds(Rect::new(0, 0, size.w, size.h));
        layers::arrange(self.output());
        self.apply_active_arrangement();
        self.mark_scene_dirty();
    }

    pub fn set_output_refresh(&mut self, refresh_millihertz: i32) {
        if !self
            .outputs
            .set_primary_refresh_millihertz(refresh_millihertz)
        {
            return;
        }

        self.mark_scene_dirty();
    }

    pub fn set_output_scale(&mut self, output: Option<&str>, scale: f64) -> bool {
        let primary = self.output().name();
        let target_is_primary = output.is_none_or(|output| output == primary);
        let Some(changed) = self.outputs.set_scale(output, scale) else {
            return false;
        };
        if !changed {
            return false;
        }

        if !target_is_primary {
            self.mark_scene_dirty();
            return true;
        }

        for surface in self
            .windows
            .iter()
            .map(|window| window.surface.wl_surface())
        {
            self.update_surface_scale(surface);
        }
        layers::arrange(self.output());
        self.apply_active_arrangement();
        self.mark_scene_dirty();
        true
    }

    pub fn set_primary_output_scale(&mut self, scale: f64) {
        let _ = self.set_output_scale(None, scale);
    }

    pub fn enter_output(&self, surface: &WlSurface) {
        self.output().enter(surface);
        self.update_surface_scale(surface);
    }

    pub fn leave_output(&self, surface: &WlSurface) {
        self.output().leave(surface);
    }

    pub fn cleanup_output(&self) {
        self.output().cleanup();
    }
}
