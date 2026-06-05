use super::{
    actions::WebShellAction,
    model::{WebShellSnapshot, WebShellSurface},
    surface_layout::{PANEL_HEIGHT, PANEL_WIDTH_HINT, panel_size, shell_surface},
    surface_sizing::{notification_toast_size, quick_settings_size},
};
use crate::dock::DockApp;
use fenestra_cef::{
    BridgeCommandDescriptor, BridgeError, BridgeResponse, CefProcess, CefWindow, RuntimeConfig,
    RuntimeMode, ShellSurfaceOptions, WebViewSecurity,
};
use serde_json::json;
use std::{
    env,
    error::Error,
    path::PathBuf,
    sync::{Arc, Mutex, mpsc::Sender},
    time::{Duration, Instant},
};
use tracing::{debug, warn};

const DOCK_MENU_WIDTH: i32 = 184;
const DOCK_MENU_HEIGHT: i32 = 128;
const DATE_CENTER_WIDTH: i32 = 360;
const DATE_CENTER_HEIGHT: i32 = 470;

pub struct WebSurfaces {
    pub dock: WebSurface,
    pub sidebar: LazyWebSurface,
    pub overview: LazyWebSurface,
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
        panel_taskbar: bool,
    ) -> Result<Self, Box<dyn Error>> {
        let mut surfaces = Self {
            panel: WebSurface::new(
                WebShellSurface::Panel,
                (PANEL_WIDTH_HINT, PANEL_HEIGHT),
                true,
                false,
                panel_taskbar,
                None,
                &actions_tx,
                snapshot,
            )?,
            dock: WebSurface::new(
                WebShellSurface::Dock,
                super::surface_layout::dock_size(dock_apps),
                true,
                false,
                false,
                None,
                &actions_tx,
                snapshot,
            )?,
            dock_menu: LazyWebSurface::new(
                WebShellSurface::DockMenu,
                (DOCK_MENU_WIDTH, DOCK_MENU_HEIGHT),
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
            overview: LazyWebSurface::new(
                WebShellSurface::Overview,
                (1, 1),
                false,
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
                notification_toast_size(),
                panel_taskbar,
                &actions_tx,
                snapshot,
            ),
        };
        surfaces.overview.prewarm();
        surfaces.quick.prewarm();
        surfaces.date.prewarm();
        surfaces.dock_menu.ensure_created();
        surfaces.notification_toast.ensure_created();
        Ok(surfaces)
    }

    pub fn evaluate_snapshot(&mut self, snapshot: &WebShellSnapshot, json: &str) {
        self.panel.evaluate_snapshot(snapshot, json);
        self.dock.evaluate_snapshot(snapshot, json);
        self.dock_menu.evaluate_snapshot(snapshot, json);
        self.sidebar.evaluate_snapshot(snapshot, json);
        self.overview.evaluate_snapshot(snapshot, json);
        self.quick.evaluate_snapshot(snapshot, json);
        self.date.evaluate_snapshot(snapshot, json);
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
        self.notification_toast.set_panel_taskbar(taskbar);
    }

    pub fn resize_dock(&mut self, apps: &[DockApp]) {
        self.dock.resize(super::surface_layout::dock_size(apps));
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
        self.overview.tick();
        self.quick.tick();
        self.date.tick();
        self.notification_toast.tick();
    }
}

pub struct LazyWebSurface {
    kind: WebShellSurface,
    size: (i32, i32),
    actions_tx: Sender<WebShellAction>,
    snapshot: WebShellSnapshot,
    snapshot_json: String,
    visible: bool,
    hide_at: Option<Instant>,
    panel_taskbar: bool,
    dock_menu_x: Option<i32>,
    surface: Option<WebSurface>,
}

