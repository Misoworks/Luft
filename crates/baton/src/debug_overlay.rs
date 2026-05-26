use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            element::{
                Kind,
                memory::{MemoryRenderBuffer, MemoryRenderBufferRenderElement},
            },
            gles::{GlesError, GlesRenderer},
        },
    },
    utils::{Buffer, Logical, Rectangle, Size, Transform},
};
use std::time::{Duration, Instant};

const WIDTH: i32 = 336;
const HEIGHT: i32 = 148;
const GLYPH_WIDTH: i32 = 5;
const SCALE: i32 = 2;
const DEBUG_X: i32 = 12;
const DEBUG_Y: i32 = 44;
const DEBUG_UPDATE_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Debug)]
pub struct DebugOverlayStats<'a> {
    pub backend: &'a str,
    pub frame_ms: f32,
    pub fps: u32,
    pub idle: bool,
    pub target_hz: u32,
    pub damage_area: i32,
    pub surfaces: usize,
    pub blur_passes: usize,
    pub workspace: &'a str,
    pub profile: &'a str,
    pub xwayland: &'a str,
}

#[derive(Default)]
pub struct DebugOverlayCache {
    frame: Option<DebugOverlayFrame>,
}

struct DebugOverlayFrame {
    key: String,
    refreshed_at: Instant,
    buffer: MemoryRenderBuffer,
}

impl DebugOverlayCache {
    pub fn clear(&mut self) {
        self.frame = None;
    }

    pub fn needs_refresh(&self) -> bool {
        match self.frame.as_ref() {
            Some(frame) => {
                Instant::now().duration_since(frame.refreshed_at) >= DEBUG_UPDATE_INTERVAL
            }
            None => true,
        }
    }

    fn buffer(&mut self, stats: &DebugOverlayStats<'_>) -> &MemoryRenderBuffer {
        let now = Instant::now();
        let key = stable_key(stats);
        let update = match self.frame.as_ref() {
            Some(frame) => {
                frame.key != key || now.duration_since(frame.refreshed_at) >= DEBUG_UPDATE_INTERVAL
            }
            None => true,
        };

        if update {
            self.frame = Some(DebugOverlayFrame {
                key,
                refreshed_at: now,
                buffer: rasterize_debug_overlay(stats),
            });
        }

        &self.frame.as_ref().expect("debug overlay buffer").buffer
    }
}

pub fn render_debug_overlay(
    cache: &mut DebugOverlayCache,
    renderer: &mut GlesRenderer,
    stats: &DebugOverlayStats<'_>,
) -> Result<MemoryRenderBufferRenderElement<GlesRenderer>, GlesError> {
    let buffer = cache.buffer(stats);
    MemoryRenderBufferRenderElement::from_buffer(
        renderer,
        (DEBUG_X as f64, DEBUG_Y as f64),
        buffer,
        None,
        None,
        Some(Size::<i32, Logical>::from((WIDTH, HEIGHT))),
        Kind::Unspecified,
    )
}

fn stable_key(stats: &DebugOverlayStats<'_>) -> String {
    format!(
        "{}:{}:{}:{}:{}:{}:{}:{}",
        stats.backend,
        stats.idle,
        stats.target_hz,
        stats.surfaces,
        stats.blur_passes,
        stats.workspace,
        stats.profile,
        stats.xwayland
    )
}

fn rasterize_debug_overlay(stats: &DebugOverlayStats<'_>) -> MemoryRenderBuffer {
    let mut pixels = vec![0; (WIDTH * HEIGHT * 4) as usize];
    fill_round_rect(
        &mut pixels,
        0,
        0,
        WIDTH,
        HEIGHT,
        16,
        Rgba::new(14, 15, 16, 206),
    );
    stroke_round_rect(
        &mut pixels,
        0,
        0,
        WIDTH,
        HEIGHT,
        16,
        Rgba::new(255, 255, 255, 28),
    );

    let fps_line = if stats.idle {
        format!("FPS IDLE  TARGET {:>3}HZ", stats.target_hz)
    } else {
        format!("FPS {:>3}   TARGET {:>3}HZ", stats.fps, stats.target_hz)
    };
    let damage_area = if stats.idle { 0 } else { stats.damage_area };
    let lines = [
        "BATON DEBUG".to_string(),
        fps_line,
        format!("FRAME {:>4.1}MS  DAMAGE {:>6}", stats.frame_ms, damage_area),
        format!("SURF {:>3}   BLUR {:>3}", stats.surfaces, stats.blur_passes),
        format!("BACKEND {}", stats.backend),
        format!("WS {}  PROFILE {}", stats.workspace, stats.profile),
        format!("XWAYLAND {}", stats.xwayland),
    ];
    for (index, line) in lines.iter().enumerate() {
        draw_text(
            &mut pixels,
            14,
            12 + index as i32 * 18,
            &line.to_uppercase(),
            if index == 0 {
                Rgba::new(233, 219, 179, 235)
            } else {
                Rgba::new(236, 238, 234, 218)
            },
        );
    }

    MemoryRenderBuffer::from_slice(
        &pixels,
        Fourcc::Abgr8888,
        Size::<i32, Buffer>::from((WIDTH, HEIGHT)),
        1,
        Transform::Normal,
        Some(vec![Rectangle::from_size(Size::from((WIDTH, HEIGHT)))]),
    )
}

