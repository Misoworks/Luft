use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    env, fs, io,
    path::{Path, PathBuf},
};
use thiserror::Error;

mod appearance;
mod apps;
mod cursor_theme;
mod display;
mod panel;
mod paths;
mod performance;
mod session;

pub use appearance::AppearanceConfig;
pub use apps::DefaultAppsConfig;
pub use cursor_theme::{
    DEFAULT_CURSOR_SIZE, DEFAULT_CURSOR_THEME_DIR, DEFAULT_CURSOR_THEME_NAME,
    DEFAULT_CURSOR_THEME_PARENT, cursor_environment_entries,
};
pub use display::{DisplayConfig, OutputConfig};
pub use panel::{PanelConfig, PinnedAppConfig};
pub use paths::ConfigPaths;
pub use performance::{PerformanceConfig, PerformanceMode};
pub use session::SessionConfig;

pub const IGNORE_USER_CONFIG_ENV: &str = "ASHER_IGNORE_USER_CONFIG";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AsherConfig {
    pub general: GeneralConfig,
    pub compositor: CompositorConfig,
    pub display: DisplayConfig,
    pub session: SessionConfig,
    pub appearance: AppearanceConfig,
    pub effects: EffectsConfig,
    pub workspaces: WorkspacesConfig,
    pub performance: PerformanceConfig,
    pub panel: PanelConfig,
    pub default_apps: DefaultAppsConfig,
}

impl AsherConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.general.default_profile.trim().is_empty() {
            return Err(ConfigError::Validation(
                "general.default_profile cannot be empty".to_string(),
            ));
        }
        for (id, workspace) in &self.workspaces.entries {
            if workspace.name.trim().is_empty() {
                return Err(ConfigError::Validation(format!(
                    "workspace {id} has an empty name"
                )));
            }
            if workspace.profile.trim().is_empty() {
                return Err(ConfigError::Validation(format!(
                    "workspace {id} has an empty profile"
                )));
            }
        }
        self.appearance.validate()?;
        self.display.validate()?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub default_profile: String,
    pub enable_effects: bool,
    pub enable_blur: bool,
    pub enable_animations: bool,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            default_profile: "panel-default".to_string(),
            enable_effects: true,
            enable_blur: true,
            enable_animations: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CompositorConfig {
    pub backend: BackendPreference,
    pub xwayland: bool,
    pub debug_overlay: bool,
    pub background_image: Option<PathBuf>,
}

impl Default for CompositorConfig {
    fn default() -> Self {
        Self {
            backend: BackendPreference::Auto,
            xwayland: true,
            debug_overlay: false,
            background_image: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BackendPreference {
    Auto,
    Nested,
    Headless,
    Session,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EffectsConfig {
    pub background_effect_protocol: bool,
    pub blur: bool,
    pub blur_quality: BlurQuality,
}

impl Default for EffectsConfig {
    fn default() -> Self {
        Self {
            background_effect_protocol: true,
            blur: true,
            blur_quality: BlurQuality::Balanced,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BlurQuality {
    Quality,
    Balanced,
    Performance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkspacesConfig {
    pub count: u32,
    pub restore_sessions: bool,
    #[serde(flatten)]
    pub entries: BTreeMap<String, WorkspaceConfig>,
}

impl Default for WorkspacesConfig {
    fn default() -> Self {
        Self {
            count: 1,
            restore_sessions: true,
            entries: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub name: String,
    pub profile: String,
}

impl WorkspaceConfig {
    pub fn new(name: impl Into<String>, profile: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            profile: profile.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoadedConfig {
    pub config: AsherConfig,
    pub source: ConfigSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigSource {
    User(PathBuf),
    Defaults,
}

pub fn load_config() -> Result<LoadedConfig, ConfigError> {
    if ignore_user_config() {
        return Ok(LoadedConfig {
            config: AsherConfig::default(),
            source: ConfigSource::Defaults,
        });
    }

    let paths = ConfigPaths::discover()?;
    load_config_from_path(&paths.config_file)
}

pub fn load_config_or_default() -> (LoadedConfig, Option<ConfigError>) {
    match load_config() {
        Ok(loaded) => (loaded, None),
        Err(error) => {
            let config = AsherConfig::default();
            (
                LoadedConfig {
                    config,
                    source: ConfigSource::Defaults,
                },
                Some(error),
            )
        }
    }
}

pub fn save_config(config: &AsherConfig) -> Result<PathBuf, ConfigError> {
    let paths = ConfigPaths::discover()?;
    save_config_to_path(&paths.config_file, config)?;
    Ok(paths.config_file)
}

pub fn save_config_to_path(path: &Path, config: &AsherConfig) -> Result<(), ConfigError> {
    config.validate()?;
    ensure_parent_dir(path)?;
    let contents = toml::to_string_pretty(config).map_err(ConfigError::Serialize)?;
    fs::write(path, contents).map_err(|source| ConfigError::Write {
        path: path.to_path_buf(),
        source,
    })
}

pub fn load_config_from_path(path: &Path) -> Result<LoadedConfig, ConfigError> {
    match fs::read_to_string(path) {
        Ok(contents) => {
            let config: AsherConfig = toml::from_str(&contents).map_err(ConfigError::Parse)?;
            config.validate()?;
            Ok(LoadedConfig {
                config,
                source: ConfigSource::User(path.to_path_buf()),
            })
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let config = AsherConfig::default();
            config.validate()?;
            Ok(LoadedConfig {
                config,
                source: ConfigSource::Defaults,
            })
        }
        Err(error) => Err(ConfigError::Read {
            path: path.to_path_buf(),
            source: error,
        }),
    }
}

pub(crate) fn ensure_parent_dir(path: &Path) -> Result<(), ConfigError> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    fs::create_dir_all(parent).map_err(|source| ConfigError::Write {
        path: parent.to_path_buf(),
        source,
    })
}

fn ignore_user_config() -> bool {
    env::var_os(IGNORE_USER_CONFIG_ENV).is_some_and(|value| {
        let value = value.to_string_lossy();
        !matches!(value.as_ref(), "" | "0" | "false" | "False" | "FALSE")
    })
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("HOME is not set, cannot resolve XDG config paths")]
    HomeMissing,
    #[error("failed to read {path}")]
    Read { path: PathBuf, source: io::Error },
    #[error("failed to write {path}")]
    Write { path: PathBuf, source: io::Error },
    #[error("failed to parse config TOML")]
    Parse(#[from] toml::de::Error),
    #[error("failed to serialize config TOML")]
    Serialize(toml::ser::Error),
    #[error("invalid config: {0}")]
    Validation(String),
}
