use asher_ipc::Rect;
use smithay::utils::{Physical, Size};
use std::time::{Duration, Instant};

const OPEN_DURATION: Duration = Duration::from_millis(150);
const RESTORE_DURATION: Duration = Duration::from_millis(190);
const MINIMIZE_DURATION: Duration = Duration::from_millis(210);
const GEOMETRY_DURATION: Duration = Duration::from_millis(180);
const CLOSE_DURATION: Duration = Duration::from_millis(150);
const PANEL_TARGET_Y: i32 = 54;
const MINIMIZED_SCALE: f64 = 0.18;

#[derive(Debug, Clone)]
pub struct WindowAnimation {
    kind: WindowAnimationKind,
    started_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowAnimationKind {
    Idle,
    Open,
    Restore,
    Minimize,
    Close,
    Geometry { from: Rect },
}

#[derive(Debug, Clone, Copy)]
pub struct WindowTransform {
    pub x: f64,
    pub y: f64,
    pub scale: f64,
    pub alpha: f32,
}

impl Default for WindowAnimation {
    fn default() -> Self {
        Self {
            kind: WindowAnimationKind::Idle,
            started_at: Instant::now(),
        }
    }
}

impl Default for WindowTransform {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            scale: 1.0,
            alpha: 1.0,
        }
    }
}

impl WindowAnimation {
    pub fn open(enabled: bool) -> Self {
        Self::new(WindowAnimationKind::Open, enabled)
    }

    pub fn show(&mut self, enabled: bool) {
        *self = Self::new(WindowAnimationKind::Restore, enabled);
    }

    pub fn hide(&mut self, enabled: bool) {
        *self = Self::new(WindowAnimationKind::Minimize, enabled);
    }

    pub fn close(&mut self, enabled: bool) {
        *self = Self::new(WindowAnimationKind::Close, enabled);
    }

    pub fn geometry(&mut self, from: Rect, enabled: bool) {
        *self = Self::new(WindowAnimationKind::Geometry { from }, enabled);
    }

    pub fn renders_while_hidden(&self) -> bool {
        self.kind == WindowAnimationKind::Minimize && self.raw_progress(MINIMIZE_DURATION) < 1.0
    }

    pub fn close_finished(&self) -> bool {
        self.kind == WindowAnimationKind::Close && self.raw_progress(CLOSE_DURATION) >= 1.0
    }

    pub fn is_active(&self) -> bool {
        match self.kind {
            WindowAnimationKind::Idle => false,
            WindowAnimationKind::Open => self.raw_progress(OPEN_DURATION) < 1.0,
            WindowAnimationKind::Restore => self.raw_progress(RESTORE_DURATION) < 1.0,
            WindowAnimationKind::Minimize => self.raw_progress(MINIMIZE_DURATION) < 1.0,
            WindowAnimationKind::Close => self.raw_progress(CLOSE_DURATION) < 1.0,
            WindowAnimationKind::Geometry { .. } => self.raw_progress(GEOMETRY_DURATION) < 1.0,
        }
    }