impl LazyWebSurface {
    fn new(
        kind: WebShellSurface,
        size: (i32, i32),
        panel_taskbar: bool,
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
            hide_at: None,
            panel_taskbar,
            dock_menu_x: None,
            surface: None,
        }
    }

    pub fn set_visible(&mut self, visible: bool) {
        if !visible {
            if !self.visible {
                if self.hide_at.is_some() {
                    return;
                }
                if self.surface.as_ref().is_none_or(|surface| !surface.visible) {
                    return;
                }
            }
            self.visible = false;
            if let Some(surface) = &mut self.surface {
                if let Some(delay) = close_animation_duration(self.kind) {
                    surface.emit_surface_close();
                    self.hide_at = Some(Instant::now() + delay);
                } else {
                    self.hide_at = None;
                    surface.set_visible(false);
                }
            }
            return;
        }

        let was_closing = self.hide_at.take().is_some();
        if self.surface.is_none() {
            self.ensure_created();
            if self.surface.is_none() {
                return;
            }
        }

        self.visible = true;
        if let Some(surface) = &mut self.surface {
            if was_closing {
                surface.emit_surface_open();
            }
            surface.set_visible(true);
        }
    }

    fn tick(&mut self) {
        let Some(hide_at) = self.hide_at else {
            return;
        };
        if Instant::now() < hide_at {
            return;
        }
        self.hide_at = None;
        if !self.visible {
            if let Some(surface) = &mut self.surface {
                surface.set_visible(false);
            }
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
            true,
            self.panel_taskbar,
            self.dock_menu_x,
            &self.actions_tx,
            &self.snapshot,
        ) {
            Ok(mut surface) => {
                surface.set_panel_taskbar(self.panel_taskbar);
                surface.evaluate_snapshot(&self.snapshot, &self.snapshot_json);
                self.surface = Some(surface);
            }
            Err(error) => {
                warn!(%error, surface = self.kind.as_str(), "failed to create web shell surface");
            }
        }
    }

    fn prewarm(&mut self) {
        self.ensure_created();
        if let Some(surface) = &mut self.surface {
            surface.prewarm();
        }
    }

    fn evaluate_snapshot(&mut self, snapshot: &WebShellSnapshot, json: &str) {
        self.snapshot = snapshot.clone();
        if self.snapshot_json != json {
            self.snapshot_json = json.to_string();
        }
        if let Some(surface) = &mut self.surface {
            surface.evaluate_snapshot(snapshot, json);
        }
    }

    fn set_panel_taskbar(&mut self, taskbar: bool) {
        self.panel_taskbar = taskbar;
        if let Some(surface) = &mut self.surface {
            surface.set_panel_taskbar(taskbar);
        }
    }

    fn set_dock_menu_x(&mut self, x: Option<i32>) {
        if self.dock_menu_x == x {
            return;
        }
        self.dock_menu_x = x;
        if let Some(surface) = &mut self.surface {
            surface.set_dock_menu_x(x);
        }
    }
}

pub struct WebSurface {
    kind: WebShellSurface,
    size: (i32, i32),
    actions_tx: Sender<WebShellAction>,
    snapshot: Arc<Mutex<WebShellSnapshot>>,
    visible: bool,
    keep_alive_when_hidden: bool,
    panel_taskbar: bool,
    dock_menu_x: Option<i32>,
    process: Option<CefProcess>,
    pending_snapshot: String,
    rendered_snapshot: String,
}

impl WebSurface {
    fn new(
        kind: WebShellSurface,
        size: (i32, i32),
        visible: bool,
        keep_alive_when_hidden: bool,
        panel_taskbar: bool,
        dock_menu_x: Option<i32>,
        actions_tx: &Sender<WebShellAction>,
        snapshot: &WebShellSnapshot,
    ) -> Result<Self, Box<dyn Error>> {
        let initial = serde_json::to_string(snapshot)?;
        let mut surface = Self {
            kind,
            size,
            actions_tx: actions_tx.clone(),
            snapshot: Arc::new(Mutex::new(snapshot.clone())),
            visible: false,
            keep_alive_when_hidden,
            panel_taskbar,
            dock_menu_x,
            process: None,
            pending_snapshot: initial,
            rendered_snapshot: String::new(),
        };
        surface.set_visible(visible);
        Ok(surface)
    }

