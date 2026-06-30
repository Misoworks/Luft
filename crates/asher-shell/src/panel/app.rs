use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct PanelApp {
    pub label: String,
    pub command: String,
    pub icon_path: Option<PathBuf>,
}

impl PanelApp {
    pub fn new(label: String, command: String, icon_path: Option<PathBuf>) -> Self {
        Self {
            label,
            command,
            icon_path,
        }
    }
}