    pub fn transform(&self, bounds: Rect, output: Size<i32, Physical>) -> WindowTransform {
        match self.kind {
            WindowAnimationKind::Idle => WindowTransform {
                x: bounds.x as f64,
                y: bounds.y as f64,
                ..WindowTransform::default()
            },
            WindowAnimationKind::Open => {
                let progress = ease_out(self.raw_progress(OPEN_DURATION));
                let scale = lerp(0.965, 1.0, progress);
                let alpha = progress as f32;
                let mut transform = scale_around_center(bounds, scale);
                transform.y += (1.0 - progress) * 10.0;
                transform.alpha = alpha;
                transform
            }
            WindowAnimationKind::Restore => {
                let progress = ease_out(self.raw_progress(RESTORE_DURATION));
                panel_transform(bounds, output, progress)
            }
            WindowAnimationKind::Minimize => {
                let progress = ease_in(self.raw_progress(MINIMIZE_DURATION));
                let mut transform = panel_transform(bounds, output, 1.0 - progress);
                transform.alpha = (1.0 - progress).clamp(0.0, 1.0) as f32;
                transform
            }
            WindowAnimationKind::Close => {
                let progress = ease_in(self.raw_progress(CLOSE_DURATION));
                let mut transform = scale_around_center(bounds, lerp(1.0, 0.965, progress));
                transform.y += progress * 8.0;
                transform.alpha = (1.0 - progress).clamp(0.0, 1.0) as f32;
                transform
            }
            WindowAnimationKind::Geometry { from } => {
                let progress = ease_out(self.raw_progress(GEOMETRY_DURATION));
                let target_center_x = bounds.x as f64 + bounds.width as f64 / 2.0;
                let target_center_y = bounds.y as f64 + bounds.height as f64 / 2.0;
                let start_center_x = from.x as f64 + from.width as f64 / 2.0;
                let start_center_y = from.y as f64 + from.height as f64 / 2.0;
                let scale_x = from.width.max(1) as f64 / bounds.width.max(1) as f64;
                let scale_y = from.height.max(1) as f64 / bounds.height.max(1) as f64;
                let scale = lerp(scale_x.min(scale_y).clamp(0.72, 1.28), 1.0, progress);
                let center_x = lerp(start_center_x, target_center_x, progress);
                let center_y = lerp(start_center_y, target_center_y, progress);

                WindowTransform {
                    x: center_x - bounds.width as f64 * scale / 2.0,
                    y: center_y - bounds.height as f64 * scale / 2.0,
                    scale,
                    alpha: 1.0,
                }
            }
        }
    }

    fn new(kind: WindowAnimationKind, enabled: bool) -> Self {
        if enabled {
            Self {
                kind,
                started_at: Instant::now(),
            }
        } else {
            Self::default()
        }
    }

    fn raw_progress(&self, duration: Duration) -> f64 {
        let elapsed = self.started_at.elapsed();
        if elapsed >= duration {
            return 1.0;
        }

        elapsed.as_secs_f64() / duration.as_secs_f64()
    }
}

fn panel_transform(bounds: Rect, output: Size<i32, Physical>, progress: f64) -> WindowTransform {
    let progress = progress.clamp(0.0, 1.0);
    let scale = lerp(MINIMIZED_SCALE, 1.0, progress);
    let window_center_x = bounds.x as f64 + bounds.width as f64 / 2.0;
    let window_center_y = bounds.y as f64 + bounds.height as f64 / 2.0;
    let panel_center_x = output.w as f64 / 2.0;
    let panel_center_y = (output.h - PANEL_TARGET_Y) as f64;
    let center_x = lerp(panel_center_x, window_center_x, progress);
    let center_y = lerp(panel_center_y, window_center_y, progress);

    WindowTransform {
        x: center_x - bounds.width as f64 * scale / 2.0,
        y: center_y - bounds.height as f64 * scale / 2.0,
        scale,
        alpha: progress as f32,
    }
}

fn scale_around_center(bounds: Rect, scale: f64) -> WindowTransform {
    let center_x = bounds.x as f64 + bounds.width as f64 / 2.0;
    let center_y = bounds.y as f64 + bounds.height as f64 / 2.0;

    WindowTransform {
        x: center_x - bounds.width as f64 * scale / 2.0,
        y: center_y - bounds.height as f64 * scale / 2.0,
        scale,
        alpha: 1.0,
    }
}

fn ease_out(progress: f64) -> f64 {
    let progress = progress.clamp(0.0, 1.0);
    1.0 - (1.0 - progress).powi(3)
}

fn ease_in(progress: f64) -> f64 {
    progress.clamp(0.0, 1.0).powi(2)
}

fn lerp(from: f64, to: f64, progress: f64) -> f64 {
    from + (to - from) * progress.clamp(0.0, 1.0)
}
