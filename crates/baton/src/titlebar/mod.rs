use crate::window::{
    TITLEBAR_CONTROL_GAP, TITLEBAR_CONTROL_HIT_PADDING, TITLEBAR_CONTROL_RIGHT,
    TITLEBAR_CONTROL_SIZE, TITLEBAR_HEIGHT,
};
use draw::{Rgba, draw_control_icon, fill_circle, fill_rect_at, fill_top_round_rect};
use smithay::{
    backend::{allocator::Fourcc, renderer::element::memory::MemoryRenderBuffer},
    utils::{Buffer, Rectangle, Size, Transform},
};

mod draw;

pub const TITLEBAR_OVERLAP: i32 = 2;

const CACHE_LIMIT: usize = 64;
const TITLEBAR_CONTROL_HOVER_PADDING: i32 = TITLEBAR_CONTROL_HIT_PADDING;

#[derive(Debug, Default)]
pub struct TitlebarCache {
    entries: Vec<TitlebarCacheEntry>,
}

impl TitlebarCache {
    pub fn buffer(&mut self, width: i32, hover: TitlebarHover, radius: i32) -> &MemoryRenderBuffer {
        let width = width.max(1);
        let radius = radius.max(0);
        if let Some(index) = self.entries.iter().position(|entry| {
            entry.width == width && entry.hover == hover && entry.radius == radius
        }) {
            return &self.entries[index].buffer;
        }

        if self.entries.len() >= CACHE_LIMIT {
            self.entries.remove(0);
        }
        self.entries.push(TitlebarCacheEntry {
            width,
            hover,
            radius,
            buffer: titlebar_buffer(width, hover, radius),
        });
        &self.entries.last().unwrap().buffer
    }
}

#[derive(Debug)]
struct TitlebarCacheEntry {
    width: i32,
    hover: TitlebarHover,
    radius: i32,
    buffer: MemoryRenderBuffer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitlebarHover {
    None,
    Minimize,
    Maximize,
    Close,
}

pub fn hover_state(width: i32, pointer_x: i32, pointer_y: i32) -> TitlebarHover {
    let right = width - TITLEBAR_CONTROL_RIGHT;
    let y = (TITLEBAR_HEIGHT - TITLEBAR_CONTROL_SIZE) / 2;
    let controls = [
        (TitlebarHover::Close, right - TITLEBAR_CONTROL_SIZE),
        (
            TitlebarHover::Maximize,
            right - TITLEBAR_CONTROL_SIZE * 2 - TITLEBAR_CONTROL_GAP,
        ),
        (
            TitlebarHover::Minimize,
            right - TITLEBAR_CONTROL_SIZE * 3 - TITLEBAR_CONTROL_GAP * 2,
        ),
    ];

    controls
        .into_iter()
        .find(|(_, x)| control_hover_contains(pointer_x, pointer_y, *x, y))
        .map(|(hover, _)| hover)
        .unwrap_or(TitlebarHover::None)
}

fn titlebar_buffer(width: i32, hover: TitlebarHover, radius: i32) -> MemoryRenderBuffer {
    let height = (TITLEBAR_HEIGHT + TITLEBAR_OVERLAP).max(1);
    let mut pixels = vec![0; (width * height * 4) as usize];
    fill_top_round_rect(
        &mut pixels,
        width,
        height,
        radius,
        Rgba::new(24, 26, 28, 132),
    );
    fill_rect_at(
        &mut pixels,
        width,
        height,
        0,
        height - TITLEBAR_OVERLAP,
        width,
        TITLEBAR_OVERLAP,
        Rgba::new(20, 22, 24, 72),
    );
    draw_controls(&mut pixels, width, height, hover);

    MemoryRenderBuffer::from_slice(
        &pixels,
        Fourcc::Abgr8888,
        Size::<i32, Buffer>::from((width, height)),
        1,
        Transform::Normal,
        Some(vec![Rectangle::from_size(Size::from((width, height)))]),
    )
}

fn draw_controls(pixels: &mut [u8], width: i32, height: i32, hover: TitlebarHover) {
    let right = width - TITLEBAR_CONTROL_RIGHT;
    let y = (TITLEBAR_HEIGHT - TITLEBAR_CONTROL_SIZE) / 2;
    let controls = [
        (
            TitlebarHover::Minimize,
            right - TITLEBAR_CONTROL_SIZE * 3 - TITLEBAR_CONTROL_GAP * 2,
            Rgba::new(202, 207, 211, 255),
            Rgba::new(246, 190, 72, 255),
        ),
        (
            TitlebarHover::Maximize,
            right - TITLEBAR_CONTROL_SIZE * 2 - TITLEBAR_CONTROL_GAP,
            Rgba::new(202, 207, 211, 255),
            Rgba::new(77, 190, 92, 255),
        ),
        (
            TitlebarHover::Close,
            right - TITLEBAR_CONTROL_SIZE,
            Rgba::new(202, 207, 211, 255),
            Rgba::new(237, 91, 82, 255),
        ),
    ];

    for (control, x, idle, active) in controls {
        let color = if hover == control { active } else { idle };
        fill_circle(
            pixels,
            width,
            height,
            x + TITLEBAR_CONTROL_SIZE / 2,
            y + TITLEBAR_CONTROL_SIZE / 2,
            TITLEBAR_CONTROL_SIZE / 2,
            color,
        );
        if hover == control {
            draw_control_icon(
                pixels,
                width,
                height,
                control,
                x + TITLEBAR_CONTROL_SIZE / 2,
                y + TITLEBAR_CONTROL_SIZE / 2,
            );
        }
    }
}

fn control_hover_contains(pointer_x: i32, pointer_y: i32, x: i32, y: i32) -> bool {
    pointer_x >= x - TITLEBAR_CONTROL_HOVER_PADDING
        && pointer_y >= y - TITLEBAR_CONTROL_HOVER_PADDING
        && pointer_x < x + TITLEBAR_CONTROL_SIZE + TITLEBAR_CONTROL_HOVER_PADDING
        && pointer_y < y + TITLEBAR_CONTROL_SIZE + TITLEBAR_CONTROL_HOVER_PADDING
}
