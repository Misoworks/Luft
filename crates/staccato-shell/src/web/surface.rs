use crate::dock::DockApp;

use super::{
    actions::WebShellAction,
    model::{WebShellSnapshot, WebShellSurface},
    surface_layout::{
        PANEL_HEIGHT, PANEL_WIDTH_HINT, configure_content_size, configure_panel_window,
        configure_popover_window, configure_window, fixed_size, panel_size,
    },
};
use gtk::glib;
use gtk::prelude::*;
use std::{cell::Cell, env, error::Error, rc::Rc, sync::mpsc::Sender, time::Duration};
use tracing::{debug, warn};
use webkit2gtk::{HardwareAccelerationPolicy, LoadEvent, SettingsExt, WebViewExt};
use wry::{WebView, WebViewBuilder, WebViewBuilderExtUnix, WebViewExtUnix};

const DOCK_MENU_WIDTH: i32 = 184;
const DOCK_MENU_HEIGHT: i32 = 128;
const SURFACE_EXIT_DURATION: Duration = Duration::from_millis(70);
const SURFACE_OPEN_SCRIPT: &str = r#"
(() => {
  const app = document.getElementById("app");
  if (!app) return;
  app.classList.remove("is-surface-closing");
  app.classList.remove("is-surface-opening");
  void app.offsetWidth;
  app.classList.add("is-surface-opening");
  window.clearTimeout(window.__staccatoSurfaceAnimation);
  window.__staccatoSurfaceAnimation = window.setTimeout(() => {
    app.classList.remove("is-surface-opening");
  }, 130);
})();
"#;
const SURFACE_CLOSE_SCRIPT: &str = r#"
(() => {
  const app = document.getElementById("app");
  if (!app) return;
  app.classList.remove("is-surface-opening");
  app.classList.add("is-surface-closing");
})();
"#;

pub struct WebSurfaces {
    pub dock: WebSurface,
    pub sidebar: LazyWebSurface,
    pub overview: LazyWebSurface,
    pub quick: LazyWebSurface,
    pub date: LazyWebSurface,
    dock_menu: LazyWebSurface,
    panel: WebSurface,
}

impl WebSurfaces {
    pub fn new(
        actions_tx: Sender<WebShellAction>,
        snapshot: &WebShellSnapshot,
        dock_apps: &[DockApp],
    ) -> Result<Self, Box<dyn Error>> {
        let mut surfaces = Self {
            panel: WebSurface::new(
                WebShellSurface::Panel,
                (PANEL_WIDTH_HINT, PANEL_HEIGHT),
                true,
                &actions_tx,
                snapshot,
            )?,
            dock: WebSurface::new(
                WebShellSurface::Dock,
                dock_size(dock_apps),
                true,
                &actions_tx,
                snapshot,
            )?,
            dock_menu: LazyWebSurface::new(
                WebShellSurface::DockMenu,
                (DOCK_MENU_WIDTH, DOCK_MENU_HEIGHT),
                &actions_tx,
                snapshot,
            ),
            sidebar: LazyWebSurface::new(WebShellSurface::Sidebar, (108, 1), &actions_tx, snapshot),
            overview: LazyWebSurface::new(WebShellSurface::Overview, (1, 1), &actions_tx, snapshot),
            quick: LazyWebSurface::new(
                WebShellSurface::QuickSettings,
                (390, 350),
                &actions_tx,
                snapshot,
            ),
            date: LazyWebSurface::new(
                WebShellSurface::DateCenter,
                (760, 392),
                &actions_tx,
                snapshot,
            ),
        };
        surfaces.overview.ensure_created();
        surfaces.quick.ensure_created();
        surfaces.date.ensure_created();
        surfaces.dock_menu.ensure_created();
        Ok(surfaces)
    }

    pub fn evaluate_snapshot(&mut self, snapshot: &WebShellSnapshot, json: &str) {
        self.panel.evaluate_snapshot(json);
        self.dock.evaluate_snapshot(json);
        self.dock_menu.evaluate_snapshot(snapshot, json);
        self.sidebar.evaluate_snapshot(snapshot, json);
        self.overview.evaluate_snapshot(snapshot, json);
        self.quick.evaluate_snapshot(snapshot, json);
        self.date.evaluate_snapshot(snapshot, json);
    }

    pub fn set_panel_visible(&mut self, visible: bool) {
        self.panel.set_visible(visible);
    }

    pub fn set_panel_taskbar(&mut self, taskbar: bool) {
        self.panel.set_panel_taskbar(taskbar);
        self.dock_menu.set_panel_taskbar(taskbar);
        self.quick.set_panel_taskbar(taskbar);
        self.date.set_panel_taskbar(taskbar);
    }

