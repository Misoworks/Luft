use crate::panel::PanelApp;

mod desktop_entry;
mod icon_theme;
mod xdg;

pub use desktop_entry::{AppEntry, discover_applications, discover_user_autostart};
pub(crate) use icon_theme::resolve_icon_path;

use luft_config::{ConfigPaths, LuftConfig, cursor_environment_entries};
use std::{
    env,
    fs::OpenOptions,
    io,
    path::PathBuf,
    process::{Child, Command, Stdio},
};

pub fn panel_apps(config: &LuftConfig) -> Vec<PanelApp> {
    if config.panel.customized || !config.panel.pinned.is_empty() {
        let applications = discover_applications(config);
        return config
            .panel
            .pinned
            .iter()
            .map(|app| {
                let matched = applications
                    .iter()
                    .find(|entry| commands_match(&entry.command, &app.command));
                let icon_path = matched
                    .and_then(|entry| entry.icon_path.clone())
                    .or_else(|| resolve_icon_path(app.icon.as_deref()));
                PanelApp::new(
                    app.label.clone(),
                    normalize_launch_command(&app.command),
                    icon_path,
                )
            })
            .collect();
    }

    let applications = discover_applications(config);
    vec![
        default_panel_app(
            "Terminal",
            &config.default_apps.terminal,
            &[
                &config.default_apps.terminal,
                "com.mitchellh.ghostty",
                "ghostty",
                "utilities-terminal",
                "org.wezfurlong.wezterm",
                "Alacritty",
                "kitty",
                "Terminal",
            ],
            &applications,
        ),
        default_panel_app(
            "Files",
            &config.default_apps.file_manager,
            &[
                &config.default_apps.file_manager,
                "system-file-manager",
                "org.kde.dolphin",
                "dolphin",
                "Thunar",
            ],
            &applications,
        ),
        default_panel_app(
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
    ]
}

pub fn launcher_apps(config: &LuftConfig, fallback: &[PanelApp]) -> Vec<AppEntry> {
    let applications = discover_applications(config);
    if !applications.is_empty() {
        return applications;
    }

    fallback
        .iter()
        .map(|app| AppEntry {
            desktop_id: None,
            name: app.label.clone(),
            command: normalize_launch_command(&app.command),
            comment: None,
            icon: None,
            icon_path: None,
            startup_wm_class: None,
        })
        .collect()
}

pub fn spawn_command(command: &str, xwayland_display: Option<&str>) -> io::Result<Child> {
    let command = normalize_launch_command(command);
    log_app_launch(&command);
    let mut child = command_for_launch(&command);
    apply_app_environment(&mut child, xwayland_display);
    child.spawn()
}

pub(crate) fn normalize_launch_command(command: &str) -> String {
    clean_exec_forwarding_tokens(&clean_exec_placeholders(
        &percent_decode(command).unwrap_or_else(|| command.to_string()),
    ))
}

fn percent_decode(value: &str) -> Option<String> {
    if !value.as_bytes().contains(&b'%') {
        return None;
    }

    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut decoded_any = false;
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
        {
            output.push((high << 4) | low);
            decoded_any = true;
            index += 3;
            continue;
        }
        output.push(bytes[index]);
        index += 1;
    }

    decoded_any.then(|| String::from_utf8_lossy(&output).into_owned())
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn clean_exec_placeholders(command: &str) -> String {
    let mut cleaned = String::new();
    let mut chars = command.chars().peekable();

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

    cleaned.trim().to_string()
}

fn clean_exec_forwarding_tokens(command: &str) -> String {
    if !command.contains("@@") {
        return command.trim().to_string();
    }

    if let Some(words) = shell_words(command) {
        return clean_forwarding_words(words)
            .into_iter()
            .map(|word| shell_quote_word(&word))
            .collect::<Vec<_>>()
            .join(" ");
    }

    clean_forwarding_words(
        command
            .split_whitespace()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
    )
    .join(" ")
}

fn clean_forwarding_words(words: Vec<String>) -> Vec<String> {
    let strips_file_forwarding = words.iter().any(|word| is_forwarding_token(word));
    let mut forwarding_payload = false;
    words
        .into_iter()
        .filter(|word| {
            if is_forwarding_start(word) {
                forwarding_payload = true;
                return false;
            }
            if word == "@@" {
                forwarding_payload = false;
                return false;
            }
            if forwarding_payload {
                return false;
            }
            !(strips_file_forwarding && word == "--file-forwarding")
        })
        .collect()
}

fn is_forwarding_token(word: &str) -> bool {
    matches!(word, "@@" | "@@u" | "@@U" | "@@f" | "@@F")
}

fn is_forwarding_start(word: &str) -> bool {
    matches!(word, "@@u" | "@@U" | "@@f" | "@@F")
}

fn shell_quote_word(word: &str) -> String {
    if word.bytes().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-' | b'=')
    }) {
        return word.to_string();
    }
    format!("'{}'", word.replace('\'', "'\\''"))
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
    command.stdin(Stdio::null());
    if let Some(log) = app_launch_log() {
        let stdout = log.try_clone().ok().map(Stdio::from);
        command.stdout(stdout.unwrap_or_else(Stdio::null));
        command.stderr(Stdio::from(log));
    } else {
        command.stdout(Stdio::null()).stderr(Stdio::null());
    }
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
    command.env("XDG_CURRENT_DESKTOP", "Luft");
    command.env("XDG_SESSION_DESKTOP", "luft");
    command.env("DESKTOP_SESSION", "luft");
    command.env("XDG_SESSION_TYPE", "wayland");
    command.env("NO_AT_BRIDGE", "1");
    command.env("GTK_A11Y", "none");
    command.env("GTK_MODULES", "");
    command.env("UBUNTU_MENUPROXY", "0");
    command.env("GTK_OVERLAY_SCROLLING", "0");
    for (name, value) in cursor_environment_entries() {
        command.env(name, value);
    }
    command.env_remove("WAYLAND_DISPLAY");
    if let Some(display) = luft_wayland_display() {
        command.env("WAYLAND_DISPLAY", display);
    }
    if let Some(display) = xwayland_display {
        command.env("DISPLAY", display);
        command.env("_JAVA_AWT_WM_NONREPARENTING", "1");
    }
}

