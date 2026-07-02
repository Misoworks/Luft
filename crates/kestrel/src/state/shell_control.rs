use super::KestrelState;
use crate::layout_config::layout_from_config;
use asher_ipc::{Rect, WindowId, WindowInfo, WindowState, WorkspaceId};
use tracing::warn;

impl KestrelState {
    pub fn send_shell_start_menu_toggle(&self) {
        self.send_shell_control(
            asher_ipc::ShellControlRequest::ToggleStartMenu,
            "start menu toggle",
        );
    }

    pub fn send_shell_launcher_open(&self) {
        self.send_shell_control(
            asher_ipc::ShellControlRequest::OpenLauncher,
            "launcher open",
        );
    }

    pub fn send_shell_default_app_launch(&self, app: asher_ipc::DefaultAppKind) {
        self.send_shell_control(
            asher_ipc::ShellControlRequest::LaunchDefaultApp { app },
            "default app launch",
        );
    }

    pub fn close_shell_transient_popovers(&self) {
        self.send_shell_control(
            asher_ipc::ShellControlRequest::CloseTransientPopovers,
            "close transient popovers",
        );
    }

    pub fn request_shell_restart(&mut self) {
        self.shell_restart_requested = true;
    }

    pub fn take_shell_restart_requested(&mut self) -> bool {
        std::mem::take(&mut self.shell_restart_requested)
    }

    pub fn replace_config(&mut self, config: asher_config::AsherConfig) {
        let mut layout = layout_from_config(&config);
        layout.set_bounds(Rect::new(0, 0, self.output_size().w, self.output_size().h));

        let active = self.layout.active_workspace().clone();
        if layout.workspaces().any(|workspace| workspace.id == active) {
            let _ = layout.switch_workspace(&active);
        }

        let available_workspaces = layout
            .workspaces()
            .map(|workspace| workspace.id.clone())
            .collect::<Vec<_>>();
        let fallback_workspace = layout.active_workspace().clone();
        let windows = self
            .windows
            .iter()
            .map(|window| {
                let workspace = if available_workspaces.contains(&window.workspace) {
                    window.workspace.clone()
                } else {
                    fallback_workspace.clone()
                };
                let mut info = WindowInfo::new(window.id, workspace, window.geometry());
                if window.hidden {
                    info.state = WindowState::Hidden;
                }
                info
            })
            .collect::<Vec<_>>();

        for window in &windows {
            let id = window.id;
            let hidden = window.state == WindowState::Hidden;
            if layout.register_window(window.clone()).is_ok() && hidden {
                let _ = layout.set_window_state(id, WindowState::Hidden);
            }
        }

        self.reassign_window_stack(&windows);
        let output_scale = config.display.output_scale(&self.output().name());
        self.config = config;
        self.layout = layout;
        self.set_primary_output_scale(output_scale);
        self.drag = None;
        self.workspace_transition = None;
        self.apply_active_arrangement();
        self.mark_scene_dirty();
    }

    fn reassign_window_stack(&mut self, windows: &[WindowInfo]) {
        for window in windows {
            self.set_window_workspace(window.id, window.workspace.clone());
        }
    }

    fn set_window_workspace(&mut self, id: WindowId, workspace: WorkspaceId) {
        let _ = self.windows.set_workspace(id, workspace);
    }

    fn send_shell_control(&self, request: asher_ipc::ShellControlRequest, action: &str) {
        let Some(path) = &self.shell_control_path else {
            return;
        };

        if let Err(error) = asher_ipc::send_shell_control_to(path, &request) {
            warn!(%error, path = %path.display(), action, "failed to send shell control request");
        }
    }
}
