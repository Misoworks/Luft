use super::{CONFIG_REFRESH, MODEL_REFRESH, STATUS_REFRESH, WebShell};
use crate::{
    apps::{launcher_apps, panel_apps},
    ipc::{ShellModel, load_model, reload_config},
    services::system_status::SystemStatus,
    theme::shell_palette,
};
use asher_config::{AsherConfig, load_config, save_config};
use std::{error::Error, time::Instant};
use tracing::{debug, warn};

impl WebShell {
    pub(super) fn apply_model_result(&mut self, result: Result<ShellModel, Box<dyn Error>>) {
        match result {
            Ok(model) => self.apply_model(model),
            Err(error) => warn!(%error, "failed to apply shell action"),
        }
    }

    pub(super) fn refresh_model(&mut self) {
        if self.last_model_refresh.elapsed() < MODEL_REFRESH {
            return;
        }
        self.last_model_refresh = Instant::now();
        match load_model() {
            Ok(model) => self.apply_model(model),
            Err(error) => debug!(%error, "failed to refresh shell model"),
        }
    }

    fn apply_model(&mut self, model: ShellModel) {
        self.model = model;
        super::running_order::sync(&mut self.running_app_order, &self.model);
    }

    pub(super) fn refresh_status(&mut self) {
        if self.last_status_refresh.elapsed() < STATUS_REFRESH {
            return;
        }
        self.refresh_status_now();
    }

    pub(super) fn refresh_status_now(&mut self) {
        self.last_status_refresh = Instant::now();
        self.status = SystemStatus::read();
    }

    pub(super) fn refresh_config(&mut self) {
        if self.last_config_refresh.elapsed() < CONFIG_REFRESH {
            return;
        }
        self.last_config_refresh = Instant::now();
        self.reload_shell_config();
    }

    pub(super) fn reload_shell_config(&mut self) {
        match load_config() {
            Ok(loaded) if loaded.config != self.config => {
                self.apply_shell_config(loaded.config);
            }
            Ok(_) => {}
            Err(error) => debug!(%error, "failed to refresh shell config"),
        }
    }

    pub(super) fn save_shell_config(&mut self, config: AsherConfig) {
        match save_config(&config) {
            Ok(path) => {
                debug!(path = %path.display(), "saved shell config");
                self.apply_model_result(reload_config());
                self.apply_shell_config(config);
            }
            Err(error) => warn!(%error, "failed to save shell config"),
        }
    }

    fn apply_shell_config(&mut self, config: AsherConfig) {
        self.palette = shell_palette(&config);
        self.panel_apps = panel_apps(&config);
        self.applications = launcher_apps(&config, &self.panel_apps);
        self.launcher_command = config.default_apps.launcher.clone();
        self.config = config;
    }

    pub(super) fn sync_surfaces(&mut self) {
        let notification_toast_visible = self.notification_toast_visible();
        let snapshot =
            super::model::WebShellSnapshot::from_shell(super::snapshot::WebShellSnapshotInput {
                model: &self.model,
                running_window_order: &self.running_app_order,
                status: &self.status,
                tray: self.tray.snapshot(),
                notifications: self.notifications.snapshot(),
                panel_apps: &self.panel_apps,
                panel_menu_command: self.panel_menu_command.as_deref(),
                panel_menu_x: self.panel_menu_x,
                applications: &self.applications,
                palette: self.palette,
                start_menu_open: self.start_menu_visible,
                quick_settings_open: self.quick_visible,
                date_center_open: self.date_visible,
            });
        let Ok(json) = serde_json::to_string(&snapshot) else {
            return;
        };
        if json != self.last_snapshot {
            self.last_snapshot = json.clone();
            self.surfaces.evaluate_snapshot(&snapshot, &json);
        }
        self.surfaces
            .set_notification_toast_visible(notification_toast_visible);
    }

    fn notification_toast_visible(&self) -> bool {
        !self.quick_visible
            && !self.date_visible
            && !self.start_menu_visible
            && !self.notifications.snapshot().toast_items.is_empty()
    }
}
