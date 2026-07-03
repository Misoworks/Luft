use super::{MIN_WINDOW_HEIGHT, MIN_WINDOW_WIDTH};
use crate::state::KestrelState;
use luft_ipc::{Rect, WindowId};
use smithay::utils::{Logical, Size};

impl KestrelState {
    pub fn next_initial_window_geometry(&self) -> Rect {
        self.next_initial_window_geometry_for_size((900, 560).into())
    }

    pub fn next_transient_window_geometry_for_size(
        &self,
        parent_id: WindowId,
        size: Size<i32, Logical>,
    ) -> Rect {
        let Some(parent) = self.windows.window(parent_id) else {
            return self.next_initial_window_geometry_for_size(size);
        };
        let width = size.w.max(MIN_WINDOW_WIDTH);
        let height = size.h.max(MIN_WINDOW_HEIGHT);
        let parent_geometry = parent.geometry();
        let x = parent_geometry.x + (parent_geometry.width - width) / 2 + 24;
        let y = parent_geometry.y + (parent_geometry.height - height) / 2 + 24;
        self.fit_initial_window_geometry(Rect::new(x, y, width, height))
    }

    pub fn next_initial_window_geometry_for_size(&self, size: Size<i32, Logical>) -> Rect {
        let output = self.output_size();
        let reserved_top = self.reserved_top();
        let reserved_bottom = self.reserved_bottom();
        let available_height = (output.h - reserved_top - reserved_bottom).max(MIN_WINDOW_HEIGHT);
        let width = size.w.max(MIN_WINDOW_WIDTH);
        let height = size.h.max(MIN_WINDOW_HEIGHT);
        let visible_windows = self
            .windows
            .iter()
            .filter(|window| !window.hidden && !window.closing)
            .count() as i32;
        let offset = ((visible_windows % 5) - 2) * 24;
        let x = ((output.w - width) / 2 + offset).max(0);
        let y = (reserved_top + (available_height - height) / 2 + offset).max(reserved_top);
        self.fit_initial_window_geometry(Rect::new(x, y, width, height))
    }

    pub fn fit_initial_window_geometry(&self, geometry: Rect) -> Rect {
        let min_x = 0;
        let min_y = self.reserved_top();
        let max_right = self.output_size().w.max(min_x + MIN_WINDOW_WIDTH);
        let max_bottom =
            (self.output_size().h - self.reserved_bottom()).max(min_y + MIN_WINDOW_HEIGHT);
        let max_width = (max_right - min_x).max(MIN_WINDOW_WIDTH);
        let max_height = (max_bottom - min_y).max(MIN_WINDOW_HEIGHT);
        let width = geometry.width.clamp(MIN_WINDOW_WIDTH, max_width);
        let height = geometry.height.clamp(MIN_WINDOW_HEIGHT, max_height);
        let max_x = (max_right - width).max(min_x);
        let max_y = (max_bottom - height).max(min_y);

        Rect::new(
            geometry.x.clamp(min_x, max_x),
            geometry.y.clamp(min_y, max_y),
            width,
            height,
        )
    }
}