    pub fn resize_dock(&mut self, apps: &[DockApp]) {
        self.dock.resize(dock_size(apps));
    }

    pub fn set_dock_menu_visible(&mut self, visible: bool) {
        self.dock_menu.set_visible(visible);
    }
}

pub struct LazyWebSurface {
    kind: WebShellSurface,
    size: (i32, i32),
    actions_tx: Sender<WebShellAction>,
    snapshot: WebShellSnapshot,
    snapshot_json: String,
    visible: bool,
    panel_taskbar: bool,
    surface: Option<WebSurface>,
}

impl LazyWebSurface {
    fn new(
        kind: WebShellSurface,
        size: (i32, i32),
        actions_tx: &Sender<WebShellAction>,
        snapshot: &WebShellSnapshot,
    ) -> Self {
        Self {
            kind,
            size,
            actions_tx: actions_tx.clone(),
            snapshot: snapshot.clone(),
            snapshot_json: serde_json::to_string(snapshot).unwrap_or_default(),
            visible: false,
            panel_taskbar: false,
            surface: None,
        }
    }

    pub fn set_visible(&mut self, visible: bool) {
        if !visible {
            self.visible = false;
            if let Some(surface) = &mut self.surface {
                surface.set_visible(false);
            }
            return;
        }

        if self.surface.is_none() {
            self.ensure_created();
            if self.surface.is_none() {
                return;
            }
        }

        self.visible = true;
        if let Some(surface) = &mut self.surface {
            surface.set_visible(true);
        }
    }

    fn ensure_created(&mut self) {
        if self.surface.is_some() {
            return;
        }
        match WebSurface::new(
            self.kind,
            self.size,
            false,
            &self.actions_tx,
            &self.snapshot,
        ) {
            Ok(mut surface) => {
                surface.set_panel_taskbar(self.panel_taskbar);
                surface.evaluate_snapshot(&self.snapshot_json);
                self.surface = Some(surface);
            }
            Err(error) => {
                warn!(%error, surface = self.kind.as_str(), "failed to create web shell surface");
            }
        }
    }

    fn evaluate_snapshot(&mut self, snapshot: &WebShellSnapshot, json: &str) {
        self.snapshot = snapshot.clone();
        if self.snapshot_json != json {
            self.snapshot_json = json.to_string();
        }
        if let Some(surface) = &mut self.surface {
            surface.evaluate_snapshot(json);
        }
    }

    fn set_panel_taskbar(&mut self, taskbar: bool) {
        self.panel_taskbar = taskbar;
        if let Some(surface) = &mut self.surface {
            surface.set_panel_taskbar(taskbar);
        }
    }
}

pub struct WebSurface {
    kind: WebShellSurface,
    window: gtk::Window,
    container: gtk::Box,
    webview: WebView,
    visible: bool,
    loaded: Rc<Cell<bool>>,
    requested_visible: Rc<Cell<bool>>,
    panel_taskbar: bool,
    pending_snapshot: String,
    rendered_snapshot: String,
}

impl WebSurface {
    fn new(
        kind: WebShellSurface,
        size: (i32, i32),
        visible: bool,
        actions_tx: &Sender<WebShellAction>,
        snapshot: &WebShellSnapshot,
    ) -> Result<Self, Box<dyn Error>> {
        let window = gtk::Window::new(gtk::WindowType::Toplevel);
        configure_window(&window, kind, size);
        let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        configure_content_size(&container, kind, size);
        window.add(&container);

        let initial = serde_json::to_string(snapshot)?;
        let surface = serde_json::to_string(kind.as_str())?;
        let init = format!(
            "window.__STACCATO_SURFACE__={surface};window.__STACCATO_INITIAL_STATE__={initial};"
        );
        let tx = actions_tx.clone();
        let webview = WebViewBuilder::new()
            .with_html(shell_html(kind))
            .with_transparent(true)
            .with_background_color((0, 0, 0, 0))
            .with_initialization_script(init)
            .with_ipc_handler(move |request| match serde_json::from_str(request.body()) {
                Ok(action) => {
                    let _ = tx.send(action);
                }
                Err(error) => warn!(%error, "ignored invalid web shell action"),
            })
            .build_gtk(&container)?;
        configure_webview(&webview);
        resize_webview(&webview, kind, size);
        let loaded = Rc::new(Cell::new(false));
        let requested_visible = Rc::new(Cell::new(visible));
        let native = webview.webview();
        let load_window = window.clone();
        let load_ready = Rc::clone(&loaded);
        let load_requested = Rc::clone(&requested_visible);
        native.connect_load_changed(move |_, event| {
            if event != LoadEvent::Finished {
                return;
            }
            load_ready.set(true);
            if load_requested.get() {
                load_window.show_all();
                load_window.present();
            }
        });
        window.hide();
        Ok(Self {
            kind,
            window,
            container,
            webview,
            visible,
            loaded,
            requested_visible,
            panel_taskbar: false,
            pending_snapshot: initial.clone(),
            rendered_snapshot: initial,
        })
    }

