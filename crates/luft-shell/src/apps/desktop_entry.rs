use super::{normalize_launch_command, resolve_icon_path, xdg};
use luft_config::LuftConfig;
use std::{
    collections::BTreeMap,
    env, fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

const APPLICATION_CACHE_TTL: Duration = Duration::from_secs(3);
static APPLICATION_CACHE: OnceLock<Mutex<ApplicationCache>> = OnceLock::new();

#[derive(Default)]
struct ApplicationCache {
    entries: Vec<AppEntry>,
    scanned_at: Option<Instant>,
    terminal: String,
}

#[derive(Debug, Clone)]
pub struct AppEntry {
    pub desktop_id: Option<String>,
    pub name: String,
    pub command: String,
    pub comment: Option<String>,
    pub icon: Option<String>,
    pub icon_path: Option<PathBuf>,
    pub startup_wm_class: Option<String>,
}

pub fn discover_applications(config: &LuftConfig) -> Vec<AppEntry> {
    let cache = APPLICATION_CACHE.get_or_init(|| Mutex::new(ApplicationCache::default()));
    if let Ok(cache) = cache.lock()
        && cache
            .scanned_at
            .is_some_and(|scanned_at| scanned_at.elapsed() < APPLICATION_CACHE_TTL)
        && cache.terminal == config.default_apps.terminal
    {
        return cache.entries.clone();
    }

    let mut entries = BTreeMap::new();

    for dir in xdg::data_dirs() {
        let applications = dir.join("applications");
        collect_desktop_entries(&applications, config, &mut entries);
    }

    let entries = entries.into_values().collect::<Vec<_>>();
    if let Ok(mut cache) = cache.lock() {
        cache.entries.clone_from(&entries);
        cache.scanned_at = Some(Instant::now());
        cache.terminal.clone_from(&config.default_apps.terminal);
    }
    entries
}

pub fn discover_user_autostart(config: &LuftConfig) -> Vec<AppEntry> {
    let Some(dir) = xdg::config_home().map(|config_home| config_home.join("autostart")) else {
        return Vec::new();
    };
    let mut entries = BTreeMap::new();
    collect_autostart_entries(&dir, config, &mut entries);
    entries.into_values().collect()
}

fn collect_desktop_entries(
    dir: &Path,
    config: &LuftConfig,
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
        if let Some(app) = parse_desktop_entry(&path, config, DesktopEntryContext::Applications) {
            entries.entry(app.name.to_lowercase()).or_insert(app);
        }
    }
}

fn collect_autostart_entries(
    dir: &Path,
    config: &LuftConfig,
    entries: &mut BTreeMap<String, AppEntry>,
) {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return;
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("desktop") {
            continue;
        }
        if let Some(app) = parse_desktop_entry(&path, config, DesktopEntryContext::Autostart) {
            entries.entry(app.name.to_lowercase()).or_insert(app);
        }
    }
}

#[derive(Clone, Copy)]
enum DesktopEntryContext {
    Applications,
    Autostart,
}

fn parse_desktop_entry(
    path: &Path,
    config: &LuftConfig,
    context: DesktopEntryContext,
) -> Option<AppEntry> {
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
    if !entry_visible_in_context(&values, context) {
        return None;
    }
    if values
        .get("TryExec")
        .is_some_and(|value| !try_exec_available(value))
    {
        return None;
    }

    let name = values.get("Name")?.trim().to_string();
    let exec = clean_exec(values.get("Exec")?)?;
    let command = if truthy(values.get("Terminal")) {
        format!("{} -e {}", config.default_apps.terminal, exec)
    } else {
        exec
    };
    let command = normalize_launch_command(&command);

    let icon = values.get("Icon").map(|value| value.trim().to_string());
    let icon_path = resolve_icon_path(icon.as_deref())
        .or_else(|| resolve_icon_path(xdg::command_name(&command)));
    let startup_wm_class = values
        .get("StartupWMClass")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    Some(AppEntry {
        desktop_id: desktop_id(path),
        name,
        command,
        comment: values.get("Comment").cloned(),
        icon,
        icon_path,
        startup_wm_class,
    })
}

fn desktop_id(path: &Path) -> Option<String> {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(ToString::to_string)
}

fn entry_visible_in_context(
    values: &BTreeMap<String, String>,
    context: DesktopEntryContext,
) -> bool {
    if truthy(values.get("Hidden")) {
        return false;
    }

    match context {
        DesktopEntryContext::Applications => !truthy(values.get("NoDisplay")),
        DesktopEntryContext::Autostart => {
            if values
                .get("OnlyShowIn")
                .is_some_and(|value| !desktop_list_contains(value, "Luft"))
            {
                return false;
            }
            if values
                .get("NotShowIn")
                .is_some_and(|value| desktop_list_contains(value, "Luft"))
            {
                return false;
            }
            true
        }
    }
}

fn truthy(value: Option<&String>) -> bool {
    value.is_some_and(|value| value.eq_ignore_ascii_case("true"))
}

fn desktop_list_contains(value: &str, desktop: &str) -> bool {
    value
        .split(';')
        .any(|entry| entry.trim().eq_ignore_ascii_case(desktop))
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

    let cleaned = cleaned.trim().to_string();
    (!cleaned.is_empty()).then_some(cleaned)
}

fn try_exec_available(value: &str) -> bool {
    let Some(command) = value
        .split_whitespace()
        .next()
        .map(|value| value.trim_matches('"').trim_matches('\''))
        .filter(|value| !value.is_empty())
    else {
        return false;
    };

    let path = Path::new(command);
    if path.is_absolute() || command.contains('/') {
        return executable_file(path);
    }

    let Some(path) = env::var_os("PATH") else {
        return false;
    };
    env::split_paths(&path).any(|dir| executable_file(&dir.join(command)))
}

fn executable_file(path: &Path) -> bool {
    fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}
