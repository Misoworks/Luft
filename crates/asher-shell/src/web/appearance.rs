use asher_config::AsherConfig;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebAppearance {
    pub animations_enabled: bool,
    pub panel_icon_size: u16,
    pub panel_magnification: bool,
    pub panel_launcher: bool,
}

impl WebAppearance {
    pub fn from_config(config: &AsherConfig) -> Self {
        Self {
            animations_enabled: config.general.enable_animations
                && config.performance.animations
                && config.appearance.animations,
            panel_icon_size: config.appearance.panel_icon_size,
            panel_magnification: config.appearance.panel_magnification,
            panel_launcher: config.appearance.panel_launcher,
        }
    }
}
