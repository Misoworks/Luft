use crate::dock::DockApp;

mod desktop_entry;
mod icon_theme;
mod xdg;

pub use desktop_entry::{AppEntry, discover_applications};
pub(crate) use icon_theme::resolve_icon_path;

use asher_config::AsherConfig;
use std::{
    env, io,
    path::PathBuf,
    process::{Child, Command, Stdio},
};

pub fn dock_apps(config: &AsherConfig) -> Vec<DockApp> {
    if config.dock.customized || !config.dock.pinned.is_empty() {
        let applications = discover_applications(config);
        return config
            .dock
            .pinned
            .iter()
            .map(|app| {
                let matched = applications
                    .iter()
                    .find(|entry| commands_match(&entry.command, &app.command));
                let icon_path = matched
                    .and_then(|entry| entry.icon_path.clone())
                    .or_else(|| resolve_icon_path(app.icon.as_deref()));
                DockApp::new(app.label.clone(), app.command.clone(), icon_path)
            })
            .collect();
    }

    let applications = discover_applications(config);
    vec![
        default_dock_app(
            "Terminal",
            &config.default_apps.terminal,
            &[
                &config.default_apps.terminal,
                "com.mitchellh.ghostty",
                "ghostty",
                "utilities-terminal",
                "org.gnome.Terminal",
                "org.wezfurlong.wezterm",
                "Alacritty",
                "kitty",
                "Terminal",
            ],
            &applications,
        ),
        default_dock_app(
            "Files",
            &config.default_apps.file_manager,
            &[
                &config.default_apps.file_manager,
                "system-file-manager",
                "org.gnome.Nautilus",
                "org.kde.dolphin",
                "nautilus",
                "dolphin",
                "Thunar",
            ],
            &applications,
        ),
        default_dock_app(
            "Browser",
            &config.default_apps.browser,
            &[
                &config.default_apps.browser,
                "web-browser",
                "google-chrome-stable",
                "google-chrome",
                "Google Chrome",
                "chromium",
                "firefox",
                "org.mozilla.firefox",
                "brave-browser",
            ],
            &applications,
        ),
        default_dock_app(
            "Settings",
            &config.default_apps.settings,
            &[
                &config.default_apps.settings,
                "org.gnome.Settings",
                "preferences-system",
                "systemsettings",
                "org.kde.systemsettings",
                "settings",
            ],
            &applications,
        ),
    ]
}

pub fn launcher_apps(config: &AsherConfig, fallback: &[DockApp]) -> Vec<AppEntry> {
    let applications = discover_applications(config);
    if !applications.is_empty() {
        return applications;
    }

    fallback
        .iter()
        .map(|app| AppEntry {
            name: app.label.clone(),
            command: app.command.clone(),
            comment: None,
            icon: None,
            icon_path: None,
        })
        .collect()
}

pub fn spawn_command(command: &str, xwayland_display: Option<&str>) -> io::Result<Child> {
    let mut child = command_for_launch(command);
    apply_app_environment(&mut child, xwayland_display);
    child.spawn()
}

fn command_for_launch(command: &str) -> Command {
    if let Some(argv) = shell_words(command) {
        let mut child = Command::new(&argv[0]);
        child.args(&argv[1..]);
        silence_stdio(&mut child);
        return child;
    }

    let mut child = Command::new("sh");
    child.arg("-lc").arg(command);
    silence_stdio(&mut child);
    child
}

fn silence_stdio(command: &mut Command) {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
}

fn shell_words(command: &str) -> Option<Vec<String>> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut chars = command.chars().peekable();
    let mut quote = None;

    while let Some(ch) = chars.next() {
        match (quote, ch) {
            (Some('\''), '\'') | (Some('"'), '"') => quote = None,
            (Some(_), ch) => current.push(ch),
            (None, '\'' | '"') => quote = Some(ch),
            (None, '\\') => current.push(chars.next()?),
            (None, ch) if ch.is_whitespace() => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            }
            (None, ch @ (';' | '&' | '|' | '<' | '>' | '$' | '`' | '(' | ')' | '{' | '}')) => {
                current.push(ch);
                return None;
            }
            (None, ch) => current.push(ch),
        }
    }

    if quote.is_some() {
        return None;
    }
    if !current.is_empty() {
        words.push(current);
    }
    (!words.is_empty()).then_some(words)
}

fn apply_app_environment(command: &mut Command, xwayland_display: Option<&str>) {
    command.env_remove("DISPLAY");
    command.env("XDG_CURRENT_DESKTOP", "Asher");
    command.env("XDG_SESSION_DESKTOP", "asher");
    command.env("DESKTOP_SESSION", "asher");
    command.env("XDG_SESSION_TYPE", "wayland");
    command.env("NO_AT_BRIDGE", "1");
    command.env("GTK_A11Y", "none");
    command.env("GTK_MODULES", "");
    command.env("UBUNTU_MENUPROXY", "0");
    command.env("GTK_OVERLAY_SCROLLING", "0");
    command.env("GDK_BACKEND", "wayland,x11");
    command.env("QT_QPA_PLATFORM", "wayland;xcb");
    command.env("SDL_VIDEODRIVER", "wayland");
    command.env("CLUTTER_BACKEND", "wayland");
    command.env("MOZ_ENABLE_WAYLAND", "1");
    command.env("ELECTRON_OZONE_PLATFORM_HINT", "wayland");
    command.env_remove("WAYLAND_DISPLAY");
    if let Some(display) = asher_wayland_display() {
        command.env("WAYLAND_DISPLAY", display);
    }
    if let Some(display) = xwayland_display {
        command.env("DISPLAY", display);
        command.env("_JAVA_AWT_WM_NONREPARENTING", "1");
    }
}

fn default_dock_app(
    label: &str,
    command: &str,
    fallback_icons: &[&str],
    applications: &[AppEntry],
) -> DockApp {
    let matched = applications
        .iter()
        .find(|app| commands_match(&app.command, command));
    let icon_path = matched
        .and_then(|app| app.icon_path.clone())
        .or_else(|| resolve_first_icon_path(fallback_icons));
    let label = matched
        .map(|app| app.name.clone())
        .unwrap_or_else(|| label.to_string());

    DockApp::new(label, command.to_string(), icon_path)
}

fn resolve_first_icon_path(icons: &[&str]) -> Option<PathBuf> {
    icons.iter().find_map(|icon| {
        let command = xdg::command_name(icon).unwrap_or(icon);
        resolve_icon_path(Some(command))
    })
}

fn commands_match(left: &str, right: &str) -> bool {
    xdg::command_name(left).is_some_and(|left| xdg::command_name(right) == Some(left))
}

fn asher_wayland_display() -> Option<std::ffi::OsString> {
    if let Some(display) = env::var_os("ASHER_WAYLAND_DISPLAY") {
        return Some(display);
    }

    env::var("XDG_CURRENT_DESKTOP")
        .is_ok_and(|desktop| desktop.split(':').any(|entry| entry == "Asher"))
        .then(|| env::var_os("WAYLAND_DISPLAY"))
        .flatten()
}
