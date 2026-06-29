use super::TitlebarHover;

const CIRCLE_SAMPLES: [(f64, f64); 16] = [
    (0.125, 0.125),
    (0.375, 0.125),
    (0.625, 0.125),
    (0.875, 0.125),
    (0.125, 0.375),
    (0.375, 0.375),
    (0.625, 0.375),
    (0.875, 0.375),
    (0.125, 0.625),
    (0.375, 0.625),
    (0.625, 0.625),
    (0.875, 0.625),
    (0.125, 0.875),
    (0.375, 0.875),
    (0.625, 0.875),
    (0.875, 0.875),
];

#[derive(Debug, Clone, Copy)]
pub(super) struct Rgba {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PixelRect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

impl PixelRect {
    pub(super) const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

impl Rgba {
    pub(super) const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    fn with_alpha(self, coverage: f32) -> Self {
        Self {
            a: ((self.a as f32 * coverage).round() as i32).clamp(0, 255) as u8,
            ..self
        }
    }
}

pub(super) fn fill_top_round_rect(
    pixels: &mut [u8],
    width: i32,
    height: i32,
    radius: i32,
    color: Rgba,
) {
    fill_round_rect_at(
        pixels,
        width,
        height,
        PixelRect::new(0, 0, width, height),
        radius,
        color,
    );
    fill_rect_at(
        pixels,
        width,
        height,
        PixelRect::new(0, radius.min(height), width, height - radius.min(height)),
        color,
    );
}

fn fill_round_rect_at(
    pixels: &mut [u8],
    width: i32,
    height: i32,
    rect: PixelRect,
    radius: i32,
    color: Rgba,
) {
    let PixelRect {
        x,
        y,
        width: rect_width,
        height: rect_height,
    } = rect;
    let x0 = x.max(0);
    let y0 = y.max(0);
    let x1 = (x + rect_width).min(width);
    let y1 = (y + rect_height).min(height);
    let radius = radius.max(0).min(rect_width / 2).min(rect_height / 2);
    for py in y0..y1 {
        for px in x0..x1 {
            let coverage = round_rect_coverage(px, py, x, y, rect_width, rect_height, radius);
            if coverage > 0.0 {
                blend_pixel(pixels, width, px, py, color.with_alpha(coverage));
            }
        }
    }
}

pub(super) fn fill_rect_at(
    pixels: &mut [u8],
    width: i32,
    height: i32,
    rect: PixelRect,
    color: Rgba,
) {
    let PixelRect {
        x,
        y,
        width: rect_width,
        height: rect_height,
    } = rect;
    let x0 = x.max(0);
    let y0 = y.max(0);
    let x1 = (x + rect_width).min(width);
    let y1 = (y + rect_height).min(height);
    for py in y0..y1 {
        for px in x0..x1 {
            write_pixel(pixels, width, px, py, color);
        }
    }
}

pub(super) fn fill_circle(
    pixels: &mut [u8],
    width: i32,
    height: i32,
    center_x: i32,
    center_y: i32,
    radius: i32,
    color: Rgba,
) {
    let radius = radius.max(1);
    for y in (center_y - radius)..=(center_y + radius) {
        for x in (center_x - radius)..=(center_x + radius) {
            if x < 0 || y < 0 || x >= width || y >= height {
                continue;
            }
            let coverage = circle_coverage(x, y, center_x, center_y, radius);
            if coverage > 0.0 {
                blend_pixel(pixels, width, x, y, color.with_alpha(coverage));
            }
        }
    }
}

pub(super) fn draw_control_icon(
    pixels: &mut [u8],
    width: i32,
    height: i32,
    control: TitlebarHover,
    center_x: i32,
    center_y: i32,
) {
    let color = Rgba::new(45, 45, 42, 230);
    let cx = f64::from(center_x) + 0.5;
    let cy = f64::from(center_y) + 0.5;
    let stroke = 0.9;
    match control {
        TitlebarHover::Close => {
            draw_line_aa(
                pixels,
                width,
                height,
                cx - 2.05,
                cy - 2.05,
                cx + 2.05,
                cy + 2.05,
                stroke,
                color,
            );
            draw_line_aa(
                pixels,
                width,
                height,
                cx - 2.05,
                cy + 2.05,
                cx + 2.05,
                cy - 2.05,
                stroke,
                color,
            );
        }
        TitlebarHover::Minimize => {
            draw_line_aa(
                pixels,
                width,
                height,
                cx - 2.35,
                cy,
                cx + 2.35,
                cy,
                stroke,
                color,
            );
        }
        TitlebarHover::Maximize => {
            draw_line_aa(
                pixels,
                width,
                height,
                cx - 2.3,
                cy - 1.45,
                cx - 2.3,
                cy + 1.35,
                stroke,
                color,
            );
            draw_line_aa(
                pixels,
                width,
                height,
                cx - 2.3,
                cy + 1.35,
                cx + 0.45,
                cy + 1.35,
                stroke,
                color,
            );
            draw_line_aa(
                pixels,
                width,
                height,
                cx - 0.45,
                cy - 1.35,
                cx + 2.3,
                cy - 1.35,
                stroke,
                color,
            );
            draw_line_aa(
                pixels,
                width,
                height,
                cx + 2.3,
                cy - 1.35,
                cx + 2.3,
                cy + 1.45,
                stroke,
                color,
            );
        }
        TitlebarHover::None => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_line_aa(
    pixels: &mut [u8],
    width: i32,
    height: i32,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    stroke: f64,
    color: Rgba,
) {
    let pad = (stroke * 0.5 + 1.0).ceil() as i32;
    let min_x = (x0.min(x1).floor() as i32 - pad).max(0);
    let max_x = (x0.max(x1).ceil() as i32 + pad).min(width - 1);
    let min_y = (y0.min(y1).floor() as i32 - pad).max(0);
    let max_y = (y0.max(y1).ceil() as i32 + pad).min(height - 1);

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let distance = distance_to_segment(x as f64 + 0.5, y as f64 + 0.5, x0, y0, x1, y1);
            let coverage = (stroke * 0.5 + 0.55 - distance).clamp(0.0, 1.0) as f32;
            if coverage > 0.0 {
                blend_pixel(pixels, width, x, y, color.with_alpha(coverage));
            }
        }
    }
}

fn circle_coverage(x: i32, y: i32, center_x: i32, center_y: i32, radius: i32) -> f32 {
    let radius_squared = (radius * radius) as f64;
    let hits = CIRCLE_SAMPLES
        .iter()
        .filter(|(sx, sy)| {
            let dx = x as f64 + sx - center_x as f64;
            let dy = y as f64 + sy - center_y as f64;
            dx * dx + dy * dy <= radius_squared
        })
        .count();
    hits as f32 / CIRCLE_SAMPLES.len() as f32
}

fn round_rect_coverage(
    px: i32,
    py: i32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    radius: i32,
) -> f32 {
    let radius = f64::from(radius);
    let hits = CIRCLE_SAMPLES
        .iter()
        .filter(|(sx, sy)| {
            let sample_x = px as f64 + sx;
            let sample_y = py as f64 + sy;
            let left = x as f64 + radius;
            let right = (x + width) as f64 - radius;
            let top = y as f64 + radius;
            let bottom = (y + height) as f64 - radius;
            let cx = sample_x.clamp(left, right);
            let cy = sample_y.clamp(top, bottom);
            let dx = sample_x - cx;
            let dy = sample_y - cy;
            dx * dx + dy * dy <= radius * radius
        })
        .count();
    hits as f32 / CIRCLE_SAMPLES.len() as f32
}

fn distance_to_segment(px: f64, py: f64, x0: f64, y0: f64, x1: f64, y1: f64) -> f64 {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let length_squared = dx * dx + dy * dy;
    if length_squared == 0.0 {
        return ((px - x0).powi(2) + (py - y0).powi(2)).sqrt();
    }
    let t = (((px - x0) * dx + (py - y0) * dy) / length_squared).clamp(0.0, 1.0);
    let x = x0 + t * dx;
    let y = y0 + t * dy;
    ((px - x).powi(2) + (py - y).powi(2)).sqrt()
}

fn write_pixel(pixels: &mut [u8], width: i32, x: i32, y: i32, color: Rgba) {
    let index = ((y * width + x) * 4) as usize;
    pixels[index] = premultiply(color.r, color.a);
    pixels[index + 1] = premultiply(color.g, color.a);
    pixels[index + 2] = premultiply(color.b, color.a);
    pixels[index + 3] = color.a;
}

fn blend_pixel(pixels: &mut [u8], width: i32, x: i32, y: i32, color: Rgba) {
    let index = ((y * width + x) * 4) as usize;
    let source_a = u32::from(color.a);
    let inverse_a = 255 - source_a;
    pixels[index] = composite(
        u32::from(premultiply(color.r, color.a)),
        u32::from(pixels[index]),
        inverse_a,
    );
    pixels[index + 1] = composite(
        u32::from(premultiply(color.g, color.a)),
        u32::from(pixels[index + 1]),
        inverse_a,
    );
    pixels[index + 2] = composite(
        u32::from(premultiply(color.b, color.a)),
        u32::from(pixels[index + 2]),
        inverse_a,
    );
    pixels[index + 3] = composite(source_a, u32::from(pixels[index + 3]), inverse_a);
}

fn premultiply(value: u8, alpha: u8) -> u8 {
    (u32::from(value) * u32::from(alpha) / 255) as u8
}

fn composite(source: u32, dest: u32, inverse_alpha: u32) -> u8 {
    (source + dest * inverse_alpha / 255).min(255) as u8
}
