use crate::{ConfigError, ConfigPaths, StaccatoConfig, load_config_from_path};
use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub fn list_config_backups() -> Result<Vec<PathBuf>, ConfigError> {
    let paths = ConfigPaths::discover()?;
    let backups_dir = paths.backups_dir();
    let mut backups = match fs::read_dir(&backups_dir) {
        Ok(entries) => entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("toml"))
            .collect::<Vec<_>>(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(source) => {
            return Err(ConfigError::Read {
                path: backups_dir,
                source,
            });
        }
    };
    backups.sort();
    backups.reverse();
    Ok(backups)
}

pub fn restore_latest_config_backup() -> Result<Option<PathBuf>, ConfigError> {
    let Some(backup) = list_config_backups()?.into_iter().next() else {
        return Ok(None);
    };
    restore_config_backup(&backup)?;
    Ok(Some(backup))
}

pub fn restore_config_backup(backup: &Path) -> Result<(), ConfigError> {
    let loaded = load_config_from_path(backup)?;
    let paths = ConfigPaths::discover()?;
    save_config_with_backup(&paths.config_file, &loaded.config, true)
}

pub(crate) fn save_config_with_backup(
    path: &Path,
    config: &StaccatoConfig,
    backup: bool,
) -> Result<(), ConfigError> {
    config.validate()?;
    crate::ensure_parent_dir(path)?;
    if backup {
        backup_existing_config(path)?;
    }
    let contents = toml::to_string_pretty(config).map_err(ConfigError::Serialize)?;
    fs::write(path, contents).map_err(|source| ConfigError::Write {
        path: path.to_path_buf(),
        source,
    })
}

fn backup_existing_config(path: &Path) -> Result<Option<PathBuf>, ConfigError> {
    if !path.exists() {
        return Ok(None);
    }

    let paths = ConfigPaths::discover()?;
    let backups_dir = paths.backups_dir();
    fs::create_dir_all(&backups_dir).map_err(|source| ConfigError::Write {
        path: backups_dir.clone(),
        source,
    })?;
    let backup = unique_backup_path(&backups_dir);
    fs::copy(path, &backup).map_err(|source| ConfigError::Write {
        path: backup.clone(),
        source,
    })?;
    Ok(Some(backup))
}

fn unique_backup_path(dir: &Path) -> PathBuf {
    let stamp = backup_stamp();
    let base = format!("config-{stamp}.toml");
    let path = dir.join(&base);
    if !path.exists() {
        return path;
    }

    for index in 1..1000 {
        let path = dir.join(format!("config-{stamp}-{index}.toml"));
        if !path.exists() {
            return path;
        }
    }
    dir.join(base)
}

fn backup_stamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    seconds.to_string()
}
