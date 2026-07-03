use super::{
    LaunchedProcess, WebShell,
    actions::{QuickSettingsPage, SessionCommand, WebShellAction, window_id, workspace_id},
    settings_command::settings_command,
};
use crate::{
    apps::spawn_command,
    ipc::{move_window_to_workspace, switch_workspace},
    services::system_status::{set_audio_volume, set_brightness, toggle_audio_mute},
};
use tracing::{debug, warn};

impl WebShell {
    pub(super) fn handle_action(&mut self, action: WebShellAction) {
        match action {
            WebShellAction::OpenLauncher => self.open_launcher(),
            WebShellAction::LaunchDefaultApp { app } => self.launch_default_app(app),
            WebShellAction::ToggleStartMenu => self.toggle_start_menu(),
            WebShellAction::CloseStartMenu => self.close_start_menu(),
            WebShellAction::ToggleQuickSettings => self.toggle_quick_settings(),
            WebShellAction::CloseQuickSettings => self.close_quick_settings(),
            WebShellAction::ToggleDateCenter => self.toggle_date_center(),
            WebShellAction::CloseDateCenter => self.close_date_center(),
            WebShellAction::WorkspaceSwitch { workspace } => {
                self.apply_model_result(switch_workspace(workspace_id(workspace)));
                self.close_transient_popovers();
            }
            WebShellAction::WorkspaceRelative { offset } => {
                self.apply_model_result(crate::ipc::switch_relative_workspace(offset))
            }
            WebShellAction::WorkspaceNew => self.new_workspace_from_start_menu(),
            WebShellAction::WindowActivate { window } => self.activate_task_window(window),
            WebShellAction::WindowClose { window } => self.close_task_window(window),
            WebShellAction::WindowMinimize { window } => self.minimize_task_window(window),
            WebShellAction::WindowMove { window, workspace } => self.apply_model_result(
                move_window_to_workspace(window_id(window), workspace_id(workspace)),
            ),
            WebShellAction::PanelLaunch { command } => self.activate_panel_command(command),
            WebShellAction::PanelMenuOpen { command, x } => self.open_panel_menu(command, x),
            WebShellAction::PanelMenuClose => self.close_panel_menu(),
            WebShellAction::PanelPin {
                label,
                command,
                icon,
            } => self.pin_panel_app(label, command, icon),
            WebShellAction::PanelUnpin { command } => self.unpin_panel_app(&command),
            WebShellAction::PanelForceQuit { command } => self.force_quit_panel_app(command),
            WebShellAction::PanelReorder { commands } => self.reorder_panel_apps(commands),
            WebShellAction::AppLaunch { command } => {
                self.close_transient_popovers();
                self.launch(command);
            }
            WebShellAction::TrayActivate { index } => self.activate_tray(index, false),
            WebShellAction::TrayMenu { index } => self.activate_tray(index, true),
            WebShellAction::QuickOpenSettings { page } => self.open_settings_page(page),
            WebShellAction::QuickSetVolume { percent } => {
                if let Err(error) = set_audio_volume(percent) {
                    warn!(%error, "failed to set audio volume");
                }
                self.refresh_status_now();
            }
            WebShellAction::QuickToggleMute => {
                if let Err(error) = toggle_audio_mute() {
                    warn!(%error, "failed to toggle audio mute");
                }
                self.refresh_status_now();
            }
            WebShellAction::QuickSetBrightness { percent } => {
                if let Err(error) = set_brightness(percent) {
                    warn!(%error, "failed to set brightness");
                }
                self.refresh_status_now();
            }
            WebShellAction::SessionCommand { command } => self.run_session_command(command),
            WebShellAction::ReloadConfig => self.reload_config_from_command(),
            WebShellAction::OpenLogsFolder => self.open_logs_folder(),
            WebShellAction::NotificationClose { notification } => {
                self.notifications.close(notification);
            }
            WebShellAction::NotificationClearAll => {
                self.notifications.clear_all();
            }
            WebShellAction::NotificationDoNotDisturb { enabled } => {
                self.notifications.set_do_not_disturb(enabled);
            }
            WebShellAction::NotificationAction {
                notification,
                action,
            } => {
                self.notifications.invoke(notification, action);
            }
        }
    }

    pub(super) fn open_settings_page(&mut self, page: QuickSettingsPage) {
        self.close_transient_popovers();
        let command = settings_command(&self.config.default_apps.settings, page.as_settings_arg());
        if !command.trim().is_empty() {
            self.launch(command);
        }
    }

    pub(super) fn open_launcher(&mut self) {
        self.close_transient_popovers();
        if self.launcher_command.trim().is_empty() {
            return;
        }
        match spawn_command(
            &self.launcher_command,
            self.model.xwayland_display.as_deref(),
        ) {
            Ok(child) => {
                debug!(pid = child.id(), command = %self.launcher_command, "launched app launcher");
                self.app_processes
                    .push(LaunchedProcess::new(self.launcher_command.clone(), child));
            }
            Err(error) => {
                warn!(%error, command = %self.launcher_command, "failed to launch app launcher")
            }
        }
    }

    pub(super) fn launch_default_app(&mut self, app: luft_ipc::DefaultAppKind) {
        if app == luft_ipc::DefaultAppKind::Settings {
            self.open_settings_page(QuickSettingsPage::Appearance);
            return;
        }
        let command = match app {
            luft_ipc::DefaultAppKind::Terminal => self.config.default_apps.terminal.clone(),
            luft_ipc::DefaultAppKind::FileManager => self.config.default_apps.file_manager.clone(),
            luft_ipc::DefaultAppKind::Browser => self.config.default_apps.browser.clone(),
            luft_ipc::DefaultAppKind::Settings => String::new(),
        };
        self.close_transient_popovers();
        if !command.trim().is_empty() {
            self.launch(command);
        }
    }

    pub(super) fn run_session_command(&mut self, command: SessionCommand) {
        let command = match command {
            SessionCommand::Lock => self.config.session.lock_command.clone(),
            SessionCommand::Suspend => self.config.session.suspend_command.clone(),
            SessionCommand::Reboot => self.config.session.reboot_command.clone(),
            SessionCommand::PowerOff => self.config.session.poweroff_command.clone(),
        };
        self.close_transient_popovers();
        match spawn_command(&command, self.model.xwayland_display.as_deref()) {
            Ok(child) => {
                debug!(pid = child.id(), command, "started session command");
                self.app_processes
                    .push(LaunchedProcess::new(command.clone(), child));
            }
            Err(error) => warn!(%error, command, "failed to start session command"),
        }
    }
}
