use super::WebShell;
use crate::{apps::spawn_command, ipc::reload_config};
use luft_config::ConfigPaths;
use tracing::warn;

impl WebShell {
    pub(super) fn reload_config_from_command(&mut self) {
        self.apply_model_result(reload_config());
        self.reload_shell_config();
    }

    pub(super) fn open_logs_folder(&mut self) {
        let Ok(paths) = ConfigPaths::discover() else {
            return;
        };
        let command = format!("xdg-open {}", paths.logs_dir().display());
        if let Err(error) = spawn_command(&command, self.model.xwayland_display.as_deref()) {
            warn!(%error, "failed to open Luft logs folder");
        }
    }
}
