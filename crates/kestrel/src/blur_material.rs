use crate::layers::LayerMaterial;
use smithay::{
    backend::{allocator::Fourcc, renderer::element::memory::MemoryRenderBuffer},
    utils::{Buffer, Logical, Physical, Point, Rectangle, Size, Transform},
};

const BLUR_RADIUS: i32 = 5;
const GLASS_SATURATION: f32 = 1.04;
const GLASS_BRIGHTNESS: f32 = 1.0;
const GLASS_NOISE: f32 = 1.1;
const GLASS_TINT: [f32; 3] = [22.0, 21.0, 19.0];
const GLASS_TINT_AMOUNT: f32 = 0.0;

pub fn build_blur_patch_for_material(
    source: &[u8],
    source_size: Size<i32, Physical>,
    location: Point<i32, Logical>,
    size: Size<i32, Logical>,
    material: LayerMaterial,
) -> MemoryRenderBuffer {
    let width = size.w.max(1);
    let height = size.h.max(1);
    let scale = blur_downscale(width, height);
    let low_width = div_ceil(width, scale).max(1);
    let low_height = div_ceil(height, scale).max(1);
    let mut low_pixels = vec![0; (low_width * low_height * 4) as usize];

    for y in 0..low_height {
        for x in 0..low_width {
            let offset_x = (x * scale + scale / 2).min(width - 1);
            let offset_y = (y * scale + scale / 2).min(height - 1);
            let source_x = (location.x + offset_x).clamp(0, source_size.w - 1);
            let source_y = (location.y + offset_y).clamp(0, source_size.h - 1);
            let source_index = ((source_y * source_size.w + source_x) * 4) as usize;
            let target_index = ((y * low_width + x) * 4) as usize;
            low_pixels[target_index..target_index + 4]
                .copy_from_slice(&source[source_index..source_index + 4]);
        }
    }

    box_blur(&mut low_pixels, low_width, low_height, BLUR_RADIUS);
    box_blur(&mut low_pixels, low_width, low_height, 2);
    let pixels = upscale_material(
        location,
        &low_pixels,
        low_width,
        low_height,
        width,
        height,
        material,
    );
    MemoryRenderBuffer::from_slice(
        &pixels,
        Fourcc::Abgr8888,
        Size::<i32, Buffer>::from((width, height)),
        1,
        Transform::Normal,
        Some(vec![Rectangle::from_size(Size::from((width, height)))]),
    )
}

fn blur_downscale(width: i32, height: i32) -> i32 {
    let area = width.saturating_mul(height);
    if area >= 420_000 {
        12
    } else if area >= 120_000 {
        10
    } else {
        7
    }
}

fn div_ceil(value: i32, divisor: i32) -> i32 {
    (value + divisor - 1) / divisor
}

fn upscale_material(
    location: Point<i32, Logical>,
    blurred: &[u8],
    blurred_width: i32,
    blurred_height: i32,
    width: i32,
    height: i32,
    material: LayerMaterial,
) -> Vec<u8> {
    let mut pixels = vec![0; (width * height * 4) as usize];
    for y in 0..height {
        for x in 0..width {
            let mut glass =
                sample_pixel(blurred, blurred_width, blurred_height, x, y, width, height);
            apply_glass_treatment(&mut glass, x, y, location);
            let coverage = material_coverage(material, x, y, width, height);
            let pixel = glass_pixel(glass, coverage);
            let index = ((y * width + x) * 4) as usize;
            pixels[index..index + 4].copy_from_slice(&pixel);
        }
    }
    pixels
}

fn apply_glass_treatment(pixel: &mut [u8; 4], x: i32, y: i32, location: Point<i32, Logical>) {
    let luma = pixel_luma(pixel);
    let noise = material_noise(x + location.x, y + location.y) * GLASS_NOISE;
    for (channel_index, channel) in pixel.iter_mut().take(3).enumerate() {
        let saturated = luma + (*channel as f32 - luma) * GLASS_SATURATION;
        let dimmed = saturated * GLASS_BRIGHTNESS;
        let tinted = dimmed + (GLASS_TINT[channel_index] - dimmed) * GLASS_TINT_AMOUNT;
        *channel = (tinted + noise).round().clamp(0.0, 255.0) as u8;
    }
    pixel[3] = 255;
}

