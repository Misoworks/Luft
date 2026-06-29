use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DockConfig {
    pub customized: bool,
    pub pinned: Vec<PinnedAppConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinnedAppConfig {
    pub label: String,
    pub command: String,
    pub icon: Option<String>,
}
