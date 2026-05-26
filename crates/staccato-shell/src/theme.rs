use crate::color::Color;
use image::ImageReader;
use staccato_config::StaccatoConfig;
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub struct ShellPalette {
    pub panel: Color,
    pub panel_control: Color,
    pub panel_text: Color,
    pub dock: Color,
}

impl Default for ShellPalette {
    fn default() -> Self {
        Self {
            panel: Color::rgba(18, 20, 22, 150),
            panel_control: Color::rgba(45, 50, 54, 96),
            panel_text: Color::rgba(244, 246, 248, 255),
            dock: Color::rgba(18, 20, 22, 86),
        }
    }
}

pub fn shell_palette(config: &StaccatoConfig) -> ShellPalette {
    config
        .compositor
        .background_image
        .as_deref()
        .and_then(load_wallpaper_average)
        .map(palette_from_wallpaper)
        .unwrap_or_default()
}

fn load_wallpaper_average(path: &Path) -> Option<[u8; 3]> {
    let image = ImageReader::open(path).ok()?.decode().ok()?.to_rgb8();
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return None;
    }

    let step_x = (width / 96).max(1);
    let step_y = (height / 96).max(1);
    let mut total = [0u64; 3];
    let mut count = 0u64;

    for y in (0..height).step_by(step_y as usize) {
        for x in (0..width).step_by(step_x as usize) {
            let pixel = image.get_pixel(x, y);
            total[0] += pixel[0] as u64;
            total[1] += pixel[1] as u64;
            total[2] += pixel[2] as u64;
            count += 1;
        }
    }

    (count > 0).then_some([
        (total[0] / count) as u8,
        (total[1] / count) as u8,
        (total[2] / count) as u8,
    ])
}

fn palette_from_wallpaper(average: [u8; 3]) -> ShellPalette {
    let dark = Color::rgba(14, 16, 18, 255);
    let soft = Color::rgba(34, 37, 40, 255);
    let source = Color::rgba(average[0], average[1], average[2], 255);

    ShellPalette {
        panel: mix_color(source, dark, 0.78).with_alpha(0.58),
        panel_control: mix_color(source, soft, 0.62).with_alpha(0.34),
        dock: mix_color(source, dark, 0.74).with_alpha(0.34),
        ..ShellPalette::default()
    }
}

pub fn mix_color(left: Color, right: Color, amount: f32) -> Color {
    let amount = amount.clamp(0.0, 1.0);
    Color::rgba(
        mix_channel(left.r, right.r, amount),
        mix_channel(left.g, right.g, amount),
        mix_channel(left.b, right.b, amount),
        mix_channel(left.a, right.a, amount),
    )
}

fn mix_channel(left: u8, right: u8, amount: f32) -> u8 {
    (left as f32 + (right as f32 - left as f32) * amount).round() as u8
}
