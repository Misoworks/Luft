use super::{CONFIG_REFRESH, MODEL_REFRESH, STATUS_REFRESH, WebShell};
use crate::{
    apps::{dock_apps, launcher_apps},
    chrome::ShellChrome,
    ipc::{ShellModel, load_model, reload_config},
    services::system_status::SystemStatus,
    theme::shell_palette,
};
use staccato_config::{StaccatoConfig, load_config, save_config};
use std::{error::Error, time::Instant};
use tracing::{debug, warn};

impl WebShell {
    pub(super) fn apply_model_result(&mut self, result: Result<ShellModel, Box<dyn Error>>) {
        match result {
            Ok(model) => self.model = model,
            Err(error) => warn!(%error, "failed to apply shell action"),
        }
    }

    pub(super) fn refresh_model(&mut self) {
        if self.last_model_refresh.elapsed() < MODEL_REFRESH {
            return;
        }
        self.last_model_refresh = Instant::now();
        match load_model() {
            Ok(model) => self.model = model,
            Err(error) => debug!(%error, "failed to refresh shell model"),
        }
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

    pub(super) fn save_shell_config(&mut self, config: StaccatoConfig) {
        match save_config(&config) {
            Ok(path) => {
                debug!(path = %path.display(), "saved shell config");
                self.apply_model_result(reload_config());
                self.apply_shell_config(config);
            }
            Err(error) => warn!(%error, "failed to save shell config"),
        }
    }

    fn apply_shell_config(&mut self, config: StaccatoConfig) {
        self.palette = shell_palette(&config);
        self.wallpaper_uri = super::model::wallpaper_uri(&config);
        self.dock_apps = dock_apps(&config);
        self.applications = launcher_apps(&config, &self.dock_apps);
        self.launcher_command = config.default_apps.launcher.clone();
        self.config = config;
        self.surfaces.resize_dock(&self.dock_apps);
        self.sync_chrome();
    }

    pub(super) fn sync_chrome(&mut self) {
        let chrome = ShellChrome::for_mode(self.model.active_mode);
        let changed = chrome != self.chrome;
        self.chrome = chrome;
        self.surfaces
            .set_panel_taskbar(self.model.active_mode == staccato_layout::ModeId::Panel);
        let chrome_mapped = self.chrome_visibility.mapped(self.overview_visible, true);
        self.surfaces
            .set_panel_visible(chrome.panel && chrome_mapped);
        self.surfaces.dock.set_visible(chrome.dock && chrome_mapped);
        let dock_menu_supported =
            chrome.dock || self.model.active_mode == staccato_layout::ModeId::Panel;
        if !dock_menu_supported || self.overview_visible {
            self.close_dock_menu();
        }
        self.surfaces.sidebar.set_visible(chrome.sidebar);
        if changed && !chrome.panel {
            self.quick_visible = false;
            self.date_visible = false;
            self.surfaces.quick.set_visible(false);
            self.surfaces.date.set_visible(false);
        }
    }

    pub(super) fn sync_surfaces(&mut self) {
        let notification_toast_visible = self.notification_toast_visible();
        let snapshot = super::model::WebShellSnapshot::from_shell(
            &self.model,
            &self.status,
            self.tray.snapshot(),
            self.notifications.snapshot(),
            &self.dock_apps,
            self.dock_menu_command.as_deref(),
            &self.applications,
            self.wallpaper_uri.clone(),
            self.palette,
            self.config.general.safe_mode,
            self.overview_visible,
        );
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
            && !self.overview_visible
            && !self.notifications.snapshot().toast_items.is_empty()
    }
}
