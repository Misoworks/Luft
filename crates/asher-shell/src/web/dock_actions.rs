use super::{LaunchedProcess, WebShell};
use crate::{
    apps::{normalize_launch_command, spawn_command},
    dock::{self, DockApp, dock_app_matches_window},
};
use std::path::Path;
use std::process::Command;
use tracing::{debug, warn};

impl WebShell {
    pub(super) fn activate_dock_command(&mut self, command: String) {
        let command = normalize_launch_command(&command);
        self.close_dock_menu();
        let Some(app) = self
            .dock_apps
            .iter()
            .find(|app| app.command == command)
            .cloned()
        else {
            self.launch(command);
            return;
        };

        if let Some(window) = self.dock_window_for(&app) {
            self.activate_task_window(window.0);
        } else {
            self.launch(app.command);
        }
    }

    pub(super) fn dock_window_for(&self, app: &DockApp) -> Option<asher_layout::WindowId> {
        self.model
            .windows
            .iter()
            .find(|window| window.is_active && dock_app_matches_window(app, window))
            .or_else(|| {
                self.model
                    .windows
                    .iter()
                    .find(|window| window.is_visible && dock_app_matches_window(app, window))
            })
            .or_else(|| {
                self.model
                    .windows
                    .iter()
                    .find(|window| dock_app_matches_window(app, window))
            })
            .map(|window| window.id)
    }

    pub(super) fn pin_dock_app(&mut self, label: String, command: String, icon: Option<String>) {
        let mut config = self.config.clone();
        if dock::pin_app(
            &mut config,
            &self.dock_apps,
            label,
            normalize_launch_command(&command),
            icon,
        ) {
            self.save_shell_config(config);
        }
    }

    pub(super) fn unpin_dock_app(&mut self, command: &str) {
        let mut config = self.config.clone();
        if dock::unpin_app(
            &mut config,
            &self.dock_apps,
            &normalize_launch_command(command),
        ) {
            self.save_shell_config(config);
        }
    }

    pub(super) fn reorder_dock_apps(&mut self, commands: Vec<String>) {
        let mut config = self.config.clone();
        let commands = commands
            .into_iter()
            .map(|command| normalize_launch_command(&command))
            .collect();
        if dock::reorder_apps(&mut config, &self.dock_apps, commands) {
            self.save_shell_config(config);
        }
    }

    pub(super) fn launch(&mut self, command: String) {
        let command = normalize_launch_command(&command);
        match spawn_command(&command, self.model.xwayland_display.as_deref()) {
            Ok(child) => {
                debug!(pid = child.id(), command, "launched dock app");
                self.app_processes
                    .push(LaunchedProcess::new(command.clone(), child));
            }
            Err(error) => warn!(%error, command, "failed to launch dock app"),
        }
    }

    pub(super) fn force_quit_dock_app(&mut self, command: String) {
        let command = normalize_launch_command(&command);
        self.close_dock_menu();

        let pids = self
            .dock_app_for_command(&command)
            .map(|app| self.window_pids_for_dock_app(&app))
            .unwrap_or_default();
        if !pids.is_empty() {
            match Command::new("kill")
                .arg("-TERM")
                .args(pids.iter().map(u32::to_string))
                .spawn()
            {
                Ok(child) => self.app_processes.push(LaunchedProcess::new(
                    format!("kill -TERM {}", format_pids(&pids)),
                    child,
                )),
                Err(error) => warn!(%error, command, "failed to terminate dock app windows"),
            }
            return;
        }

        let Some(program) = command_basename(&command) else {
            return;
        };
        match Command::new("pkill").args(["-TERM", "-x", program]).spawn() {
            Ok(child) => self.app_processes.push(LaunchedProcess::new(
                format!("pkill -TERM -x {program}"),
                child,
            )),
            Err(error) => warn!(%error, command, "failed to force quit dock app"),
        }
    }

    fn dock_app_for_command(&self, command: &str) -> Option<DockApp> {
        self.dock_apps
            .iter()
            .find(|app| app.command == command)
            .cloned()
    }

    fn window_pids_for_dock_app(&self, app: &DockApp) -> Vec<u32> {
        let mut pids = self
            .model
            .windows
            .iter()
            .filter(|window| dock_app_matches_window(app, window))
            .filter_map(|window| window.pid)
            .collect::<Vec<_>>();
        pids.sort_unstable();
        pids.dedup();
        pids
    }

    pub(super) fn open_dock_menu(&mut self, command: String, x: Option<i32>) {
        let command = normalize_launch_command(&command);
        if self.dock_menu_open
            && self.dock_menu_command.as_deref() == Some(command.as_str())
            && self.dock_menu_x == x
        {
            return;
        }
        self.dock_menu_open = true;
        self.dock_menu_command = Some(command);
        self.dock_menu_x = x;
        self.surfaces.set_dock_menu_x(x);
        self.sync_surfaces();
        self.surfaces.set_dock_menu_visible(true);
    }

    pub(super) fn close_dock_menu(&mut self) {
        if !self.dock_menu_open {
            return;
        }
        self.dock_menu_open = false;
        self.surfaces.set_dock_menu_visible(false);
    }

    pub(super) fn activate_tray(&self, index: usize, menu: bool) {
        let Some(item) = self.tray.snapshot().items.get(index) else {
            return;
        };
        if menu {
            self.tray.context_menu(item, 0, 0);
        } else {
            self.tray.activate(item, 0, 0);
        }
    }
}

fn command_basename(command: &str) -> Option<&str> {
    let first = command.split_whitespace().next()?.trim_matches(['"', '\'']);
    Path::new(first).file_name()?.to_str()
}

fn format_pids(pids: &[u32]) -> String {
    pids.iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}
