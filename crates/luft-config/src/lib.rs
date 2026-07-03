use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};
use thiserror::Error;

mod apps;
mod cursor_theme;
mod display;
mod panel;
mod paths;
mod session;

pub use apps::DefaultAppsConfig;
pub use cursor_theme::{
    DEFAULT_CURSOR_SIZE, DEFAULT_CURSOR_THEME_DIR, DEFAULT_CURSOR_THEME_NAME,
    DEFAULT_CURSOR_THEME_PARENT, cursor_environment_entries,
};
pub use display::{DisplayConfig, OutputConfig};
pub use panel::{PanelConfig, PinnedAppConfig};
pub use paths::ConfigPaths;
pub use session::SessionConfig;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LuftConfig {
    pub compositor: CompositorConfig,
    pub display: DisplayConfig,
    pub session: SessionConfig,
    pub workspaces: WorkspacesConfig,
    pub panel: PanelConfig,
    pub default_apps: DefaultAppsConfig,
}

impl LuftConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        for (id, workspace) in &self.workspaces.entries {
            if workspace.name.trim().is_empty() {
                return Err(ConfigError::Validation(format!(
                    "workspace {id} has an empty name"
                )));
            }
        }
        self.display.validate()?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CompositorConfig {
    pub backend: BackendPreference,
    pub xwayland: bool,
    pub background_image: Option<PathBuf>,
}

impl Default for CompositorConfig {
    fn default() -> Self {
        Self {
            backend: BackendPreference::Auto,
            xwayland: true,
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
pub struct WorkspacesConfig {
    pub count: u32,
    #[serde(flatten)]
    pub entries: BTreeMap<String, WorkspaceConfig>,
}

impl Default for WorkspacesConfig {
    fn default() -> Self {
        Self {
            count: 1,
            entries: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub name: String,
}

impl WorkspaceConfig {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoadedConfig {
    pub config: LuftConfig,
    pub source: ConfigSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigSource {
    User(PathBuf),
    Defaults,
}

pub fn load_config() -> Result<LoadedConfig, ConfigError> {
    let paths = ConfigPaths::discover()?;
    load_config_from_path(&paths.config_file)
}

pub fn save_config(config: &LuftConfig) -> Result<PathBuf, ConfigError> {
    let paths = ConfigPaths::discover()?;
    save_config_to_path(&paths.config_file, config)?;
    Ok(paths.config_file)
}

pub fn save_config_to_path(path: &Path, config: &LuftConfig) -> Result<(), ConfigError> {
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
            let config: LuftConfig = toml::from_str(&contents).map_err(ConfigError::Parse)?;
            config.validate()?;
            Ok(LoadedConfig {
                config,
                source: ConfigSource::User(path.to_path_buf()),
            })
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let config = LuftConfig::default();
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

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("HOME is not set, cannot resolve XDG config paths")]
    HomeMissing,
    #[error("failed to read {path}")]
    Read { path: PathBuf, source: io::Error },
    #[error("failed to write {path}")]
    Write { path: PathBuf, source: io::Error },
    #[error("failed to parse config TOML: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("failed to serialize config TOML")]
    Serialize(toml::ser::Error),
    #[error("invalid config: {0}")]
    Validation(String),
}
