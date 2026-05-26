use crate::state::BatonState;
use smithay::input::pointer::{CursorIcon, CursorImageStatus};

impl BatonState {
    pub(crate) fn set_frame_cursor(&mut self, icon: CursorIcon) {
        self.frame_cursor_active = true;
        if matches!(&self.cursor_image, CursorImageStatus::Named(current) if *current == icon) {
            return;
        }

        self.cursor_image = CursorImageStatus::Named(icon);
        self.cursor_dirty = true;
    }

    pub(crate) fn clear_frame_cursor(&mut self) {
        if !self.frame_cursor_active {
            return;
        }

        self.frame_cursor_active = false;
        self.cursor_image = CursorImageStatus::Named(CursorIcon::Default);
        self.cursor_dirty = true;
    }
}
