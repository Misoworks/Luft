use super::{BatonState, ShellRestartRequest};
use crate::layout_config::layout_from_config;
use staccato_config::StaccatoConfig;
use staccato_layout::{Rect, WindowId, WindowInfo, WindowState, WorkspaceId};
use tracing::warn;

impl BatonState {
    pub fn send_shell_overview_toggle(&self) {
        self.send_shell_control(
            staccato_ipc::ShellControlRequest::ToggleOverview,
            "overview toggle",
        );
    }

    pub fn send_shell_launcher_open(&self) {
        self.send_shell_control(
            staccato_ipc::ShellControlRequest::OpenLauncher,
            "launcher open",
        );
    }

    pub fn send_shell_default_app_launch(&self, app: staccato_ipc::DefaultAppKind) {
        self.send_shell_control(
            staccato_ipc::ShellControlRequest::LaunchDefaultApp { app },
            "default app launch",
        );
    }

    pub fn request_shell_restart(&mut self) {
        self.shell_restart_requested = Some(ShellRestartRequest::Normal);
    }

    pub fn take_shell_restart_requested(&mut self) -> Option<ShellRestartRequest> {
        self.shell_restart_requested.take()
    }

    pub fn fallback_to_default_config(&mut self) {
        let config = StaccatoConfig::default();
        self.replace_config(config, Some(ShellRestartRequest::DefaultConfig));
    }

    pub fn replace_config(&mut self, config: StaccatoConfig, restart: Option<ShellRestartRequest>) {
        let mut layout = layout_from_config(&config);
        layout.set_bounds(Rect::new(0, 0, self.output_size.w, self.output_size.h));

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
        self.config = config;
        self.layout = layout;
        self.drag = None;
        self.workspace_transition = None;
        if let Some(restart) = restart {
            self.shell_restart_requested = Some(restart);
        }
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

    fn send_shell_control(&self, request: staccato_ipc::ShellControlRequest, action: &str) {
        let Some(path) = &self.shell_control_path else {
            return;
        };

        if let Err(error) = staccato_ipc::send_shell_control_to(path, &request) {
            warn!(%error, path = %path.display(), action, "failed to send shell control request");
        }
    }
}
