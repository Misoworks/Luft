use super::{
    ManagedWindow, RESIZE_BORDER, TITLEBAR_CONTROL_GAP, TITLEBAR_CONTROL_HIT_PADDING,
    TITLEBAR_CONTROL_RIGHT, TITLEBAR_CONTROL_SIZE, TITLEBAR_HEIGHT,
};
use asher_ipc::WindowId;
use smithay::{
    utils::{Logical, Point},
    wayland::shell::xdg::ToplevelSurface,
};

#[derive(Debug, Clone)]
pub enum WindowFrameHit {
    Titlebar {
        surface: ToplevelSurface,
    },
    Control {
        id: WindowId,
        control: WindowFrameControl,
    },
    Resize {
        surface: ToplevelSurface,
        edge: ResizeEdge,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowFrameControl {
    Minimize,
    Maximize,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResizeEdge {
    pub left: bool,
    pub right: bool,
    pub top: bool,
    pub bottom: bool,
}

impl ResizeEdge {
    pub fn new(left: bool, right: bool, top: bool, bottom: bool) -> Option<Self> {
        (left || right || top || bottom).then_some(Self {
            left,
            right,
            top,
            bottom,
        })
    }
}

pub(super) fn contains(window: &ManagedWindow, point: Point<f64, Logical>) -> bool {
    let location = window.location.to_f64();
    let size = window.size.to_f64();

    point.x >= location.x
        && point.y >= location.y
        && point.x < location.x + size.w
        && point.y < location.y + size.h + window.titlebar_height() as f64
}

pub(super) fn content_contains(window: &ManagedWindow, point: Point<f64, Logical>) -> bool {
    let location = window.content_location().to_f64();
    let size = window.size.to_f64();

    point.x >= location.x
        && point.y >= location.y
        && point.x < location.x + size.w
        && point.y < location.y + size.h
}

pub(super) fn titlebar_contains(window: &ManagedWindow, point: Point<f64, Logical>) -> bool {
    if window.titlebar_height() == 0 {
        return false;
    }

    let location = window.location.to_f64();
    let size = window.size.to_f64();

    point.x >= location.x
        && point.y >= location.y
        && point.x < location.x + size.w
        && point.y < location.y + TITLEBAR_HEIGHT as f64
}

pub(super) fn titlebar_control_at(
    window: &ManagedWindow,
    point: Point<f64, Logical>,
) -> Option<WindowFrameControl> {
    let right = window.location.x + window.size.w - TITLEBAR_CONTROL_RIGHT;
    let y = window.location.y + (TITLEBAR_HEIGHT - TITLEBAR_CONTROL_SIZE) / 2;
    let controls = [
        (WindowFrameControl::Close, right - TITLEBAR_CONTROL_SIZE),
        (
            WindowFrameControl::Maximize,
            right - TITLEBAR_CONTROL_SIZE * 2 - TITLEBAR_CONTROL_GAP,
        ),
        (
            WindowFrameControl::Minimize,
            right - TITLEBAR_CONTROL_SIZE * 3 - TITLEBAR_CONTROL_GAP * 2,
        ),
    ];

    controls
        .into_iter()
        .find(|(_, x)| control_contains(*x, y, point))
        .map(|(control, _)| control)
}

pub(super) fn resize_edge_at(
    window: &ManagedWindow,
    point: Point<f64, Logical>,
) -> Option<ResizeEdge> {
    if !contains(window, point) {
        return None;
    }

    let location = window.location.to_f64();
    let right = location.x + window.size.w as f64;
    let bottom = location.y + window.titlebar_height() as f64 + window.size.h as f64;
    let border = RESIZE_BORDER as f64;
    let left = point.x < location.x + border;
    let right_edge = point.x >= right - border;
    let top = point.y < location.y + border && (left || right_edge);
    let bottom_edge = point.y >= bottom - border;

    ResizeEdge::new(left, right_edge, top, bottom_edge)
}

pub(super) fn modifier_resize_edge_at(
    window: &ManagedWindow,
    point: Point<f64, Logical>,
) -> Option<ResizeEdge> {
    if !contains(window, point) {
        return None;
    }

    let location = window.location.to_f64();
    let center_x = location.x + f64::from(window.size.w) * 0.5;
    let center_y = location.y + f64::from(window.titlebar_height() + window.size.h) * 0.5;
    ResizeEdge::new(
        point.x < center_x,
        point.x >= center_x,
        point.y < center_y,
        point.y >= center_y,
    )
}

fn control_contains(x: i32, y: i32, point: Point<f64, Logical>) -> bool {
    let padding = TITLEBAR_CONTROL_HIT_PADDING;
    point.x >= (x - padding) as f64
        && point.y >= (y - padding) as f64
        && point.x < (x + TITLEBAR_CONTROL_SIZE + padding) as f64
        && point.y < (y + TITLEBAR_CONTROL_SIZE + padding) as f64
}
