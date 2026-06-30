use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppearanceConfig {
    pub animations: bool,
    pub panel_icon_size: u16,
    pub panel_magnification: bool,
    pub panel_launcher: bool,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            animations: true,
            panel_icon_size: 40,
            panel_magnification: false,
            panel_launcher: true,
        }
    }
}

impl AppearanceConfig {
    pub(crate) fn validate(&self) -> Result<(), crate::ConfigError> {
        if !(32..=64).contains(&self.panel_icon_size) {
            return Err(crate::ConfigError::Validation(
                "appearance.panel_icon_size must be between 32 and 64".to_string(),
            ));
        }
        Ok(())
    }
}
