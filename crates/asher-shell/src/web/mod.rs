use crate::{
    apps::AppEntry,
    chrome::ShellChrome,
    control::ShellControlServer,
    dock::DockApp,
    ipc::ShellModel,
    services::{
        notifications::NotificationService,
        system_status::SystemStatus,
        tray::TrayService,
    },
    theme::ShellPalette,
};
mod action_dispatch;
mod actions;
mod appearance;
mod command_actions;
mod dock_actions;
mod icons;
mod init;
mod launched_process;
mod model;
mod palette;
mod popover_actions;
mod lazy_surface;
mod settings_command;
mod surface;
mod surface_layout;
mod surface_motion;
mod surface_sizing;
mod sync;
mod wallpaper;
mod web_surface;
mod window_actions;
use actions::WebShellAction;
use asher_config::AsherConfig;
use launched_process::LaunchedProcess;
use std::{
    cell::RefCell,
    error::Error,
    rc::Rc,
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};
use surface::WebSurfaces;
use tracing::warn;

const MODEL_REFRESH: Duration = Duration::from_millis(500);
const STATUS_REFRESH: Duration = Duration::from_secs(1);
const CONFIG_REFRESH: Duration = Duration::from_secs(2);
const ACTION_TICK: Duration = Duration::from_millis(16);
const MAINTENANCE_TICK: Duration = Duration::from_millis(100);
const OUTPUT_REFRESH_ENV: &str = "ASHER_OUTPUT_REFRESH_MILLIHERTZ";

pub fn run(config: AsherConfig) -> Result<(), Box<dyn Error>> {
    let (actions_tx, actions_rx) = mpsc::channel();
    let shell = Rc::new(RefCell::new(WebShell::new(config, actions_tx, actions_rx)?));
    shell.borrow_mut().sync_surfaces();

    let animation_tick = animation_tick_interval();
    let mut last_maintenance = Instant::now();
    loop {
        let animating = {
            let mut shell = shell.borrow_mut();
            shell.tick_actions();
            if last_maintenance.elapsed() >= MAINTENANCE_TICK {
                shell.tick();
                last_maintenance = Instant::now();
            }
            shell.surfaces.is_animating()
        };
        thread::sleep(if animating {
            animation_tick
        } else {
            ACTION_TICK
        });
    }
}

fn animation_tick_interval() -> Duration {
    let millihertz = std::env::var(OUTPUT_REFRESH_ENV)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|refresh| *refresh > 0)
        .unwrap_or(60_000);
    Duration::from_nanos((1_000_000_000_000u64 + millihertz / 2) / millihertz)
}

pub(super) struct WebShell {
    pub(super) config: AsherConfig,
    pub(super) palette: ShellPalette,
    pub(super) wallpaper_uri: Option<String>,
    pub(super) glass_blur_wallpaper_uri: Option<String>,
    pub(super) model: ShellModel,
    pub(super) status: SystemStatus,
    pub(super) chrome: ShellChrome,
    pub(super) tray: TrayService,
    pub(super) notifications: NotificationService,
    pub(super) dock_apps: Vec<DockApp>,
    pub(super) applications: Vec<AppEntry>,
    pub(super) surfaces: WebSurfaces,
    actions_rx: Receiver<WebShellAction>,
    control: Option<ShellControlServer>,
    pub(super) app_processes: Vec<LaunchedProcess>,
    pub(super) launcher_command: String,
    pub(super) start_menu_visible: bool,
    pub(super) quick_visible: bool,
    pub(super) date_visible: bool,
    pub(super) dock_menu_open: bool,
    pub(super) dock_menu_command: Option<String>,
    pub(super) dock_menu_x: Option<i32>,
    last_model_refresh: Instant,
    last_status_refresh: Instant,
    last_config_refresh: Instant,
    last_snapshot: String,
}

impl WebShell {
    fn tick_actions(&mut self) {
        let pending_actions: Vec<WebShellAction> = self.actions_rx.try_iter().collect();

        let mut handled_action = false;
        for action in pending_actions {
            handled_action = true;
            self.handle_action(action);
        }

        self.handle_control_requests();

        if handled_action
            || self.start_menu_visible
            || self.quick_visible
            || self.date_visible
        {
            self.sync_chrome();
            self.sync_surfaces();
        }
        self.surfaces.tick();
    }

    fn tick(&mut self) {
        self.tick_actions();

        self.app_processes
            .retain_mut(LaunchedProcess::is_running_or_report_exit);
        self.tray.refresh();
        self.notifications.refresh();
        self.refresh_model();
        self.refresh_status();
        self.refresh_config();
        self.sync_chrome();
        self.sync_surfaces();
    }

    fn handle_control_requests(&mut self) {
        let Some(control) = &self.control else {
            return;
        };

        match control.drain() {
            Ok(requests) => {
                for request in requests {
                    match request {
                        asher_ipc::ShellControlRequest::LaunchDefaultApp { app } => {
                            self.launch_default_app(app)
                        }
                        asher_ipc::ShellControlRequest::OpenLauncher => self.open_launcher(),
                        asher_ipc::ShellControlRequest::ToggleStartMenu => self.toggle_start_menu(),
                        asher_ipc::ShellControlRequest::CloseTransientPopovers => {
                            self.close_transient_popovers()
                        }
                    }
                }
            }
            Err(error) => warn!(%error, "failed to read shell control request"),
        }
    }
}