fn draw_text(pixels: &mut [u8], x: i32, y: i32, value: &str, color: Rgba) {
    let mut cursor = x;
    for character in value.chars() {
        if character == ' ' {
            cursor += (GLYPH_WIDTH + 1) * SCALE;
            continue;
        }
        draw_glyph(pixels, cursor, y, character, color);
        cursor += (GLYPH_WIDTH + 1) * SCALE;
    }
}

fn draw_glyph(pixels: &mut [u8], x: i32, y: i32, character: char, color: Rgba) {
    let glyph = glyph(character);
    for (row, pattern) in glyph.iter().enumerate() {
        for (col, pixel) in pattern.bytes().enumerate() {
            if pixel == b'1' {
                fill_rect(
                    pixels,
                    x + col as i32 * SCALE,
                    y + row as i32 * SCALE,
                    SCALE,
                    SCALE,
                    color,
                );
            }
        }
    }
}

fn glyph(character: char) -> [&'static str; 7] {
    match character {
        '0' => [
            "11111", "10001", "10011", "10101", "11001", "10001", "11111",
        ],
        '1' => [
            "00100", "01100", "00100", "00100", "00100", "00100", "01110",
        ],
        '2' => [
            "11110", "00001", "00001", "11110", "10000", "10000", "11111",
        ],
        '3' => [
            "11110", "00001", "00001", "01110", "00001", "00001", "11110",
        ],
        '4' => [
            "10010", "10010", "10010", "11111", "00010", "00010", "00010",
        ],
        '5' => [
            "11111", "10000", "10000", "11110", "00001", "00001", "11110",
        ],
        '6' => [
            "01111", "10000", "10000", "11110", "10001", "10001", "01110",
        ],
        '7' => [
            "11111", "00001", "00010", "00100", "01000", "01000", "01000",
        ],
        '8' => [
            "01110", "10001", "10001", "01110", "10001", "10001", "01110",
        ],
        '9' => [
            "01110", "10001", "10001", "01111", "00001", "00001", "11110",
        ],
        'A' => [
            "01110", "10001", "10001", "11111", "10001", "10001", "10001",
        ],
        'B' => [
            "11110", "10001", "10001", "11110", "10001", "10001", "11110",
        ],
        'C' => [
            "01111", "10000", "10000", "10000", "10000", "10000", "01111",
        ],
        'D' => [
            "11110", "10001", "10001", "10001", "10001", "10001", "11110",
        ],
        'E' => [
            "11111", "10000", "10000", "11110", "10000", "10000", "11111",
        ],
        'F' => [
            "11111", "10000", "10000", "11110", "10000", "10000", "10000",
        ],
        'G' => [
            "01111", "10000", "10000", "10011", "10001", "10001", "01111",
        ],
        'H' => [
            "10001", "10001", "10001", "11111", "10001", "10001", "10001",
        ],
        'I' => [
            "11111", "00100", "00100", "00100", "00100", "00100", "11111",
        ],
        'J' => [
            "00111", "00010", "00010", "00010", "00010", "10010", "01100",
        ],
        'K' => [
            "10001", "10010", "10100", "11000", "10100", "10010", "10001",
        ],
        'L' => [
            "10000", "10000", "10000", "10000", "10000", "10000", "11111",
        ],
        'M' => [
            "10001", "11011", "10101", "10101", "10001", "10001", "10001",
        ],
        'N' => [
            "10001", "11001", "10101", "10011", "10001", "10001", "10001",
        ],
        'O' => [
            "01110", "10001", "10001", "10001", "10001", "10001", "01110",
        ],
        'P' => [
            "11110", "10001", "10001", "11110", "10000", "10000", "10000",
        ],
        'Q' => [
            "01110", "10001", "10001", "10001", "10101", "10010", "01101",
        ],
        'R' => [
            "11110", "10001", "10001", "11110", "10100", "10010", "10001",
        ],
        'S' => [
            "01111", "10000", "10000", "01110", "00001", "00001", "11110",
        ],
        'T' => [
            "11111", "00100", "00100", "00100", "00100", "00100", "00100",
        ],
        'U' => [
            "10001", "10001", "10001", "10001", "10001", "10001", "01110",
        ],
        'V' => [
            "10001", "10001", "10001", "10001", "01010", "01010", "00100",
        ],
        'W' => [
            "10001", "10001", "10001", "10101", "10101", "10101", "01010",
        ],
        'X' => [
            "10001", "01010", "00100", "00100", "00100", "01010", "10001",
        ],
        'Y' => [
            "10001", "01010", "00100", "00100", "00100", "00100", "00100",
        ],
        'Z' => [
            "11111", "00001", "00010", "00100", "01000", "10000", "11111",
        ],
        '.' => [
            "00000", "00000", "00000", "00000", "00000", "01100", "01100",
        ],
        ':' => [
            "00000", "01100", "01100", "00000", "01100", "01100", "00000",
        ],
        '-' => [
            "00000", "00000", "00000", "11111", "00000", "00000", "00000",
        ],
        '/' => [
            "00001", "00010", "00010", "00100", "01000", "01000", "10000",
        ],
        '_' => [
            "00000", "00000", "00000", "00000", "00000", "00000", "11111",
        ],
        _ => [
            "11111", "00001", "00010", "00100", "00100", "00000", "00100",
        ],
    }
}

