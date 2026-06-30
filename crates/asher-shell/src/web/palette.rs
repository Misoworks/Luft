use crate::{color::Color, theme::ShellPalette};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebPalette {
    pub panel: String,
    pub panel_control: String,
    pub panel_text: String,
    pub panel_bar: String,
    pub accent: String,
    pub text_soft: String,
    pub text_muted: String,
}

impl From<ShellPalette> for WebPalette {
    fn from(value: ShellPalette) -> Self {
        Self {
            panel: css_color(value.panel),
            panel_control: css_color(value.panel_control),
            panel_text: css_color(value.panel_text),
            panel_bar: css_color(value.panel_bar),
            accent: css_color(value.accent),
            text_soft: css_color(value.text_soft),
            text_muted: css_color(value.text_muted),
        }
    }
}

fn css_color(color: Color) -> String {
    format!(
        "rgba({}, {}, {}, {:.3})",
        color.r,
        color.g,
        color.b,
        color.a as f32 / 255.0
    )
}
