use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    env, fs, io,
    path::{Path, PathBuf},
};
use thiserror::Error;

mod appearance;
mod apps;
mod backups;
mod display;
mod dock;
mod paths;
mod performance;
mod session;

pub use appearance::{AppearanceConfig, MaterialModePreference, ShellModePreference};
pub use apps::DefaultAppsConfig;
pub use backups::{list_config_backups, restore_config_backup, restore_latest_config_backup};
pub use display::{DisplayConfig, OutputConfig};
pub use dock::{DockConfig, PinnedAppConfig};
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
    pub recovery: RecoveryConfig,
    pub performance: PerformanceConfig,
    pub dock: DockConfig,
    pub default_apps: DefaultAppsConfig,
}

impl AsherConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.general.default_profile.trim().is_empty() {
            return Err(ConfigError::Validation(
                "general.default_profile cannot be empty".to_string(),
            ));
        }
        if self.recovery.crash_limit == 0 {
            return Err(ConfigError::Validation(
                "recovery.crash_limit must be greater than zero".to_string(),
            ));
        }
        if self.recovery.crash_window_seconds == 0 {
            return Err(ConfigError::Validation(
                "recovery.crash_window_seconds must be greater than zero".to_string(),
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
    pub safe_mode: bool,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            default_profile: "panel-default".to_string(),
            enable_effects: true,
            enable_blur: true,
            enable_animations: true,
            safe_mode: false,
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
    pub disable_blur_on_battery: bool,
}

impl Default for EffectsConfig {
    fn default() -> Self {
        Self {
            background_effect_protocol: true,
            blur: true,
            blur_quality: BlurQuality::Balanced,
            disable_blur_on_battery: false,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RecoveryConfig {
    pub crash_limit: u32,
    pub crash_window_seconds: u64,
    pub auto_safe_mode: bool,
    pub backup_before_apply: bool,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            crash_limit: 3,
            crash_window_seconds: 60,
            auto_safe_mode: true,
            backup_before_apply: true,
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
    backups::save_config_with_backup(
        &paths.config_file,
        config,
        config.recovery.backup_before_apply,
    )?;
    Ok(paths.config_file)
}

pub fn save_config_to_path(path: &Path, config: &AsherConfig) -> Result<(), ConfigError> {
    backups::save_config_with_backup(path, config, false)
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
