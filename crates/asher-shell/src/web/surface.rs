use super::{
    actions::WebShellAction,
    model::{WebShellSnapshot, WebShellSurface},
    surface_layout::{PANEL_HEIGHT, PANEL_WIDTH_HINT, panel_size, shell_surface},
    surface_sizing::{dock_menu_size, notification_toast_size, quick_settings_size},
};
use crate::dock::DockApp;
use fenestra_cef::{
    BridgeCommandDescriptor, BridgeError, BridgeResponse, FenestraProcess, FenestraWindow,
    RuntimeConfig, RuntimeMode, ShellSurfaceMargin, ShellSurfaceOptions, WebViewSecurity,
    WindowRegion,
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

const DATE_CENTER_WIDTH: i32 = 360;
const DATE_CENTER_HEIGHT: i32 = 560;
const START_MENU_WIDTH: i32 = 720;
const START_MENU_HEIGHT: i32 = 640;
const TRANSIENT_SURFACE_IDLE_TTL: Duration = Duration::from_secs(8);

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
                super::surface_layout::dock_size(dock_apps, dock_icon_size),
                true,
                false,
                false,
                None,
                &actions_tx,
                snapshot,
            )?,
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
                notification_toast_size(),
                panel_taskbar,
                &actions_tx,
                snapshot,
            ),
        };
        surfaces.quick.prewarm();
        surfaces.date.prewarm();
        if shell_prewarm_enabled() {
            surfaces.start_menu.prewarm();
        }
        surfaces.dock_menu.ensure_created();
        surfaces.notification_toast.ensure_created();
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

