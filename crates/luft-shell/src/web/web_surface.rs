use super::{
    actions::WebShellAction,
    model::{WebShellSnapshot, WebShellSurface},
    surface_layout::shell_surface,
    surface_motion::shell_blur_region,
};
use fenestra_cef::{
    BridgeCommandDescriptor, BridgeError, BridgeResponse, FenestraProcess, FenestraWindow,
    RuntimeConfig, RuntimeMode, ShellSurfaceMargin, ShellSurfaceOptions, WebViewSecurity,
};
use serde_json::json;
use std::{
    env,
    error::Error,
    path::PathBuf,
    sync::{Arc, Mutex, mpsc::Sender},
};
use tracing::{debug, warn};

pub struct WebSurface {
    kind: WebShellSurface,
    pub(crate) size: (i32, i32),
    actions_tx: Sender<WebShellAction>,
    snapshot: Arc<Mutex<WebShellSnapshot>>,
    pub(crate) visible: bool,
    keep_alive_when_hidden: bool,
    panel_menu_x: Option<i32>,
    process: Option<FenestraProcess>,
    surface_alpha: f32,
    pub(crate) shell_margin: ShellSurfaceMargin,
    pending_snapshot: String,
    rendered_snapshot: String,
}

impl WebSurface {
    pub(crate) fn new(config: WebSurfaceConfig<'_>) -> Result<Self, Box<dyn Error>> {
        let initial = serde_json::to_string(config.snapshot)?;
        let shell_margin = shell_surface(config.kind, config.size, config.panel_menu_x).margin;
        let mut surface = Self {
            kind: config.kind,
            size: config.size,
            actions_tx: config.actions_tx.clone(),
            snapshot: Arc::new(Mutex::new(config.snapshot.clone())),
            visible: false,
            keep_alive_when_hidden: config.keep_alive_when_hidden,
            panel_menu_x: config.panel_menu_x,
            process: None,
            surface_alpha: if config.visible { 1.0 } else { 0.0 },
            shell_margin,
            pending_snapshot: initial,
            rendered_snapshot: String::new(),
        };
        surface.set_visible(config.visible);
        Ok(surface)
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.set_visible_with_alpha(visible, 1.0);
    }

    pub(crate) fn set_visible_with_alpha(&mut self, visible: bool, alpha: f32) {
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

    pub(crate) fn set_panel_menu_x(&mut self, x: Option<i32>) {
        if self.panel_menu_x == x {
            return;
        }
        self.panel_menu_x = x;
        self.shell_margin = self.base_shell_margin();
        if self.kind == WebShellSurface::PanelMenu {
            self.restart_for_geometry_change();
        }
    }

    pub(crate) fn evaluate_snapshot(&mut self, snapshot: &WebShellSnapshot, json: &str) {
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

    pub(crate) fn prewarm(&mut self) {
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

    pub(crate) fn release_hidden_process(&mut self) {
        if self.visible {
            return;
        }
        if self.process.take().is_some() {
            self.rendered_snapshot.clear();
        }
    }

    pub(crate) fn set_surface_alpha(&mut self, alpha: f32) {
        let alpha = alpha.clamp(0.0, 1.0);
        if (self.surface_alpha - alpha).abs() < f32::EPSILON {
            return;
        }
        self.surface_alpha = alpha;
        if let Some(process) = &self.process {
            let _ = process.set_shell_surface_alpha(self.surface_alpha);
        }
    }

    pub(crate) fn set_shell_margin(&mut self, margin: ShellSurfaceMargin) {
        if self.shell_margin == margin {
            return;
        }
        self.shell_margin = margin;
        if let Some(process) = &self.process {
            let _ = process.set_shell_surface_margin(margin);
        }
    }

    pub(crate) fn base_shell_margin(&self) -> ShellSurfaceMargin {
        shell_surface(self.kind, self.size, self.panel_menu_x).margin
    }

    fn build_window(&self) -> FenestraWindow {
        let snapshot = Arc::clone(&self.snapshot);
        let action_tx = self.actions_tx.clone();
        let kind = self.kind;
        let shell_options =
            shell_surface(kind, self.size, self.panel_menu_x).margin(self.shell_margin);
        let (width, height) = cef_initial_size(&shell_options, self.size);
        let window = FenestraWindow::new()
            .title(format!("Luft {}", kind.as_str()))
            .fixed_size(width, height)
            .frameless()
            .glass()
            .always_on_top(true)
            .shell_surface(shell_options)
            .shell_surface_alpha(self.surface_alpha)
            .visible(self.visible)
            .active(self.visible && kind == WebShellSurface::StartMenu)
            .active_frame_rate(shell_surface_frame_rate(kind))
            .background_frame_rate(1)
            .blur_region(shell_blur_region(kind, width as i32, height as i32))
            .runtime(runtime_config())
            .security(WebViewSecurity::default())
            .bridge_descriptor_handler(
                BridgeCommandDescriptor::new("luft.ready").target("desktop"),
                move |_| {
                    let snapshot = snapshot
                        .lock()
                        .map_err(|_| BridgeError::new("failed to read luft shell snapshot"))?;
                    Ok(BridgeResponse::json(json!({
                        "surface": kind.as_str(),
                        "snapshot": &*snapshot,
                    })))
                },
            )
            .bridge_descriptor_handler(
                BridgeCommandDescriptor::new("luft.action").target("desktop"),
                move |command| match serde_json::from_value::<WebShellAction>(command.params) {
                    Ok(action) => {
                        action_tx
                            .send(action)
                            .map_err(|_| BridgeError::new("luft shell action channel closed"))?;
                        Ok(BridgeResponse::json(json!({ "ok": true })))
                    }
                    Err(error) => Err(BridgeError::new(format!(
                        "invalid luft shell action: {error}"
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
        if process.emit_bridge_event("luft.snapshot", value) {
            self.rendered_snapshot.clone_from(&self.pending_snapshot);
        }
    }

    pub(crate) fn emit_surface_open(&self) {
        let Some(process) = &self.process else {
            return;
        };
        let _ = process.emit_bridge_event(
            "luft.surface-open",
            json!({ "surface": self.kind.as_str() }),
        );
    }

    pub(crate) fn emit_surface_close(&self) {
        let Some(process) = &self.process else {
            return;
        };
        let _ = process.emit_bridge_event(
            "luft.surface-close",
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

pub(crate) struct WebSurfaceConfig<'a> {
    pub kind: WebShellSurface,
    pub size: (i32, i32),
    pub visible: bool,
    pub keep_alive_when_hidden: bool,
    pub panel_menu_x: Option<i32>,
    pub actions_tx: &'a Sender<WebShellAction>,
    pub snapshot: &'a WebShellSnapshot,
}

fn shell_surface_frame_rate(kind: WebShellSurface) -> u32 {
    match kind {
        WebShellSurface::Panel => 30,
        _ => output_frame_rate(),
    }
}

fn output_frame_rate() -> u32 {
    env::var("LUFT_OUTPUT_REFRESH_MILLIHERTZ")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .map(|millihertz| (millihertz.saturating_add(999)) / 1000)
        .filter(|rate| *rate > 0)
        .unwrap_or(60)
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
    if let Ok(url) = env::var("LUFT_SHELL_WEB_DEV_URL") {
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