    pub fn set_visible(&mut self, visible: bool) {
        if self.visible == visible {
            return;
        }
        self.visible = visible;
        if visible {
            self.show_process();
        } else {
            self.hide_process();
        }
    }

    pub fn resize(&mut self, size: (i32, i32)) {
        if self.size == size {
            return;
        }
        self.size = size;
        self.restart_for_geometry_change();
    }

    fn set_panel_taskbar(&mut self, taskbar: bool) {
        if self.panel_taskbar == taskbar {
            return;
        }
        self.panel_taskbar = taskbar;
        if self.kind == WebShellSurface::Panel {
            self.resize(panel_size(taskbar));
        } else {
            self.restart_for_geometry_change();
        }
    }

    fn set_dock_menu_x(&mut self, x: Option<i32>) {
        if self.dock_menu_x == x {
            return;
        }
        self.dock_menu_x = x;
        if self.kind == WebShellSurface::DockMenu {
            self.restart_for_geometry_change();
        }
    }

    fn evaluate_snapshot(&mut self, snapshot: &WebShellSnapshot, json: &str) {
        if let Ok(mut current) = self.snapshot.lock() {
            *current = snapshot.clone();
        }
        if self.pending_snapshot != json {
            self.pending_snapshot = json.to_string();
        }
        self.flush_snapshot();
    }

    fn launch(&mut self) {
        if self.process.is_some() {
            self.flush_snapshot();
            return;
        }

        let window = self.build_window();
        match window.launch_or_install() {
            Ok(process) => {
                debug!(
                    pid = process.id(),
                    surface = self.kind.as_str(),
                    "launched Fenestra shell surface"
                );
                self.process = Some(process);
            }
            Err(error) => {
                warn!(%error, surface = self.kind.as_str(), "failed to launch Fenestra shell surface");
            }
        }
    }

    fn prewarm(&mut self) {
        if self.process.is_none() {
            self.launch();
        }
        if !self.visible {
            self.hide_process();
        }
    }

    fn show_process(&mut self) {
        let had_process = self.process.is_some();
        if had_process {
            self.flush_snapshot();
        }
        let restored = self
            .process
            .as_ref()
            .is_some_and(|process| process.set_shell_surface_visible(true));
        if had_process && !restored {
            self.process = None;
            self.rendered_snapshot.clear();
        }
        if self.process.is_none() {
            self.launch();
        }
        self.flush_snapshot();
        self.emit_surface_open();
    }

    fn hide_process(&mut self) {
        if !self.keep_alive_when_hidden {
            self.process = None;
            self.rendered_snapshot.clear();
            return;
        }
        if self
            .process
            .as_ref()
            .is_none_or(|process| process.set_shell_surface_visible(false))
        {
            return;
        }
        self.process = None;
        self.rendered_snapshot.clear();
    }

    fn build_window(&self) -> CefWindow {
        let snapshot = Arc::clone(&self.snapshot);
        let action_tx = self.actions_tx.clone();
        let kind = self.kind;
        let shell_options = shell_surface(kind, self.size, self.panel_taskbar, self.dock_menu_x);
        let (width, height) = cef_initial_size(&shell_options, self.size);

        let window = CefWindow::new()
            .title(format!("Staccato {}", kind.as_str()))
            .fixed_size(width, height)
            .frameless()
            .transparent(true)
            .always_on_top(true)
            .shell_surface(shell_options)
            .visible(self.visible)
            .active(self.visible && kind == WebShellSurface::Overview)
            .runtime(runtime_config())
            .security(WebViewSecurity::default())
            .bridge_descriptor_handler(
                BridgeCommandDescriptor::new("staccato.ready").target("desktop"),
                move |_| {
                    let snapshot = snapshot
                        .lock()
                        .map_err(|_| BridgeError::new("failed to read staccato shell snapshot"))?;
                    Ok(BridgeResponse::json(json!({
                        "surface": kind.as_str(),
                        "snapshot": &*snapshot,
                    })))
                },
            )
            .bridge_descriptor_handler(
                BridgeCommandDescriptor::new("staccato.action").target("desktop"),
                move |command| match serde_json::from_value::<WebShellAction>(command.params) {
                    Ok(action) => {
                        action_tx.send(action).map_err(|_| {
                            BridgeError::new("staccato shell action channel closed")
                        })?;
                        Ok(BridgeResponse::json(json!({ "ok": true })))
                    }
                    Err(error) => Err(BridgeError::new(format!(
                        "invalid staccato shell action: {error}"
                    ))),
                },
            );

        match shell_entry(kind) {
            ShellEntry::Dev(url) => window.dev_url(url),
            ShellEntry::File(path) => window.entry(path),
        }
    }

