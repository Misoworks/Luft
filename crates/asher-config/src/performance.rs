use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PerformanceConfig {
    pub mode: PerformanceMode,
    pub animations: bool,
    pub reduce_effects_on_battery: bool,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            mode: PerformanceMode::Balanced,
            animations: true,
            reduce_effects_on_battery: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PerformanceMode {
    Quality,
    Balanced,
    Performance,
    Battery,
}
