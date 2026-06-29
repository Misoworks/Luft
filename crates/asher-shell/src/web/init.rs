use super::*;
use super::{model::WebShellSnapshot, snapshot::WebShellSnapshotInput};
use crate::{
    apps::{dock_apps, launcher_apps},
    ipc::load_model,
    theme::shell_palette,
};
use std::sync::mpsc::Sender;

impl WebShell {
    pub(super) fn new(
        config: AsherConfig,
        actions_tx: Sender<WebShellAction>,
        actions_rx: Receiver<WebShellAction>,
    ) -> Result<Self, Box<dyn Error>> {
        let palette = shell_palette(&config);
        let wallpaper_uri = super::wallpaper::wallpaper_uri(&config);
        let glass_blur_wallpaper_uri = super::wallpaper::glass_blur_wallpaper_uri(&config);
        let model = load_model()?;
        let status = SystemStatus::read();
        let chrome = ShellChrome::for_mode(model.active_mode);
        let dock_apps = dock_apps(&config);
        let applications = launcher_apps(&config, &dock_apps);
        let tray = TrayService::start();
        let notifications = NotificationService::start();
        let snapshot = WebShellSnapshot::from_shell(WebShellSnapshotInput {
            model: &model,
            status: &status,
            tray: tray.snapshot(),
            notifications: notifications.snapshot(),
            dock_apps: &dock_apps,
            dock_menu_command: None,
            dock_menu_x: None,
            applications: &applications,
            wallpaper_uri: wallpaper_uri.clone(),
            glass_blur_wallpaper_uri: glass_blur_wallpaper_uri.clone(),
            palette,
            config: &config,
            safe_mode: config.general.safe_mode,
            start_menu_open: false,
            quick_settings_open: false,
            date_center_open: false,
        });
        let mut surfaces = WebSurfaces::new(
            actions_tx,
            &snapshot,
            &dock_apps,
            config.appearance.dock_icon_size,
            model.active_mode == asher_layout::ModeId::Panel,
        )?;
        surfaces.set_panel_visible(chrome.panel);
        surfaces.dock.set_visible(chrome.dock);
        surfaces.sidebar.set_visible(chrome.sidebar);

        Ok(Self {
            launcher_command: config.default_apps.launcher.clone(),
            config,
            palette,
            wallpaper_uri,
            glass_blur_wallpaper_uri,
            model,
            status,
            chrome,
            tray,
            notifications,
            dock_apps,
            applications,
            surfaces,
            actions_rx,
            queued_actions: Default::default(),
            control: ShellControlServer::bind_from_env()?,
            app_processes: Vec::new(),
            start_menu_visible: false,
            quick_visible: false,
            date_visible: false,
            dock_menu_open: false,
            dock_menu_command: None,
            dock_menu_x: None,
            last_model_refresh: Instant::now(),
            last_status_refresh: Instant::now(),
            last_config_refresh: Instant::now(),
            last_snapshot: String::new(),
        })
    }
}
