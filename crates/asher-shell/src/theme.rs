use crate::color::Color;
use asher_config::AsherConfig;
use image::{ImageReader, imageops::FilterType};
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub struct ShellPalette {
    pub panel: Color,
    pub panel_control: Color,
    pub panel_text: Color,
    pub panel_bar: Color,
    pub accent: Color,
    pub text_soft: Color,
    pub text_muted: Color,
}

impl Default for ShellPalette {
    fn default() -> Self {
        Self {
            panel: Color::rgba(22, 22, 20, 158),
            panel_control: Color::rgba(255, 255, 255, 20),
            panel_text: Color::rgba(248, 248, 246, 245),
            panel_bar: Color::rgba(24, 23, 20, 86),
            accent: Color::rgba(210, 192, 130, 255),
            text_soft: Color::rgba(218, 216, 205, 232),
            text_muted: Color::rgba(164, 162, 154, 222),
        }
    }
}

pub fn shell_palette(config: &AsherConfig) -> ShellPalette {
    let mut palette = ShellPalette::default();
    if let Some(path) = config.compositor.background_image.as_deref()
        && let Some(accent) = wallpaper_accent(path)
    {
        palette.accent = accent;
    }
    palette
}

fn wallpaper_accent(path: &Path) -> Option<Color> {
    let image = ImageReader::open(path).ok()?.decode().ok()?;
    let image = image.resize(64, 64, FilterType::Triangle).to_rgba8();
    let mut total = 0.0f32;
    let mut r_total = 0.0f32;
    let mut g_total = 0.0f32;
    let mut b_total = 0.0f32;

    for (_, _, pixel) in image.enumerate_pixels() {
        let [r, g, b, a] = pixel.0;
        if a < 128 {
            continue;
        }

        let rgb = [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0];
        let max = rgb[0].max(rgb[1]).max(rgb[2]);
        let min = rgb[0].min(rgb[1]).min(rgb[2]);
        let chroma = max - min;
        let lightness = (max + min) * 0.5;
        let luma = rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722;
        if !(0.16..=0.88).contains(&luma) || chroma < 0.035 {
            continue;
        }

        let mid_lightness = 1.0 - (lightness - 0.58).abs().min(0.58) / 0.58;
        let weight = (chroma * 1.8 + mid_lightness * 0.55).powf(1.35);
        total += weight;
        r_total += rgb[0] * weight;
        g_total += rgb[1] * weight;
        b_total += rgb[2] * weight;
    }

    if total <= f32::EPSILON {
        return None;
    }

    let (h, s, l) = rgb_to_hsl(r_total / total, g_total / total, b_total / total);
    let saturation = (s * 1.22).clamp(0.26, 0.72);
    let lightness = l.clamp(0.48, 0.72);
    let (r, g, b) = hsl_to_rgb(h, saturation, lightness);
    Some(Color::rgba(
        (r * 255.0).round().clamp(0.0, 255.0) as u8,
        (g * 255.0).round().clamp(0.0, 255.0) as u8,
        (b * 255.0).round().clamp(0.0, 255.0) as u8,
        255,
    ))
}

fn rgb_to_hsl(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let lightness = (max + min) * 0.5;
    let chroma = max - min;
    if chroma <= f32::EPSILON {
        return (0.0, 0.0, lightness);
    }

    let saturation = chroma / (1.0 - (2.0 * lightness - 1.0).abs()).max(f32::EPSILON);
    let hue = if max == r {
        ((g - b) / chroma).rem_euclid(6.0)
    } else if max == g {
        (b - r) / chroma + 2.0
    } else {
        (r - g) / chroma + 4.0
    } / 6.0;
    (hue, saturation, lightness)
}

fn hsl_to_rgb(hue: f32, saturation: f32, lightness: f32) -> (f32, f32, f32) {
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let segment = hue * 6.0;
    let x = chroma * (1.0 - (segment.rem_euclid(2.0) - 1.0).abs());
    let (r1, g1, b1) = if segment < 1.0 {
        (chroma, x, 0.0)
    } else if segment < 2.0 {
        (x, chroma, 0.0)
    } else if segment < 3.0 {
        (0.0, chroma, x)
    } else if segment < 4.0 {
        (0.0, x, chroma)
    } else if segment < 5.0 {
        (x, 0.0, chroma)
    } else {
        (chroma, 0.0, x)
    };
    let m = lightness - chroma * 0.5;
    (r1 + m, g1 + m, b1 + m)
}
