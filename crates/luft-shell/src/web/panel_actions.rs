use super::{LaunchedProcess, WebShell};
use crate::{
    apps::{normalize_launch_command, spawn_command},
    panel::{self, PanelApp, panel_app_matches_window},
};
use std::path::Path;
use std::process::Command;
use tracing::{debug, warn};

impl WebShell {
    pub(super) fn activate_panel_command(&mut self, command: String) {
        let command = normalize_launch_command(&command);
        self.close_panel_menu();
        if let Some(window) = window_command_id(&command) {
            self.activate_task_window(window);
            return;
        }
        if let Some(group) = window_group_id(&command) {
            if let Some(window) = self.next_group_window(group) {
                self.activate_task_window(window.0);
            }
            return;
        }

        let Some(app) = self.panel_app_for_command(&command) else {
            self.launch(command);
            return;
        };

        if let Some(window) = self.panel_window_for(&app) {
            self.activate_task_window(window.0);
        } else {
            self.launch(app.command);
        }
    }

    pub(super) fn panel_window_for(&self, app: &PanelApp) -> Option<luft_ipc::WindowId> {
        next_window(self.ordered_panel_windows(app)).map(|window| window.id)
    }

    pub(super) fn pin_panel_app(&mut self, label: String, command: String, icon: Option<String>) {
        let mut config = self.config.clone();
        if panel::pin_app(
            &mut config,
            &self.panel_apps,
            label,
            normalize_launch_command(&command),
            icon,
        ) {
            self.save_shell_config(config);
        }
    }

    pub(super) fn unpin_panel_app(&mut self, command: &str) {
        let mut config = self.config.clone();
        if panel::unpin_app(
            &mut config,
            &self.panel_apps,
            &normalize_launch_command(command),
        ) {
            self.save_shell_config(config);
        }
    }

    pub(super) fn reorder_panel_apps(&mut self, commands: Vec<String>) {
        let mut config = self.config.clone();
        let commands = commands
            .into_iter()
            .map(|command| normalize_launch_command(&command))
            .collect();
        if panel::reorder_apps(&mut config, &self.panel_apps, commands) {
            self.save_shell_config(config);
        }
    }

    pub(super) fn launch(&mut self, command: String) {
        let command = normalize_launch_command(&command);
        match spawn_command(&command, self.model.xwayland_display.as_deref()) {
            Ok(child) => {
                debug!(pid = child.id(), command, "launched panel app");
                self.app_processes
                    .push(LaunchedProcess::new(command.clone(), child));
            }
            Err(error) => warn!(%error, command, "failed to launch panel app"),
        }
    }

    pub(super) fn force_quit_panel_app(&mut self, command: String) {
        let command = normalize_launch_command(&command);
        self.close_panel_menu();

        let pids = self
            .panel_app_for_command(&command)
            .map(|app| self.window_pids_for_panel_app(&app))
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
                Err(error) => warn!(%error, command, "failed to terminate panel app windows"),
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
            Err(error) => warn!(%error, command, "failed to force quit panel app"),
        }
    }

    fn panel_app_for_command(&self, command: &str) -> Option<PanelApp> {
        self.panel_apps
            .iter()
            .find(|app| commands_equal(&app.command, command))
            .cloned()
            .or_else(|| {
                self.applications
                    .iter()
                    .find(|app| commands_equal(&app.command, command))
                    .map(|app| {
                        PanelApp::new(app.name.clone(), app.command.clone(), app.icon_path.clone())
                    })
            })
    }

    fn window_pids_for_panel_app(&self, app: &PanelApp) -> Vec<u32> {
        let mut pids = self
            .model
            .windows
            .iter()
            .filter(|window| panel_app_matches_window(app, window))
            .filter_map(|window| window.pid)
            .collect::<Vec<_>>();
        pids.sort_unstable();
        pids.dedup();
        pids
    }

    fn ordered_panel_windows<'a>(&'a self, app: &PanelApp) -> Vec<&'a luft_ipc::WindowSummary> {
        self.ordered_windows()
            .into_iter()
            .filter(|window| panel_app_matches_window(app, window))
            .collect()
    }

    fn next_group_window(&self, group: &str) -> Option<luft_ipc::WindowId> {
        next_window(
            self.ordered_windows()
                .into_iter()
                .filter(|window| window_matches_group(window, group))
                .collect(),
        )
        .map(|window| window.id)
    }

    fn ordered_windows(&self) -> Vec<&luft_ipc::WindowSummary> {
        let mut windows = Vec::new();
        for id in &self.running_app_order {
            if let Some(window) = self.model.windows.iter().find(|window| window.id == *id) {
                windows.push(window);
            }
        }
        for window in &self.model.windows {
            if !windows.iter().any(|ordered| ordered.id == window.id) {
                windows.push(window);
            }
        }
        windows
    }

    pub(super) fn open_panel_menu(&mut self, command: String, x: Option<i32>) {
        let command = normalize_launch_command(&command);
        if self.panel_menu_open
            && self.panel_menu_command.as_deref() == Some(command.as_str())
            && self.panel_menu_x == x
        {
            return;
        }
        self.panel_menu_open = true;
        self.panel_menu_command = Some(command);
        self.panel_menu_x = x;
        self.surfaces.set_panel_menu_x(x);
        self.sync_surfaces();
        self.surfaces.set_panel_menu_visible(true);
    }

    pub(super) fn close_panel_menu(&mut self) {
        if !self.panel_menu_open {
            return;
        }
        self.panel_menu_open = false;
        self.surfaces.set_panel_menu_visible(false);
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

fn window_command_id(command: &str) -> Option<u64> {
    command.strip_prefix("window:")?.parse::<u64>().ok()
}

fn window_group_id(command: &str) -> Option<&str> {
    command.strip_prefix("window-group:")
}

fn next_window(windows: Vec<&luft_ipc::WindowSummary>) -> Option<&luft_ipc::WindowSummary> {
    if let Some(active) = windows.iter().position(|window| window.is_active) {
        if windows.len() > 1 {
            return Some(windows[(active + 1) % windows.len()]);
        }
        return windows.get(active).copied();
    }
    windows
        .iter()
        .find(|window| window.is_visible)
        .copied()
        .or_else(|| windows.first().copied())
}

fn window_matches_group(window: &luft_ipc::WindowSummary, group: &str) -> bool {
    [window.app_id.as_deref(), window.title.as_deref()]
        .into_iter()
        .flatten()
        .map(normalized_identifier)
        .any(|identifier| identifier == group)
}

fn commands_equal(left: &str, right: &str) -> bool {
    normalize_launch_command(left) == normalize_launch_command(right)
}

fn normalized_identifier(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
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