fn app_launch_log() -> Option<std::fs::File> {
    let path = ConfigPaths::discover().ok()?.log_file("luft-apps");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok()?;
    }
    OpenOptions::new().create(true).append(true).open(path).ok()
}

fn log_app_launch(command: &str) {
    use std::io::Write;

    if let Some(mut log) = app_launch_log() {
        let _ = writeln!(log, "\n--- luft launch: {command}");
    }
}

fn default_panel_app(
    label: &str,
    command: &str,
    fallback_icons: &[&str],
    applications: &[AppEntry],
) -> PanelApp {
    let matched = applications
        .iter()
        .find(|app| commands_match(&app.command, command));
    let icon_path = matched
        .and_then(|app| app.icon_path.clone())
        .or_else(|| resolve_first_icon_path(fallback_icons));
    let label = matched
        .map(|app| app.name.clone())
        .unwrap_or_else(|| label.to_string());

    PanelApp::new(label, normalize_launch_command(command), icon_path)
}

fn resolve_first_icon_path(icons: &[&str]) -> Option<PathBuf> {
    icons.iter().find_map(|icon| {
        let command = xdg::command_name(icon).unwrap_or(icon);
        resolve_icon_path(Some(command))
    })
}

fn commands_match(left: &str, right: &str) -> bool {
    let left = normalize_launch_command(left);
    let right = normalize_launch_command(right);
    xdg::command_name(&left).is_some_and(|left| xdg::command_name(&right) == Some(left))
}

fn luft_wayland_display() -> Option<std::ffi::OsString> {
    env::var_os("LUFT_WAYLAND_DISPLAY").or_else(|| env::var_os("WAYLAND_DISPLAY"))
}

#[cfg(test)]
mod tests {
    use super::normalize_launch_command;

    #[test]
    fn launch_command_decodes_percent_encoded_paths() {
        assert_eq!(
            normalize_launch_command(
                "env DESKTOPINTEGRATION=1 %2Fhome%2Fkristof%2FAppImages%2Fcodex_desktop.appimage",
            ),
            "env DESKTOPINTEGRATION=1 /home/kristof/AppImages/codex_desktop.appimage",
        );
    }

    #[test]
    fn launch_command_leaves_desktop_exec_placeholders_to_clean_exec() {
        assert_eq!(normalize_launch_command("ghostty %u"), "ghostty");
    }

    #[test]
    fn launch_command_removes_flatpak_file_forwarding_tokens() {
        assert_eq!(
            normalize_launch_command(
                "/usr/bin/flatpak run --branch=stable --file-forwarding app.id @@u %U @@",
            ),
            "/usr/bin/flatpak run --branch=stable app.id",
        );
        assert_eq!(
            normalize_launch_command("/usr/bin/flatpak run app.id @@u /tmp/file @@"),
            "/usr/bin/flatpak run app.id",
        );
    }
}
