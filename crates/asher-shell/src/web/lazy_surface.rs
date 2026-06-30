use super::{
    actions::WebShellAction,
    model::{WebShellSnapshot, WebShellSurface},
    surface_motion::{
        close_animation_duration, close_motion_ease, hidden_process_ttl, hidden_shell_margin,
        lerp_margin, open_animation_duration, open_motion_ease, smoothstep, surface_alpha_animates,
        surface_margin_animates,
    },
    web_surface::{WebSurface, WebSurfaceConfig},
};
use fenestra_cef::ShellSurfaceMargin;
use std::{sync::mpsc::Sender, time::Instant};
use tracing::warn;

pub(crate) struct LazyWebSurface {
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
    panel_menu_x: Option<i32>,
    surface: Option<WebSurface>,
}

impl LazyWebSurface {
    pub(super) fn new(
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
            show_at: None,
            show_started_at: None,
            show_start_alpha: 0.0,
            show_start_margin: None,
            hide_at: None,
            hide_started_at: None,
            hide_start_margin: None,
            release_at: None,
            panel_menu_x: None,
            surface: None,
        }
    }

    pub(super) fn set_visible(&mut self, visible: bool) {
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
                    if !surface_alpha_animates(self.kind) {
                        surface.set_surface_alpha(1.0);
                    }
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
        let resume_alpha = if surface_alpha_animates(self.kind) {
            self.current_close_alpha(now)
        } else {
            Some(1.0)
        };
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

    pub(super) fn tick(&mut self) {
        let now = Instant::now();
        if let Some(show_at) = self.show_at {
            self.tick_open(now, show_at);
            if now >= show_at {
                self.show_at = None;
                self.show_started_at = None;
                self.show_start_margin = None;
                if self.visible
                    && let Some(surface) = &mut self.surface
                {
                    surface.set_surface_alpha(1.0);
                    surface.set_shell_margin(surface.base_shell_margin());
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

    pub(super) fn is_animating(&self) -> bool {
        self.show_at.is_some() || self.hide_at.is_some()
    }

    fn ensure_created(&mut self) {
        if self.surface.is_some() {
            return;
        }
        match WebSurface::new(WebSurfaceConfig {
            kind: self.kind,
            size: self.size,
            visible: false,
            keep_alive_when_hidden: true,
            panel_menu_x: self.panel_menu_x,
            actions_tx: &self.actions_tx,
            snapshot: &self.snapshot,
        }) {
            Ok(mut surface) => {
                surface.evaluate_snapshot(&self.snapshot, &self.snapshot_json);
                self.surface = Some(surface);
            }
            Err(error) => {
                warn!(%error, surface = self.kind.as_str(), "failed to create web shell surface");
            }
        }
    }

    pub(super) fn prewarm(&mut self) {
        self.ensure_created();
        if let Some(surface) = &mut self.surface {
            surface.prewarm();
        }
        if !self.visible {
            self.schedule_release(Instant::now());
        }
    }

    pub(super) fn evaluate_snapshot(&mut self, snapshot: &WebShellSnapshot, json: &str) {
        self.snapshot = snapshot.clone();
        if self.snapshot_json != json {
            self.snapshot_json = json.to_string();
        }
        if let Some(surface) = &mut self.surface {
            surface.evaluate_snapshot(snapshot, json);
        }
    }

    pub(super) fn resize(&mut self, size: (i32, i32)) {
        if self.size == size {
            return;
        }
        self.size = size;
        if let Some(surface) = &mut self.surface {
            surface.resize(size);
        }
    }

    pub(super) fn set_panel_menu_x(&mut self, x: Option<i32>) {
        if self.panel_menu_x == x {
            return;
        }
        self.panel_menu_x = x;
        if let Some(surface) = &mut self.surface {
            surface.set_panel_menu_x(x);
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
            let to = hidden_shell_margin(self.kind, surface.base_shell_margin(), surface.size);
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
                hidden_shell_margin(self.kind, surface.base_shell_margin(), surface.size)
            });
            let to = surface.base_shell_margin();
            let margin = if progress >= 1.0 {
                to
            } else {
                lerp_margin(from, to, motion)
            };
            surface.set_shell_margin(margin);
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
