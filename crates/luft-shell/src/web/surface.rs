use super::{
    lazy_surface::LazyWebSurface,
    web_surface::{WebSurface, WebSurfaceConfig},
};

use super::actions::WebShellAction;
use super::{
    model::{WebShellSnapshot, WebShellSurface},
    surface_layout::{PANEL_HEIGHT, PANEL_WIDTH_HINT},
    surface_sizing::{
        date_center_size, notification_toast_size, panel_menu_size, quick_settings_size,
    },
};
use std::{error::Error, sync::mpsc::Sender};

const START_MENU_WIDTH: i32 = 720;
const START_MENU_HEIGHT: i32 = 640;

pub struct WebSurfaces {
    pub start_menu: LazyWebSurface,
    pub quick: LazyWebSurface,
    pub date: LazyWebSurface,
    pub notification_toast: LazyWebSurface,
    panel_menu: LazyWebSurface,
    panel: WebSurface,
}

impl WebSurfaces {
    pub fn new(
        actions_tx: Sender<WebShellAction>,
        snapshot: &WebShellSnapshot,
    ) -> Result<Self, Box<dyn Error>> {
        let mut surfaces = Self {
            panel: WebSurface::new(WebSurfaceConfig {
                kind: WebShellSurface::Panel,
                size: (PANEL_WIDTH_HINT, PANEL_HEIGHT),
                visible: true,
                keep_alive_when_hidden: false,
                panel_menu_x: None,
                actions_tx: &actions_tx,
                snapshot,
            })?,
            panel_menu: LazyWebSurface::new(
                WebShellSurface::PanelMenu,
                panel_menu_size(snapshot),
                &actions_tx,
                snapshot,
            ),
            start_menu: LazyWebSurface::new(
                WebShellSurface::StartMenu,
                (START_MENU_WIDTH, START_MENU_HEIGHT),
                &actions_tx,
                snapshot,
            ),
            quick: LazyWebSurface::new(
                WebShellSurface::QuickSettings,
                quick_settings_size(snapshot),
                &actions_tx,
                snapshot,
            ),
            date: LazyWebSurface::new(
                WebShellSurface::DateCenter,
                date_center_size(snapshot),
                &actions_tx,
                snapshot,
            ),
            notification_toast: LazyWebSurface::new(
                WebShellSurface::NotificationToast,
                notification_toast_size(snapshot),
                &actions_tx,
                snapshot,
            ),
        };
        surfaces.quick.prewarm();
        surfaces.date.prewarm();
        surfaces.start_menu.prewarm();
        surfaces.panel_menu.prewarm();
        surfaces.notification_toast.prewarm();
        Ok(surfaces)
    }

    pub fn evaluate_snapshot(&mut self, snapshot: &WebShellSnapshot, json: &str) {
        self.panel.evaluate_snapshot(snapshot, json);
        self.panel_menu.resize(panel_menu_size(snapshot));
        self.panel_menu.evaluate_snapshot(snapshot, json);
        self.start_menu.evaluate_snapshot(snapshot, json);
        self.quick.resize(quick_settings_size(snapshot));
        self.quick.evaluate_snapshot(snapshot, json);
        self.date.resize(date_center_size(snapshot));
        self.date.evaluate_snapshot(snapshot, json);
        self.notification_toast
            .resize(notification_toast_size(snapshot));
        self.notification_toast.evaluate_snapshot(snapshot, json);
    }

    pub fn set_panel_visible(&mut self, visible: bool) {
        self.panel.set_visible(visible);
    }

    pub fn set_panel_menu_visible(&mut self, visible: bool) {
        self.panel_menu.set_visible(visible);
    }

    pub fn set_panel_menu_x(&mut self, x: Option<i32>) {
        self.panel_menu.set_panel_menu_x(x);
    }

    pub fn set_notification_toast_visible(&mut self, visible: bool) {
        self.notification_toast.set_visible(visible);
    }

    pub fn tick(&mut self) {
        self.panel_menu.tick();
        self.start_menu.tick();
        self.quick.tick();
        self.date.tick();
        self.notification_toast.tick();
    }

    pub fn is_animating(&self) -> bool {
        self.panel_menu.is_animating()
            || self.start_menu.is_animating()
            || self.quick.is_animating()
            || self.date.is_animating()
            || self.notification_toast.is_animating()
    }
}
