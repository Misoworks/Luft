use super::{resolve_icon_path, xdg};
use staccato_config::StaccatoConfig;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub struct AppEntry {
    pub name: String,
    pub command: String,
    pub comment: Option<String>,
    pub icon: Option<String>,
    pub icon_path: Option<PathBuf>,
}

pub fn discover_applications(config: &StaccatoConfig) -> Vec<AppEntry> {
    let mut entries = BTreeMap::new();

    for dir in xdg::data_dirs() {
        let applications = dir.join("applications");
        collect_desktop_entries(&applications, config, &mut entries);
    }

    entries.into_values().collect()
}

fn collect_desktop_entries(
    dir: &Path,
    config: &StaccatoConfig,
    entries: &mut BTreeMap<String, AppEntry>,
) {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return;
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_desktop_entries(&path, config, entries);
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("desktop") {
            continue;
        }
        if let Some(app) = parse_desktop_entry(&path, config) {
            entries.entry(app.name.to_lowercase()).or_insert(app);
        }
    }
}

fn parse_desktop_entry(path: &Path, config: &StaccatoConfig) -> Option<AppEntry> {
    let content = fs::read_to_string(path).ok()?;
    let mut in_desktop_entry = false;
    let mut values = BTreeMap::new();

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            in_desktop_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_desktop_entry {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        values
            .entry(key.to_string())
            .or_insert(value.trim().to_string());
    }

    if values
        .get("Type")
        .is_some_and(|value| value != "Application")
    {
        return None;
    }
    if truthy(values.get("NoDisplay")) || truthy(values.get("Hidden")) {
        return None;
    }

    let name = values.get("Name")?.trim().to_string();
    let exec = clean_exec(values.get("Exec")?)?;
    let command = if truthy(values.get("Terminal")) {
        format!("{} -e {}", config.default_apps.terminal, exec)
    } else {
        exec
    };

    let icon = values.get("Icon").map(|value| value.trim().to_string());
    let icon_path = resolve_icon_path(icon.as_deref())
        .or_else(|| resolve_icon_path(xdg::command_name(&command)));

    Some(AppEntry {
        name,
        command,
        comment: values.get("Comment").cloned(),
        icon,
        icon_path,
    })
}

fn truthy(value: Option<&String>) -> bool {
    value.is_some_and(|value| value.eq_ignore_ascii_case("true"))
}

fn clean_exec(exec: &str) -> Option<String> {
    let mut cleaned = String::new();
    let mut chars = exec.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '%' {
            cleaned.push(ch);
            continue;
        }

        match chars.next() {
            Some('%') => cleaned.push('%'),
            Some('f' | 'F' | 'u' | 'U' | 'd' | 'D' | 'n' | 'N' | 'i' | 'c' | 'k' | 'm' | 'v') => {}
            Some(other) => {
                cleaned.push('%');
                cleaned.push(other);
            }
            None => cleaned.push('%'),
        }
    }

    let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    (!cleaned.is_empty()).then_some(cleaned)
}