fn fill_round_rect(
    pixels: &mut [u8],
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    radius: i32,
    color: Rgba,
) {
    for py in y..(y + height) {
        for px in x..(x + width) {
            if inside_round_rect(px, py, x, y, width, height, radius) {
                blend_pixel(pixels, px, py, color);
            }
        }
    }
}

fn stroke_round_rect(
    pixels: &mut [u8],
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    radius: i32,
    color: Rgba,
) {
    fill_round_rect(pixels, x, y, width, height, radius, color);
    fill_round_rect(
        pixels,
        x + 1,
        y + 1,
        width - 2,
        height - 2,
        radius - 1,
        Rgba::new(0, 0, 0, 0),
    );
}

fn inside_round_rect(
    px: i32,
    py: i32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    radius: i32,
) -> bool {
    let radius = radius.max(0).min(width / 2).min(height / 2);
    let left = x + radius;
    let right = x + width - radius - 1;
    let top = y + radius;
    let bottom = y + height - radius - 1;
    let cx = px.clamp(left, right);
    let cy = py.clamp(top, bottom);
    let dx = px - cx;
    let dy = py - cy;
    dx * dx + dy * dy <= radius * radius
}

fn fill_rect(pixels: &mut [u8], x: i32, y: i32, width: i32, height: i32, color: Rgba) {
    for py in y.max(0)..(y + height).min(HEIGHT) {
        for px in x.max(0)..(x + width).min(WIDTH) {
            blend_pixel(pixels, px, py, color);
        }
    }
}

fn blend_pixel(pixels: &mut [u8], x: i32, y: i32, color: Rgba) {
    if color.a == 0 || x < 0 || y < 0 || x >= WIDTH || y >= HEIGHT {
        return;
    }
    let index = ((y * WIDTH + x) * 4) as usize;
    let source_a = color.a as u32;
    let inverse_a = 255 - source_a;
    pixels[index] = composite(
        premultiply(color.r, color.a) as u32,
        pixels[index] as u32,
        inverse_a,
    );
    pixels[index + 1] = composite(
        premultiply(color.g, color.a) as u32,
        pixels[index + 1] as u32,
        inverse_a,
    );
    pixels[index + 2] = composite(
        premultiply(color.b, color.a) as u32,
        pixels[index + 2] as u32,
        inverse_a,
    );
    pixels[index + 3] = composite(source_a, pixels[index + 3] as u32, inverse_a);
}

#[derive(Debug, Clone, Copy)]
struct Rgba {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl Rgba {
    const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
}

fn premultiply(value: u8, alpha: u8) -> u8 {
    (value as u32 * alpha as u32 / 255) as u8
}

fn composite(source: u32, dest: u32, inverse_alpha: u32) -> u8 {
    (source + dest * inverse_alpha / 255).min(255) as u8
}
