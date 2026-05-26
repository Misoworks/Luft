use crate::window::{MIN_WINDOW_HEIGHT, MIN_WINDOW_WIDTH, ResizeEdge};
use smithay::utils::{Logical, Point};
use staccato_layout::Rect;

pub fn move_geometry(
    start: Rect,
    pointer_start: Point<f64, Logical>,
    pointer: Point<f64, Logical>,
) -> Rect {
    Rect::new(
        start.x + (pointer.x - pointer_start.x).round() as i32,
        start.y + (pointer.y - pointer_start.y).round() as i32,
        start.width,
        start.height,
    )
}

pub fn resize_geometry(
    start: Rect,
    edge: ResizeEdge,
    pointer_start: Point<f64, Logical>,
    pointer: Point<f64, Logical>,
) -> Rect {
    let dx = (pointer.x - pointer_start.x).round() as i32;
    let dy = (pointer.y - pointer_start.y).round() as i32;
    let mut x = start.x;
    let mut y = start.y;
    let mut width = start.width;
    let mut height = start.height;

    if edge.left {
        x = start.x + dx;
        width = start.width - dx;
    } else if edge.right {
        width = start.width + dx;
    }

    if edge.top {
        y = start.y + dy;
        height = start.height - dy;
    } else if edge.bottom {
        height = start.height + dy;
    }

    if width < MIN_WINDOW_WIDTH {
        if edge.left {
            x = start.x + start.width - MIN_WINDOW_WIDTH;
        }
        width = MIN_WINDOW_WIDTH;
    }

    if height < MIN_WINDOW_HEIGHT {
        if edge.top {
            y = start.y + start.height - MIN_WINDOW_HEIGHT;
        }
        height = MIN_WINDOW_HEIGHT;
    }

    Rect::new(x, y, width, height)
}
