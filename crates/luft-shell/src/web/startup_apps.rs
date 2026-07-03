use super::{LaunchedProcess, WebShell};
use crate::apps::{discover_user_autostart, normalize_launch_command, spawn_command};
use std::collections::HashSet;
use std::time::Instant;
use tracing::{debug, warn};

impl WebShell {
    pub(super) fn launch_startup_apps(&mut self) {
        if self.startup_apps_launched || Instant::now() < self.startup_apps_launch_after {
            return;
        }

        self.startup_apps_launched = true;
        let commands = self.startup_commands();
        for command in commands {
            match spawn_command(&command, self.model.xwayland_display.as_deref()) {
                Ok(child) => {
                    debug!(pid = child.id(), command, "launched startup app");
                    self.app_processes
                        .push(LaunchedProcess::new(command, child));
                }
                Err(error) => warn!(%error, command, "failed to launch startup app"),
            }
        }
    }

    fn startup_commands(&mut self) -> Vec<String> {
        let configured = std::mem::take(&mut self.startup_apps);
        let autostart = discover_user_autostart(&self.config)
            .into_iter()
            .map(|app| app.command);
        let mut seen = HashSet::new();
        configured
            .into_iter()
            .chain(autostart)
            .map(|command| normalize_launch_command(&command))
            .filter(|command| !command.trim().is_empty())
            .filter(|command| seen.insert(command.clone()))
            .collect()
    }
}
