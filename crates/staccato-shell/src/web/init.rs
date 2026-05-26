use super::model::WebShellSnapshot;
use super::*;
use crate::{
    apps::{dock_apps, launcher_apps},
    ipc::load_model,
    theme::shell_palette,
};
use std::sync::mpsc::Sender;

impl WebShell {
    pub(super) fn new(
        config: StaccatoConfig,
        actions_tx: Sender<WebShellAction>,
        actions_rx: Receiver<WebShellAction>,
    ) -> Result<Self, Box<dyn Error>> {
        let palette = shell_palette(&config);
        let model = load_model()?;
        let status = SystemStatus::read();
        let chrome = ShellChrome::for_mode(model.active_mode);
        let dock_apps = dock_apps(&config);
        let applications = launcher_apps(&config, &dock_apps);
        let tray = TrayService::start();
        let notifications = NotificationService::start();
        let snapshot = WebShellSnapshot::from_shell(
            &model,
            &status,
            tray.snapshot(),
            notifications.snapshot(),
            &dock_apps,
            None,
            &applications,
            palette,
            false,
        );
        let mut surfaces = WebSurfaces::new(actions_tx, &snapshot, &dock_apps)?;
        surfaces.set_panel_taskbar(model.active_mode == staccato_layout::ModeId::Panel);
        surfaces.set_panel_visible(chrome.panel);
        surfaces.dock.set_visible(chrome.dock);
        surfaces.sidebar.set_visible(chrome.sidebar);

        Ok(Self {
            launcher_command: config.default_apps.launcher.clone(),
            config,
            palette,
            model,
            status,
            chrome,
            tray,
            notifications,
            dock_apps,
            applications,
            surfaces,
            actions_rx,
            control: ShellControlServer::bind_from_env()?,
            app_processes: Vec::new(),
            overview_visible: false,
            quick_visible: false,
            date_visible: false,
            chrome_visibility: ChromeVisibility::default(),
            dock_menu_open: false,
            dock_menu_command: None,
            last_model_refresh: Instant::now(),
            last_status_refresh: Instant::now(),
            last_config_refresh: Instant::now(),
            last_snapshot: String::new(),
        })
    }
}
