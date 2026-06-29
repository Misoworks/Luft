use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DefaultAppsConfig {
    pub terminal: String,
    pub file_manager: String,
    pub browser: String,
    pub settings: String,
    pub launcher: String,
}

impl Default for DefaultAppsConfig {
    fn default() -> Self {
        Self {
            terminal: "ghostty".to_string(),
            file_manager: "rover".to_string(),
            browser: "google-chrome-stable".to_string(),
            settings: "asher-settings".to_string(),
            launcher: "vicinae".to_string(),
        }
    }
}