    fn flush_snapshot(&mut self) {
        if !self.visible || self.pending_snapshot == self.rendered_snapshot {
            return;
        }
        let Some(process) = &self.process else {
            return;
        };
        let Ok(snapshot) = self.snapshot.lock() else {
            return;
        };
        let Ok(value) = serde_json::to_value(&*snapshot) else {
            return;
        };
        if process.emit_bridge_event("staccato.snapshot", value) {
            self.rendered_snapshot.clone_from(&self.pending_snapshot);
        }
    }

    fn emit_surface_open(&self) {
        let Some(process) = &self.process else {
            return;
        };
        let _ = process.emit_bridge_event(
            "staccato.surface-open",
            json!({ "surface": self.kind.as_str() }),
        );
    }

    fn emit_surface_close(&self) {
        let Some(process) = &self.process else {
            return;
        };
        let _ = process.emit_bridge_event(
            "staccato.surface-close",
            json!({ "surface": self.kind.as_str() }),
        );
    }

    fn restart_for_geometry_change(&mut self) {
        let was_running = self.process.is_some();
        if !self.visible && (!self.keep_alive_when_hidden || !was_running) {
            return;
        }
        self.process = None;
        self.rendered_snapshot.clear();
        self.launch();
        if !self.visible {
            self.hide_process();
        }
    }
}

fn close_animation_duration(kind: WebShellSurface) -> Option<Duration> {
    match kind {
        WebShellSurface::Overview => Some(Duration::from_millis(190)),
        WebShellSurface::QuickSettings | WebShellSurface::DateCenter => {
            Some(Duration::from_millis(150))
        }
        WebShellSurface::Panel
        | WebShellSurface::Dock
        | WebShellSurface::DockMenu
        | WebShellSurface::Sidebar
        | WebShellSurface::NotificationToast => None,
    }
}

fn runtime_config() -> RuntimeConfig {
    RuntimeConfig {
        mode: RuntimeMode::SharedPreferred,
        allow_user_install: true,
        bundled_dir: Some(workspace_root()),
        ..RuntimeConfig::default()
    }
}

enum ShellEntry {
    Dev(String),
    File(String),
}

fn shell_entry(kind: WebShellSurface) -> ShellEntry {
    if let Ok(url) = env::var("STACCATO_SHELL_WEB_DEV_URL") {
        return ShellEntry::Dev(append_shell_query(url.trim_end_matches('/'), kind));
    }
    ShellEntry::File(append_shell_query(
        &manifest_dir()
            .join("web/dist/index.html")
            .display()
            .to_string(),
        kind,
    ))
}

fn append_shell_query(base: &str, kind: WebShellSurface) -> String {
    let separator = if base.contains('?') { '&' } else { '?' };
    format!("{base}{separator}surface={}&fenestra=1", kind.as_str())
}

fn cef_initial_size(shell_surface: &ShellSurfaceOptions, fallback: (i32, i32)) -> (u32, u32) {
    let (width, height) = shell_surface
        .size
        .unwrap_or((fallback.0.max(1) as u32, fallback.1.max(1) as u32));
    (width.max(1), height.max(1))
}

fn workspace_root() -> PathBuf {
    manifest_dir()
        .parent()
        .and_then(|path| path.parent())
        .map(PathBuf::from)
        .unwrap_or_else(manifest_dir)
}

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}
