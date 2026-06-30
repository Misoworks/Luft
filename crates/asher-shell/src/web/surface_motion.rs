use super::model::WebShellSurface;
use fenestra_cef::{ShellSurfaceMargin, WindowRegion};
use std::time::Duration;

pub(super) fn smoothstep(value: f32) -> f32 {
    value * value * (3.0 - 2.0 * value)
}

pub(super) fn open_motion_ease(value: f32) -> f32 {
    1.0 - (1.0 - value).powi(4)
}

pub(super) fn close_motion_ease(value: f32) -> f32 {
    value.powi(3)
}

pub(super) fn close_animation_duration(kind: WebShellSurface) -> Option<Duration> {
    match kind {
        WebShellSurface::StartMenu => Some(Duration::from_millis(170)),
        WebShellSurface::QuickSettings | WebShellSurface::DateCenter => {
            Some(Duration::from_millis(170))
        }
        WebShellSurface::PanelMenu | WebShellSurface::NotificationToast => {
            Some(Duration::from_millis(170))
        }
        WebShellSurface::Panel => None,
    }
}

pub(super) fn open_animation_duration(kind: WebShellSurface) -> Option<Duration> {
    match kind {
        WebShellSurface::StartMenu => Some(Duration::from_millis(190)),
        WebShellSurface::QuickSettings | WebShellSurface::DateCenter => {
            Some(Duration::from_millis(190))
        }
        WebShellSurface::PanelMenu | WebShellSurface::NotificationToast => {
            Some(Duration::from_millis(190))
        }
        WebShellSurface::Panel => None,
    }
}

pub(super) fn surface_alpha_animates(_kind: WebShellSurface) -> bool {
    false
}

pub(super) fn surface_margin_animates(kind: WebShellSurface) -> bool {
    matches!(
        kind,
        WebShellSurface::StartMenu
            | WebShellSurface::QuickSettings
            | WebShellSurface::DateCenter
            | WebShellSurface::PanelMenu
            | WebShellSurface::NotificationToast
    )
}

pub(super) fn hidden_process_ttl(kind: WebShellSurface) -> Option<Duration> {
    match kind {
        WebShellSurface::StartMenu
        | WebShellSurface::QuickSettings
        | WebShellSurface::DateCenter
        | WebShellSurface::PanelMenu
        | WebShellSurface::NotificationToast
        | WebShellSurface::Panel => None,
    }
}

pub(super) fn hidden_shell_margin(
    kind: WebShellSurface,
    base: ShellSurfaceMargin,
    size: (i32, i32),
) -> ShellSurfaceMargin {
    let mut margin = base;
    match kind {
        WebShellSurface::QuickSettings => {
            margin.bottom = -(size.1 + 8);
        }
        WebShellSurface::StartMenu => {
            margin.bottom = -(size.1 + 58);
        }
        WebShellSurface::PanelMenu => {
            margin.bottom = -(size.1 + 8);
        }
        WebShellSurface::NotificationToast => {
            margin.right = -(size.0 + 12);
        }
        WebShellSurface::DateCenter => {
            margin.right = -(size.0 + 8);
        }
        _ => {}
    }
    margin
}

pub(super) fn lerp_margin(
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

pub(super) fn shell_blur_region(kind: WebShellSurface, _width: i32, _height: i32) -> WindowRegion {
    match kind {
        WebShellSurface::QuickSettings | WebShellSurface::DateCenter => {
            WindowRegion::adaptive_rounded_rect(26)
        }
        WebShellSurface::NotificationToast => WindowRegion::adaptive_rounded_rect(22),
        WebShellSurface::StartMenu => WindowRegion::adaptive_rounded_rect(24),
        _ => WindowRegion::adaptive_full(),
    }
}