fn glass_pixel(mut pixel: [u8; 4], coverage: f32) -> [u8; 4] {
    let alpha = (coverage.clamp(0.0, 1.0) * 255.0).round() as u8;
    pixel[0] = premultiply(pixel[0], alpha);
    pixel[1] = premultiply(pixel[1], alpha);
    pixel[2] = premultiply(pixel[2], alpha);
    pixel[3] = alpha;
    pixel
}

fn premultiply(value: u8, alpha: u8) -> u8 {
    (u32::from(value) * u32::from(alpha) / 255) as u8
}

fn pixel_luma(pixel: &[u8; 4]) -> f32 {
    pixel[0] as f32 * 0.2126 + pixel[1] as f32 * 0.7152 + pixel[2] as f32 * 0.0722
}

fn material_noise(x: i32, y: i32) -> f32 {
    let mut value = (x as u32).wrapping_mul(0x45d9f3b) ^ (y as u32).wrapping_mul(0x119de1f3);
    value ^= value >> 16;
    value = value.wrapping_mul(0x45d9f3b);
    value ^= value >> 16;
    value as f32 / u32::MAX as f32 - 0.5
}

fn sample_pixel(
    pixels: &[u8],
    source_width: i32,
    source_height: i32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> [u8; 4] {
    let source_x = ((x as f32 + 0.5) * source_width as f32 / width as f32 - 0.5).max(0.0);
    let source_y = ((y as f32 + 0.5) * source_height as f32 / height as f32 - 0.5).max(0.0);
    let x0 = source_x.floor().min((source_width - 1) as f32) as i32;
    let y0 = source_y.floor().min((source_height - 1) as f32) as i32;
    let x1 = (x0 + 1).min(source_width - 1);
    let y1 = (y0 + 1).min(source_height - 1);
    let tx = source_x - source_x.floor();
    let ty = source_y - source_y.floor();
    let top = mix_pixel(
        read_pixel(pixels, source_width, x0, y0),
        read_pixel(pixels, source_width, x1, y0),
        tx,
    );
    let bottom = mix_pixel(
        read_pixel(pixels, source_width, x0, y1),
        read_pixel(pixels, source_width, x1, y1),
        tx,
    );
    mix_pixel(top, bottom, ty)
}

fn read_pixel(pixels: &[u8], width: i32, x: i32, y: i32) -> [u8; 4] {
    let index = ((y * width + x) * 4) as usize;
    [
        pixels[index],
        pixels[index + 1],
        pixels[index + 2],
        pixels[index + 3],
    ]
}

fn mix_pixel(left: [u8; 4], right: [u8; 4], amount: f32) -> [u8; 4] {
    [
        mix_channel(left[0], right[0], amount),
        mix_channel(left[1], right[1], amount),
        mix_channel(left[2], right[2], amount),
        mix_channel(left[3], right[3], amount),
    ]
}

fn mix_channel(left: u8, right: u8, amount: f32) -> u8 {
    (left as f32 + (right as f32 - left as f32) * amount).round() as u8
}

fn material_coverage(material: LayerMaterial, x: i32, y: i32, width: i32, height: i32) -> f32 {
    match material {
        LayerMaterial::Rect => 1.0,
        LayerMaterial::RoundRect { radius } => round_rect_coverage(x, y, width, height, radius),
        LayerMaterial::RoundLeft { radius } => {
            left_round_rect_coverage(x, y, width, height, radius)
        }
        LayerMaterial::RoundRight { radius } => {
            right_round_rect_coverage(x, y, width, height, radius)
        }
    }
}

fn round_rect_coverage(x: i32, y: i32, width: i32, height: i32, radius: i32) -> f32 {
    let radius = radius.max(0).min(width / 2).min(height / 2);
    if radius == 0 {
        return 1.0;
    }

    if (x >= radius && x < width - radius) || (y >= radius && y < height - radius) {
        return 1.0;
    }

    const SAMPLES: [(f64, f64); 16] = [
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
    let radius = radius as f64;
    let hits = SAMPLES
        .iter()
        .filter(|(sx, sy)| inside_round_rect(x as f64 + sx, y as f64 + sy, width, height, radius))
        .count();
    hits as f32 / SAMPLES.len() as f32
}

fn inside_round_rect(x: f64, y: f64, width: i32, height: i32, radius: f64) -> bool {
    let left = radius;
    let right = width as f64 - radius;
    let top = radius;
    let bottom = height as f64 - radius;
    let cx = x.clamp(left, right);
    let cy = y.clamp(top, bottom);
    let dx = x - cx;
    let dy = y - cy;
    dx * dx + dy * dy <= radius * radius
}

fn left_round_rect_coverage(x: i32, y: i32, width: i32, height: i32, radius: i32) -> f32 {
    if x >= radius.max(0).min(width) {
        return 1.0;
    }

    round_rect_coverage(x, y, width + radius.max(0), height, radius)
}

fn right_round_rect_coverage(x: i32, y: i32, width: i32, height: i32, radius: i32) -> f32 {
    if x < width - radius.max(0).min(width) {
        return 1.0;
    }

    round_rect_coverage(x + radius.max(0), y, width + radius.max(0), height, radius)
}

fn box_blur(pixels: &mut [u8], width: i32, height: i32, radius: i32) {
    if radius <= 0 || width <= 1 || height <= 1 {
        return;
    }

    let mut scratch = pixels.to_vec();
    blur_horizontal(pixels, &mut scratch, width, height, radius);
    blur_vertical(&scratch, pixels, width, height, radius);
}

fn blur_horizontal(source: &[u8], target: &mut [u8], width: i32, height: i32, radius: i32) {
    for y in 0..height {
        let mut left = 0;
        let mut right = -1;
        let mut total = [0u32; 4];
        let mut count = 0u32;

        for x in 0..width {
            let target_left = (x - radius).max(0);
            let target_right = (x + radius).min(width - 1);
            while left < target_left {
                subtract_pixel(source, &mut total, (y * width + left) as usize);
                count -= 1;
                left += 1;
            }
            while right < target_right {
                right += 1;
                add_pixel(source, &mut total, (y * width + right) as usize);
                count += 1;
            }
            write_average(target, (y * width + x) as usize, total, count);
        }
    }
}

fn blur_vertical(source: &[u8], target: &mut [u8], width: i32, height: i32, radius: i32) {
    for x in 0..width {
        let mut top = 0;
        let mut bottom = -1;
        let mut total = [0u32; 4];
        let mut count = 0u32;

        for y in 0..height {
            let target_top = (y - radius).max(0);
            let target_bottom = (y + radius).min(height - 1);
            while top < target_top {
                subtract_pixel(source, &mut total, (top * width + x) as usize);
                count -= 1;
                top += 1;
            }
            while bottom < target_bottom {
                bottom += 1;
                add_pixel(source, &mut total, (bottom * width + x) as usize);
                count += 1;
            }
            write_average(target, (y * width + x) as usize, total, count);
        }
    }
}

fn add_pixel(source: &[u8], total: &mut [u32; 4], pixel: usize) {
    let index = pixel * 4;
    total[0] += source[index] as u32;
    total[1] += source[index + 1] as u32;
    total[2] += source[index + 2] as u32;
    total[3] += source[index + 3] as u32;
}

fn subtract_pixel(source: &[u8], total: &mut [u32; 4], pixel: usize) {
    let index = pixel * 4;
    total[0] -= source[index] as u32;
    total[1] -= source[index + 1] as u32;
    total[2] -= source[index + 2] as u32;
    total[3] -= source[index + 3] as u32;
}

fn write_average(target: &mut [u8], target_pixel: usize, total: [u32; 4], count: u32) {
    let index = target_pixel * 4;
    target[index] = (total[0] / count) as u8;
    target[index + 1] = (total[1] / count) as u8;
    target[index + 2] = (total[2] / count) as u8;
    target[index + 3] = (total[3] / count) as u8;
}
