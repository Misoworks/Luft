use super::{
    lazy_surface::LazyWebSurface,
    web_surface::{WebSurface, WebSurfaceConfig},
};

use super::actions::WebShellAction;
use super::{
    model::{WebShellSnapshot, WebShellSurface},
    surface_layout::{PANEL_HEIGHT, PANEL_WIDTH_HINT},
    surface_sizing::{dock_menu_size, notification_toast_size, quick_settings_size},
};
use crate::dock::DockApp;
use std::{error::Error, sync::mpsc::Sender};

const DATE_CENTER_WIDTH: i32 = 360;
const DATE_CENTER_HEIGHT: i32 = 560;
const START_MENU_WIDTH: i32 = 720;
const START_MENU_HEIGHT: i32 = 640;

pub struct WebSurfaces {
    pub dock: WebSurface,
    pub sidebar: LazyWebSurface,
    pub start_menu: LazyWebSurface,
    pub quick: LazyWebSurface,
    pub date: LazyWebSurface,
    pub notification_toast: LazyWebSurface,
    dock_menu: LazyWebSurface,
    panel: WebSurface,
}

impl WebSurfaces {
    pub fn new(
        actions_tx: Sender<WebShellAction>,
        snapshot: &WebShellSnapshot,
        dock_apps: &[DockApp],
        dock_icon_size: u16,
        panel_taskbar: bool,
    ) -> Result<Self, Box<dyn Error>> {
        let mut surfaces = Self {
            panel: WebSurface::new(WebSurfaceConfig {
                kind: WebShellSurface::Panel,
                size: (PANEL_WIDTH_HINT, PANEL_HEIGHT),
                visible: true,
                keep_alive_when_hidden: false,
                panel_taskbar,
                dock_menu_x: None,
                actions_tx: &actions_tx,
                snapshot,
            })?,
            dock: WebSurface::new(WebSurfaceConfig {
                kind: WebShellSurface::Dock,
                size: super::surface_layout::dock_size(dock_apps, dock_icon_size),
                visible: true,
                keep_alive_when_hidden: false,
                panel_taskbar: false,
                dock_menu_x: None,
                actions_tx: &actions_tx,
                snapshot,
            })?,
            dock_menu: LazyWebSurface::new(
                WebShellSurface::DockMenu,
                dock_menu_size(snapshot),
                panel_taskbar,
                &actions_tx,
                snapshot,
            ),
            sidebar: LazyWebSurface::new(
                WebShellSurface::Sidebar,
                (108, 1),
                false,
                &actions_tx,
                snapshot,
            ),
            start_menu: LazyWebSurface::new(
                WebShellSurface::StartMenu,
                (START_MENU_WIDTH, START_MENU_HEIGHT),
                panel_taskbar,
                &actions_tx,
                snapshot,
            ),
            quick: LazyWebSurface::new(
                WebShellSurface::QuickSettings,
                quick_settings_size(snapshot),
                panel_taskbar,
                &actions_tx,
                snapshot,
            ),
            date: LazyWebSurface::new(
                WebShellSurface::DateCenter,
                (DATE_CENTER_WIDTH, DATE_CENTER_HEIGHT),
                panel_taskbar,
                &actions_tx,
                snapshot,
            ),
            notification_toast: LazyWebSurface::new(
                WebShellSurface::NotificationToast,
                notification_toast_size(snapshot),
                panel_taskbar,
                &actions_tx,
                snapshot,
            ),
        };
        surfaces.sidebar.prewarm();
        surfaces.quick.prewarm();
        surfaces.date.prewarm();
        surfaces.start_menu.prewarm();
        surfaces.dock_menu.prewarm();
        surfaces.notification_toast.prewarm();
        Ok(surfaces)
    }

    pub fn evaluate_snapshot(&mut self, snapshot: &WebShellSnapshot, json: &str) {
        self.panel.evaluate_snapshot(snapshot, json);
        self.dock.evaluate_snapshot(snapshot, json);
        self.dock_menu.resize(dock_menu_size(snapshot));
        self.dock_menu.evaluate_snapshot(snapshot, json);
        self.sidebar.evaluate_snapshot(snapshot, json);
        self.start_menu.evaluate_snapshot(snapshot, json);
        self.quick.resize(quick_settings_size(snapshot));
        self.quick.evaluate_snapshot(snapshot, json);
        self.date.evaluate_snapshot(snapshot, json);
        self.notification_toast
            .resize(notification_toast_size(snapshot));
        self.notification_toast.evaluate_snapshot(snapshot, json);
    }

    pub fn set_panel_visible(&mut self, visible: bool) {
        self.panel.set_visible(visible);
    }

    pub fn set_panel_taskbar(&mut self, taskbar: bool) {
        self.panel.set_panel_taskbar(taskbar);
        self.dock_menu.set_panel_taskbar(taskbar);
        self.quick.set_panel_taskbar(taskbar);
        self.date.set_panel_taskbar(taskbar);
        self.start_menu.set_panel_taskbar(taskbar);
        self.notification_toast.set_panel_taskbar(taskbar);
    }

    pub fn resize_dock(&mut self, apps: &[DockApp], icon_size: u16) {
        self.dock
            .resize(super::surface_layout::dock_size(apps, icon_size));
    }

    pub fn set_dock_menu_visible(&mut self, visible: bool) {
        self.dock_menu.set_visible(visible);
    }

    pub fn set_dock_menu_x(&mut self, x: Option<i32>) {
        self.dock_menu.set_dock_menu_x(x);
    }

    pub fn set_notification_toast_visible(&mut self, visible: bool) {
        self.notification_toast.set_visible(visible);
    }

    pub fn tick(&mut self) {
        self.dock_menu.tick();
        self.sidebar.tick();
        self.start_menu.tick();
        self.quick.tick();
        self.date.tick();
        self.notification_toast.tick();
    }

    pub fn is_animating(&self) -> bool {
        self.dock_menu.is_animating()
            || self.sidebar.is_animating()
            || self.start_menu.is_animating()
            || self.quick.is_animating()
            || self.date.is_animating()
            || self.notification_toast.is_animating()
    }
}
