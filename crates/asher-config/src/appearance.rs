use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppearanceConfig {
    pub material_mode: MaterialModePreference,
    pub shell_mode: ShellModePreference,
    pub dock_icon_size: u16,
    pub dock_magnification: bool,
    pub taskbar_launcher: bool,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            material_mode: MaterialModePreference::Glass,
            shell_mode: ShellModePreference::Panel,
            dock_icon_size: 40,
            dock_magnification: true,
            taskbar_launcher: true,
        }
    }
}

impl AppearanceConfig {
    pub(crate) fn validate(&self) -> Result<(), crate::ConfigError> {
        if !(32..=64).contains(&self.dock_icon_size) {
            return Err(crate::ConfigError::Validation(
                "appearance.dock_icon_size must be between 32 and 64".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MaterialModePreference {
    #[default]
    Glass,
}

impl MaterialModePreference {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Glass => "glass",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShellModePreference {
    Panel,
    Dock,
    Tiling,
    Focus,
    Browser,
}

impl ShellModePreference {
    pub fn profile_id(self) -> &'static str {
        match self {
            Self::Panel => "panel-default",
            Self::Dock => "dock-default",
            Self::Tiling => "tiling-dev",
            Self::Focus => "focus-writing",
            Self::Browser => "browser-dev",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Panel => "panel",
            Self::Dock => "dock",
            Self::Tiling => "tiling",
            Self::Focus => "focus",
            Self::Browser => "browser",
        }
    }
}