    pub fn set_visible(&mut self, visible: bool) {
        if self.visible == visible {
            return;
        }
        self.visible = visible;
        self.requested_visible.set(visible);
        if visible {
            if self.loaded.get() {
                self.window.show_all();
                self.window.present();
                self.run_animation_script(SURFACE_OPEN_SCRIPT);
            }
            self.flush_snapshot();
        } else if self.loaded.get() && animated_surface(self.kind) {
            self.run_animation_script(SURFACE_CLOSE_SCRIPT);
            let window = self.window.clone();
            let requested_visible = Rc::clone(&self.requested_visible);
            glib::timeout_add_local(SURFACE_EXIT_DURATION, move || {
                if !requested_visible.get() {
                    window.hide();
                }
                glib::ControlFlow::Break
            });
        } else {
            self.window.hide();
        }
    }

    pub fn resize(&self, size: (i32, i32)) {
        self.window.set_default_size(size.0, size.1);
        resize_webview(&self.webview, self.kind, size);
        if self.kind == WebShellSurface::Panel {
            self.window.set_size_request(1, size.1);
            self.container.set_size_request(-1, size.1);
            self.window.resize(1, size.1);
            return;
        }
        if fixed_size(self.kind) || self.kind == WebShellSurface::Panel {
            self.window.set_size_request(size.0, size.1);
            self.container.set_size_request(size.0, size.1);
        }
        self.window.resize(size.0, size.1);
    }

    fn set_panel_taskbar(&mut self, taskbar: bool) {
        if self.panel_taskbar == taskbar {
            return;
        }
        self.panel_taskbar = taskbar;
        match self.kind {
            WebShellSurface::Panel => {
                configure_panel_window(&self.window, taskbar);
                self.resize(panel_size(taskbar));
            }
            WebShellSurface::DockMenu
            | WebShellSurface::QuickSettings
            | WebShellSurface::DateCenter => {
                configure_popover_window(&self.window, self.kind, taskbar);
            }
            _ => {}
        }
    }

    fn evaluate_snapshot(&mut self, json: &str) {
        if self.pending_snapshot != json {
            self.pending_snapshot = json.to_string();
        }
        self.flush_snapshot();
    }

    fn flush_snapshot(&mut self) {
        if !self.visible || self.pending_snapshot == self.rendered_snapshot {
            return;
        }
        let script = format!(
            "window.staccatoShell?.setSnapshot({});",
            self.pending_snapshot
        );
        match self.webview.evaluate_script(&script) {
            Ok(()) => self.rendered_snapshot.clone_from(&self.pending_snapshot),
            Err(error) => {
                debug!(%error, "failed to update web shell surface");
            }
        }
    }

    fn run_animation_script(&self, script: &str) {
        if !animated_surface(self.kind) {
            return;
        }
        if let Err(error) = self.webview.evaluate_script(script) {
            debug!(%error, surface = self.kind.as_str(), "failed to run shell surface animation");
        }
    }
}

fn animated_surface(kind: WebShellSurface) -> bool {
    matches!(
        kind,
        WebShellSurface::DockMenu
            | WebShellSurface::QuickSettings
            | WebShellSurface::DateCenter
            | WebShellSurface::Overview
    )
}

fn configure_webview(webview: &WebView) {
    let native = webview.webview();
    if let Some(settings) = WebViewExt::settings(&native) {
        settings.set_hardware_acceleration_policy(HardwareAccelerationPolicy::Always);
    }
}

fn resize_webview(webview: &WebView, kind: WebShellSurface, size: (i32, i32)) {
    let native = webview.webview();
    if kind == WebShellSurface::Panel {
        native.set_size_request(-1, size.1);
        return;
    }
    if fixed_size(kind) {
        native.set_size_request(size.0, size.1);
    }
}

fn shell_dev_url(kind: WebShellSurface) -> Option<String> {
    if let Ok(url) = env::var("STACCATO_SHELL_WEB_DEV_URL") {
        return Some(format!(
            "{}?surface={}",
            url.trim_end_matches('/'),
            kind.as_str()
        ));
    }
    None
}

fn shell_html(kind: WebShellSurface) -> String {
    if let Some(url) = shell_dev_url(kind) {
        return format!(
            r#"<!doctype html><meta charset="utf-8"><script>location.replace({url:?});</script>"#
        );
    }

    include_str!("../../web/dist/index.html").to_string()
}

pub fn dock_size(apps: &[DockApp]) -> (i32, i32) {
    super::surface_layout::dock_size(apps)
}
