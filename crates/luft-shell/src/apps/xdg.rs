use std::{
    env,
    path::{Path, PathBuf},
};

pub(super) fn data_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = env::var_os("XDG_DATA_HOME") {
        dirs.push(PathBuf::from(home));
    } else if let Some(home) = env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(".local/share"));
    }

    let data_dirs =
        env::var_os("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    dirs.extend(env::split_paths(&data_dirs));
    dirs
}

pub(super) fn config_home() -> Option<PathBuf> {
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
}

pub(super) fn command_name(command: &str) -> Option<&str> {
    let first = command
        .split_whitespace()
        .next()?
        .trim_matches('"')
        .trim_matches('\'');
    Path::new(first).file_name()?.to_str()
}
