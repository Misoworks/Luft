use super::{WebShell, actions::window_id};
use crate::ipc::{activate_window, close_window, minimize_window, switch_relative_workspace};

impl WebShell {
    pub(super) fn new_workspace_from_start_menu(&mut self) {
        let previous = self.model.active_workspace.clone();
        self.apply_model_result(switch_relative_workspace(1));
        if self.model.active_workspace != previous {
            self.close_transient_popovers();
        }
    }

    pub(super) fn activate_task_window(&mut self, window: u64) {
        let id = window_id(window);
        let result = if self
            .model
            .windows
            .iter()
            .any(|summary| summary.id == id && summary.is_active && summary.is_visible)
        {
            minimize_window(id)
        } else {
            activate_window(id)
        };
        self.apply_model_result(result);
        self.close_transient_popovers();
    }

    pub(super) fn close_task_window(&mut self, window: u64) {
        self.apply_model_result(close_window(window_id(window)));
        self.close_panel_menu();
    }

    pub(super) fn minimize_task_window(&mut self, window: u64) {
        self.apply_model_result(minimize_window(window_id(window)));
        self.close_panel_menu();
    }
}
