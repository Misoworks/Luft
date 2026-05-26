use super::{BatonState, ShellRestartRequest};
use crate::layout_config::layout_from_config;
use staccato_config::StaccatoConfig;
use staccato_layout::{Rect, WindowInfo, WindowState};
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
        let mut layout = layout_from_config(&config);
        layout.set_bounds(Rect::new(0, 0, self.output_size.w, self.output_size.h));
        let workspace = layout.active_workspace().clone();
        let windows = self
            .windows
            .iter()
            .map(|window| {
                let mut info = WindowInfo::new(window.id, workspace.clone(), window.geometry());
                if window.hidden {
                    info.state = WindowState::Hidden;
                }
                info
            })
            .collect::<Vec<_>>();

        for window in windows {
            let id = window.id;
            let hidden = window.state == WindowState::Hidden;
            if layout.register_window(window).is_ok() && hidden {
                let _ = layout.set_window_state(id, WindowState::Hidden);
            }
        }

        self.windows.set_all_workspace(workspace);
        self.config = config;
        self.layout = layout;
        self.drag = None;
        self.workspace_transition = None;
        self.shell_restart_requested = Some(ShellRestartRequest::DefaultConfig);
        self.apply_active_arrangement();
        self.mark_scene_dirty();
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