pub struct LazyWebSurface {
    kind: WebShellSurface,
    size: (i32, i32),
    actions_tx: Sender<WebShellAction>,
    snapshot: WebShellSnapshot,
    snapshot_json: String,
    visible: bool,
    show_at: Option<Instant>,
    show_started_at: Option<Instant>,
    show_start_alpha: f32,
    show_start_margin: Option<ShellSurfaceMargin>,
    hide_at: Option<Instant>,
    hide_started_at: Option<Instant>,
    hide_start_margin: Option<ShellSurfaceMargin>,
    release_at: Option<Instant>,
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
            show_at: None,
            show_started_at: None,
            show_start_alpha: 0.0,
            show_start_margin: None,
            hide_at: None,
            hide_started_at: None,
            hide_start_margin: None,
            release_at: None,
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
            self.show_at = None;
            self.show_started_at = None;
            self.show_start_margin = None;
            if let Some(surface) = &mut self.surface {
                if let Some(delay) = close_animation_duration(self.kind) {
                    let now = Instant::now();
                    self.hide_started_at = Some(now);
                    self.hide_start_margin = Some(surface.shell_margin);
                    surface.set_surface_alpha(1.0);
                    surface.emit_surface_close();
                    self.hide_at = Some(now + delay);
                } else {
                    self.hide_at = None;
                    self.hide_started_at = None;
                    self.hide_start_margin = None;
                    surface.set_surface_alpha(0.0);
                    surface.set_visible(false);
                    self.schedule_release(Instant::now());
                }
            }
            return;
        }

        let now = Instant::now();
        let resume_alpha = self.current_close_alpha(now);
        let was_closing = self.hide_at.take().is_some();
        self.hide_started_at = None;
        self.hide_start_margin = None;
        self.release_at = None;
        if self.surface.is_none() {
            self.ensure_created();
            if self.surface.is_none() {
                return;
            }
        }

        self.visible = true;
        if let Some(surface) = &mut self.surface {
            let open_duration = open_animation_duration(self.kind);
            let animates_alpha = surface_alpha_animates(self.kind);
            let animates_margin = surface_margin_animates(self.kind);
            let target_margin = surface.base_shell_margin();
            let initial_margin = if animates_margin {
                surface.shell_margin
            } else {
                target_margin
            };
            if !was_closing && animates_margin {
                surface.set_shell_margin(hidden_shell_margin(
                    self.kind,
                    target_margin,
                    surface.size,
                    self.panel_taskbar,
                ));
            }
            let initial_alpha = if open_duration.is_some() && animates_alpha && !was_closing {
                0.0
            } else {
                resume_alpha.unwrap_or(1.0)
            };
            surface.set_visible_with_alpha(true, initial_alpha);
            surface.emit_surface_open();
            if let Some(duration) = open_duration.filter(|_| animates_alpha || animates_margin) {
                self.show_started_at = Some(now);
                self.show_at = Some(now + duration);
                self.show_start_alpha = initial_alpha;
                self.show_start_margin = Some(if was_closing {
                    initial_margin
                } else {
                    surface.shell_margin
                });
                if was_closing {
                    self.tick_open(now, now + duration);
                } else if animates_alpha {
                    surface.set_surface_alpha(0.0);
                }
            } else {
                self.show_started_at = None;
                self.show_at = None;
                self.show_start_alpha = 1.0;
                self.show_start_margin = None;
                surface.set_surface_alpha(1.0);
                surface.set_shell_margin(target_margin);
            }
        }
    }

    fn tick(&mut self) {
        let now = Instant::now();
        if let Some(show_at) = self.show_at {
            self.tick_open(now, show_at);
            if now >= show_at {
                self.show_at = None;
                self.show_started_at = None;
                self.show_start_margin = None;
                if self.visible {
                    if let Some(surface) = &mut self.surface {
                        surface.set_surface_alpha(1.0);
                        surface.set_shell_margin(surface.base_shell_margin());
                    }
                }
            }
        }

        if let Some(hide_at) = self.hide_at {
            self.tick_close_alpha(now, hide_at);
            if now < hide_at {
                return;
            }
            self.hide_at = None;
            self.hide_started_at = None;
            self.hide_start_margin = None;
            if !self.visible {
                if let Some(surface) = &mut self.surface {
                    surface.set_surface_alpha(0.0);
                    surface.set_shell_margin(hidden_shell_margin(
                        self.kind,
                        surface.base_shell_margin(),
                        surface.size,
                        self.panel_taskbar,
                    ));
                    surface.set_visible(false);
                }
                self.schedule_release(now);
            }
        }

        let Some(release_at) = self.release_at else {
            return;
        };
        if self.visible || self.hide_at.is_some() || self.show_at.is_some() || now < release_at {
            return;
        }
        self.release_at = None;
        if let Some(surface) = &mut self.surface {
            surface.release_hidden_process();
        }
    }

    fn is_animating(&self) -> bool {
        self.show_at.is_some() || self.hide_at.is_some()
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
        if !self.visible {
            self.schedule_release(Instant::now());
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

    fn resize(&mut self, size: (i32, i32)) {
        if self.size == size {
            return;
        }
        self.size = size;
        if let Some(surface) = &mut self.surface {
            surface.resize(size);
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

    fn schedule_release(&mut self, now: Instant) {
        self.release_at = hidden_process_ttl(self.kind).map(|ttl| now + ttl);
    }

    fn tick_close_alpha(&mut self, now: Instant, hide_at: Instant) {
        let Some(started_at) = self.hide_started_at else {
            return;
        };
        let Some(surface) = &mut self.surface else {
            return;
        };
        let total = hide_at.saturating_duration_since(started_at);
        if total.is_zero() {
            surface.set_surface_alpha(0.0);
            return;
        }
        let elapsed = now.saturating_duration_since(started_at);
        let progress = (elapsed.as_secs_f32() / total.as_secs_f32()).clamp(0.0, 1.0);
        let eased = smoothstep(progress);
        let motion = close_motion_ease(progress);
        if surface_alpha_animates(self.kind) {
            surface.set_surface_alpha(1.0 - eased);
        }
        if surface_margin_animates(self.kind) {
            let from = self
                .hide_start_margin
                .unwrap_or_else(|| surface.base_shell_margin());
            let to = hidden_shell_margin(
                self.kind,
                surface.base_shell_margin(),
                surface.size,
                self.panel_taskbar,
            );
            surface.set_shell_margin(lerp_margin(from, to, motion));
        }
    }

    fn tick_open(&mut self, now: Instant, show_at: Instant) {
        let Some(started_at) = self.show_started_at else {
            return;
        };
        let Some(surface) = &mut self.surface else {
            return;
        };
        let total = show_at.saturating_duration_since(started_at);
        if total.is_zero() {
            surface.set_surface_alpha(1.0);
            surface.set_shell_margin(surface.base_shell_margin());
            return;
        }
        let elapsed = now.saturating_duration_since(started_at);
        let progress = (elapsed.as_secs_f32() / total.as_secs_f32()).clamp(0.0, 1.0);
        let eased = smoothstep(progress);
        let motion = open_motion_ease(progress);
        if surface_alpha_animates(self.kind) {
            surface
                .set_surface_alpha(self.show_start_alpha + (1.0 - self.show_start_alpha) * eased);
        }
        if surface_margin_animates(self.kind) {
            let from = self.show_start_margin.unwrap_or_else(|| {
                hidden_shell_margin(
                    self.kind,
                    surface.base_shell_margin(),
                    surface.size,
                    self.panel_taskbar,
                )
            });
            let to = surface.base_shell_margin();
            surface.set_shell_margin(lerp_margin(from, to, motion));
        }
    }

    fn current_close_alpha(&self, now: Instant) -> Option<f32> {
        let started_at = self.hide_started_at?;
        let hide_at = self.hide_at?;
        let total = hide_at.saturating_duration_since(started_at);
        if total.is_zero() {
            return Some(0.0);
        }
        let elapsed = now.saturating_duration_since(started_at);
        let progress = (elapsed.as_secs_f32() / total.as_secs_f32()).clamp(0.0, 1.0);
        Some(1.0 - smoothstep(progress))
    }
}

fn smoothstep(value: f32) -> f32 {
    value * value * (3.0 - 2.0 * value)
}

fn open_motion_ease(value: f32) -> f32 {
    1.0 - (1.0 - value).powi(4)
}

fn close_motion_ease(value: f32) -> f32 {
    value.powi(3)
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
    process: Option<FenestraProcess>,
    surface_alpha: f32,
    shell_margin: ShellSurfaceMargin,
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
        let shell_margin = shell_surface(kind, size, panel_taskbar, dock_menu_x).margin;
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
            surface_alpha: if visible { 1.0 } else { 0.0 },
            shell_margin,
            pending_snapshot: initial,
            rendered_snapshot: String::new(),
        };
        surface.set_visible(visible);
        Ok(surface)
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.set_visible_with_alpha(visible, 1.0);
    }

    fn set_visible_with_alpha(&mut self, visible: bool, alpha: f32) {
        if self.visible == visible {
            if visible {
                self.set_surface_alpha(alpha);
            }
            return;
        }
        self.visible = visible;
        if visible {
            self.show_process(alpha);
        } else {
            self.hide_process();
        }
    }

    pub fn resize(&mut self, size: (i32, i32)) {
        if self.size == size {
            return;
        }
        self.size = size;
        self.shell_margin = self.base_shell_margin();
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
            self.shell_margin = self.base_shell_margin();
            self.restart_for_geometry_change();
        }
    }

    fn set_dock_menu_x(&mut self, x: Option<i32>) {
        if self.dock_menu_x == x {
            return;
        }
        self.dock_menu_x = x;
        self.shell_margin = self.base_shell_margin();
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

    fn show_process(&mut self, alpha: f32) {
        let had_process = self.process.is_some();
        if had_process {
            self.flush_snapshot();
            self.set_surface_alpha(alpha);
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
            self.set_surface_alpha(alpha);
            self.flush_snapshot();
            return;
        }
        self.flush_snapshot();
    }

    fn hide_process(&mut self) {
        self.set_surface_alpha(0.0);
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

    fn release_hidden_process(&mut self) {
        if self.visible {
            return;
        }
        if self.process.take().is_some() {
            self.rendered_snapshot.clear();
        }
    }

    fn set_surface_alpha(&mut self, alpha: f32) {
        self.surface_alpha = alpha.clamp(0.0, 1.0);
        if let Some(process) = &self.process {
            let _ = process.set_shell_surface_alpha(self.surface_alpha);
        }
    }

    fn set_shell_margin(&mut self, margin: ShellSurfaceMargin) {
        if self.shell_margin == margin {
            return;
        }
        self.shell_margin = margin;
        if let Some(process) = &self.process {
            let _ = process.set_shell_surface_margin(margin);
        }
    }

    fn base_shell_margin(&self) -> ShellSurfaceMargin {
        shell_surface(self.kind, self.size, self.panel_taskbar, self.dock_menu_x).margin
    }

    fn build_window(&self) -> FenestraWindow {
        let snapshot = Arc::clone(&self.snapshot);
        let action_tx = self.actions_tx.clone();
        let kind = self.kind;
        let shell_options = shell_surface(kind, self.size, self.panel_taskbar, self.dock_menu_x)
            .margin(self.shell_margin);
        let (width, height) = cef_initial_size(&shell_options, self.size);

        let window = FenestraWindow::new()
            .title(format!("Asher {}", kind.as_str()))
            .fixed_size(width, height)
            .frameless()
            .glass()
            .always_on_top(true)
            .shell_surface(shell_options)
            .shell_surface_alpha(self.surface_alpha)
            .visible(self.visible)
            .active(self.visible && kind == WebShellSurface::StartMenu)
            .active_frame_rate(shell_surface_frame_rate())
            .blur_region(shell_blur_region(kind, width as i32, height as i32))
            .runtime(runtime_config())
            .security(WebViewSecurity::default())
            .bridge_descriptor_handler(
                BridgeCommandDescriptor::new("asher.ready").target("desktop"),
                move |_| {
                    let snapshot = snapshot
                        .lock()
                        .map_err(|_| BridgeError::new("failed to read asher shell snapshot"))?;
                    Ok(BridgeResponse::json(json!({
                        "surface": kind.as_str(),
                        "snapshot": &*snapshot,
                    })))
                },
            )
            .bridge_descriptor_handler(
                BridgeCommandDescriptor::new("asher.action").target("desktop"),
                move |command| match serde_json::from_value::<WebShellAction>(command.params) {
                    Ok(action) => {
                        action_tx
                            .send(action)
                            .map_err(|_| BridgeError::new("asher shell action channel closed"))?;
                        Ok(BridgeResponse::json(json!({ "ok": true })))
                    }
                    Err(error) => Err(BridgeError::new(format!(
                        "invalid asher shell action: {error}"
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
        if process.emit_bridge_event("asher.snapshot", value) {
            self.rendered_snapshot.clone_from(&self.pending_snapshot);
        }
    }

    fn emit_surface_open(&self) {
        let Some(process) = &self.process else {
            return;
        };
        let _ = process.emit_bridge_event(
            "asher.surface-open",
            json!({ "surface": self.kind.as_str() }),
        );
    }

    fn emit_surface_close(&self) {
        let Some(process) = &self.process else {
            return;
        };
        let _ = process.emit_bridge_event(
            "asher.surface-close",
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
        WebShellSurface::StartMenu => Some(Duration::from_millis(170)),
        WebShellSurface::QuickSettings | WebShellSurface::DateCenter => {
            Some(Duration::from_millis(170))
        }
        WebShellSurface::DockMenu => Some(Duration::from_millis(170)),
        WebShellSurface::Panel
        | WebShellSurface::Dock
        | WebShellSurface::Sidebar
        | WebShellSurface::NotificationToast => None,
    }
}

fn open_animation_duration(kind: WebShellSurface) -> Option<Duration> {
    match kind {
        WebShellSurface::StartMenu => Some(Duration::from_millis(190)),
        WebShellSurface::QuickSettings | WebShellSurface::DateCenter => {
            Some(Duration::from_millis(190))
        }
        WebShellSurface::DockMenu => Some(Duration::from_millis(190)),
        WebShellSurface::Panel
        | WebShellSurface::Dock
        | WebShellSurface::Sidebar
        | WebShellSurface::NotificationToast => None,
    }
}

fn shell_surface_frame_rate() -> u32 {
    env::var("ASHER_OUTPUT_REFRESH_MILLIHERTZ")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .map(|millihertz| (millihertz.saturating_add(999)) / 1000)
        .filter(|rate| *rate > 0)
        .unwrap_or(60)
}

fn surface_alpha_animates(kind: WebShellSurface) -> bool {
    !matches!(
        kind,
        WebShellSurface::StartMenu | WebShellSurface::QuickSettings | WebShellSurface::DateCenter
    )
}

fn surface_margin_animates(kind: WebShellSurface) -> bool {
    matches!(
        kind,
        WebShellSurface::StartMenu
            | WebShellSurface::QuickSettings
            | WebShellSurface::DateCenter
            | WebShellSurface::DockMenu
    )
}

fn hidden_process_ttl(kind: WebShellSurface) -> Option<Duration> {
    match kind {
        WebShellSurface::StartMenu => None,
        WebShellSurface::QuickSettings | WebShellSurface::DateCenter => None,
        WebShellSurface::DockMenu
        | WebShellSurface::NotificationToast
        | WebShellSurface::Sidebar => Some(TRANSIENT_SURFACE_IDLE_TTL),
        WebShellSurface::Panel | WebShellSurface::Dock => None,
    }
}

fn hidden_shell_margin(
    kind: WebShellSurface,
    base: ShellSurfaceMargin,
    size: (i32, i32),
    panel_taskbar: bool,
) -> ShellSurfaceMargin {
    let mut margin = base;
    match kind {
        WebShellSurface::QuickSettings if panel_taskbar => {
            margin.bottom = -(size.1 + 8);
        }
        WebShellSurface::StartMenu if panel_taskbar => {
            margin.bottom = -(size.1 + 58);
        }
        WebShellSurface::StartMenu => {
            margin.bottom = -(size.1 + 8);
        }
        WebShellSurface::QuickSettings => {
            margin.top = -(size.1 + 8);
        }
        WebShellSurface::DockMenu => {
            margin.bottom = -(size.1 + 8);
        }
        WebShellSurface::DateCenter => {
            margin.right = -(size.0 + 8);
        }
        _ => {}
    }
    margin
}

fn lerp_margin(
    from: ShellSurfaceMargin,
    to: ShellSurfaceMargin,
    progress: f32,
) -> ShellSurfaceMargin {
    ShellSurfaceMargin {
        top: lerp_i32(from.top, to.top, progress),
        right: lerp_i32(from.right, to.right, progress),
        bottom: lerp_i32(from.bottom, to.bottom, progress),
        left: lerp_i32(from.left, to.left, progress),
    }
}

fn lerp_i32(from: i32, to: i32, progress: f32) -> i32 {
    (from as f32 + (to - from) as f32 * progress)
        .round()
        .clamp(i32::MIN as f32, i32::MAX as f32) as i32
}

fn shell_blur_region(kind: WebShellSurface, _width: i32, _height: i32) -> WindowRegion {
    match kind {
        WebShellSurface::QuickSettings | WebShellSurface::DateCenter => {
            WindowRegion::adaptive_rounded_rect(26)
        }
        WebShellSurface::StartMenu => WindowRegion::adaptive_rounded_rect(24),
        _ => WindowRegion::adaptive_full(),
    }
}

fn shell_prewarm_enabled() -> bool {
    env::var("ASHER_SHELL_PREWARM")
        .ok()
        .is_some_and(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "on"))
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
    if let Ok(url) = env::var("ASHER_SHELL_WEB_DEV_URL") {
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
