use crate::dock::DockApp;

mod browser_launch;
mod desktop_entry;
mod icon_theme;
mod xdg;

pub use desktop_entry::{AppEntry, discover_applications};
pub(crate) use icon_theme::resolve_icon_path;

use staccato_config::StaccatoConfig;
use std::{
    env, io,
    path::PathBuf,
    process::{Child, Command},
};

pub fn dock_apps(config: &StaccatoConfig) -> Vec<DockApp> {
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

pub fn launcher_apps(config: &StaccatoConfig, fallback: &[DockApp]) -> Vec<AppEntry> {
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
    let mut child = Command::new("sh");
    child
        .arg("-lc")
        .arg(browser_launch::command_for_shell(command));
    apply_app_environment(&mut child, xwayland_display);
    child.spawn()
}

fn apply_app_environment(command: &mut Command, xwayland_display: Option<&str>) {
    command.env_remove("DISPLAY");
    command.env("XDG_CURRENT_DESKTOP", "Staccato");
    command.env("XDG_SESSION_DESKTOP", "staccato");
    command.env("DESKTOP_SESSION", "staccato");
    command.env("XDG_SESSION_TYPE", "wayland");
    command.env("NO_AT_BRIDGE", "1");
    command.env("GTK_A11Y", "none");
    command.env("GTK_MODULES", "");
    command.env("GDK_BACKEND", "wayland,x11");
    command.env("QT_QPA_PLATFORM", "wayland;xcb");
    command.env("SDL_VIDEODRIVER", "wayland");
    command.env("CLUTTER_BACKEND", "wayland");
    command.env("MOZ_ENABLE_WAYLAND", "1");
    command.env("ELECTRON_OZONE_PLATFORM_HINT", "wayland");
    command.env_remove("WAYLAND_DISPLAY");
    if let Some(display) = staccato_wayland_display() {
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

fn staccato_wayland_display() -> Option<std::ffi::OsString> {
    if let Some(display) = env::var_os("STACCATO_WAYLAND_DISPLAY") {
        return Some(display);
    }

    env::var("XDG_CURRENT_DESKTOP")
        .is_ok_and(|desktop| desktop.split(':').any(|entry| entry == "Staccato"))
        .then(|| env::var_os("WAYLAND_DISPLAY"))
        .flatten()
}
