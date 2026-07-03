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
    output::Output,
    utils::{Buffer, Logical, Physical, Rectangle, Size, Transform},
};

use crate::layers;
use luft_ipc::ShellStatus;

const WIDTH: i32 = 96;
const HEIGHT: i32 = 96;
const SPINNER_SAMPLES: i32 = 4;
const TAU: f64 = std::f64::consts::PI * 2.0;

pub fn render_loading_overlay(
    renderer: &mut GlesRenderer,
    output_size: Size<i32, Physical>,
    phase: f32,
) -> Result<MemoryRenderBufferRenderElement<GlesRenderer>, GlesError> {
    let mut pixels = vec![0; (WIDTH * HEIGHT * 4) as usize];
    draw_spinner(&mut pixels, phase);

    let buffer = MemoryRenderBuffer::from_slice(
        &pixels,
        Fourcc::Abgr8888,
        Size::<i32, Buffer>::from((WIDTH, HEIGHT)),
        1,
        Transform::Normal,
        Some(vec![Rectangle::from_size(Size::from((WIDTH, HEIGHT)))]),
    );
    let geometry = loading_overlay_geometry(output_size);

    MemoryRenderBufferRenderElement::from_buffer(
        renderer,
        (geometry.loc.x as f64, geometry.loc.y as f64),
        &buffer,
        None,
        None,
        Some(Size::<i32, Logical>::from((WIDTH, HEIGHT))),
        Kind::Unspecified,
    )
}

pub fn loading_overlay_geometry(output_size: Size<i32, Physical>) -> Rectangle<i32, Physical> {
    Rectangle::new(
        (
            ((output_size.w - WIDTH) / 2).max(0),
            ((output_size.h - HEIGHT) / 2).max(0),
        )
            .into(),
        (WIDTH, HEIGHT).into(),
    )
}

pub fn shell_layers_ready(output: &Output, shell_status: ShellStatus) -> bool {
    shell_status == ShellStatus::Running && layers::has_panel_surface(output)
}

pub fn should_show_loading_overlay(
    shell_layers_ready: bool,
    shell_layers_seen_ready: bool,
) -> bool {
    !shell_layers_ready || !shell_layers_seen_ready
}

fn draw_spinner(pixels: &mut [u8], phase: f32) {
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let mut alpha = 0.0;
            for sample_y in 0..SPINNER_SAMPLES {
                for sample_x in 0..SPINNER_SAMPLES {
                    let px = x as f64 + (sample_x as f64 + 0.5) / f64::from(SPINNER_SAMPLES);
                    let py = y as f64 + (sample_y as f64 + 0.5) / f64::from(SPINNER_SAMPLES);
                    alpha += spinner_alpha(px, py, phase);
                }
            }
            let samples = f64::from(SPINNER_SAMPLES * SPINNER_SAMPLES);
            let alpha = (alpha / samples * 236.0).round() as u8;
            write_pixel(pixels, x, y, Rgba::new(255, 255, 255, alpha));
        }
    }
}

fn spinner_alpha(x: f64, y: f64, phase: f32) -> f64 {
    let center = (WIDTH as f64 - 1.0) * 0.5;
    let radius = 18.0;
    let thickness = 3.0;
    let head = f64::from(phase.rem_euclid(1.0)) * TAU;
    let length = TAU * 0.72;
    let dx = x - center;
    let dy = y - center;
    let distance = (dx * dx + dy * dy).sqrt();
    let ring = ((thickness * 0.5 + 0.75) - (distance - radius).abs()).clamp(0.0, 1.0);
    if ring <= 0.0 {
        return 0.0;
    }

    let angle = dy.atan2(dx).rem_euclid(TAU);
    let tail = (head - angle).rem_euclid(TAU);
    if tail > length {
        return 0.0;
    }

    ring * (1.0 - tail / length).powf(0.7)
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

fn write_pixel(pixels: &mut [u8], x: i32, y: i32, color: Rgba) {
    if color.a == 0 {
        return;
    }
    let index = ((y * WIDTH + x) * 4) as usize;
    pixels[index] = premultiply(color.r, color.a);
    pixels[index + 1] = premultiply(color.g, color.a);
    pixels[index + 2] = premultiply(color.b, color.a);
    pixels[index + 3] = color.a;
}

fn premultiply(value: u8, alpha: u8) -> u8 {
    (value as u32 * alpha as u32 / 255) as u8
}
