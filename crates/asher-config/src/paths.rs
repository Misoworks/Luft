use crate::ConfigError;
use std::{env, path::PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigPaths {
    pub config_home: PathBuf,
    pub config_file: PathBuf,
    pub profiles_dir: PathBuf,
    pub materials_dir: PathBuf,
    pub state_home: PathBuf,
    pub cache_home: PathBuf,
}

impl ConfigPaths {
    pub fn discover() -> Result<Self, ConfigError> {
        let home = env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or(ConfigError::HomeMissing)?;
        let config_home = xdg_dir("XDG_CONFIG_HOME", &home, ".config").join("asher");
        let state_home = xdg_dir("XDG_STATE_HOME", &home, ".local/state").join("asher");
        let cache_home = xdg_dir("XDG_CACHE_HOME", &home, ".cache").join("asher");

        Ok(Self {
            config_file: config_home.join("config.toml"),
            profiles_dir: config_home.join("profiles"),
            materials_dir: config_home.join("materials"),
            config_home,
            state_home,
            cache_home,
        })
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.state_home.join("logs")
    }

    pub fn backups_dir(&self) -> PathBuf {
        self.config_home.join("backups")
    }

    pub fn log_file(&self, component: &str) -> PathBuf {
        self.logs_dir()
            .join(format!("{}.log", safe_path_segment(component)))
    }
}

fn xdg_dir(variable: &str, home: &std::path::Path, fallback: &str) -> PathBuf {
    env::var_os(variable)
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(fallback))
}

fn safe_path_segment(value: &str) -> String {
    let segment = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();

    if segment.is_empty() {
        "asher".to_string()
    } else {
        segment
    }
}
