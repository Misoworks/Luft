use std::path::{Path, PathBuf};

pub fn settings_command(command: &str, page: &str) -> String {
    let command = command.trim();
    if command.is_empty() {
        return String::new();
    }

    let command = if is_legacy_settings_command(command) {
        format!("asher-settings --page {page}")
    } else if command.contains("{page}") {
        command.replace("{page}", page)
    } else {
        format!("{command} --page {page}")
    };

    resolve_asher_settings_binary(
        &normalize_dbus_run_session(&command),
        current_exe().as_deref(),
    )
}

fn current_exe() -> Option<PathBuf> {
    std::env::current_exe().ok()
}

fn is_legacy_settings_command(command: &str) -> bool {
    command
        .split_whitespace()
        .next()
        .is_some_and(|binary| binary.ends_with("gnome-control-center"))
}

fn resolve_asher_settings_binary(command: &str, current_exe: Option<&Path>) -> String {
    let tokens = command.split_whitespace().collect::<Vec<_>>();
    let Some(index) = settings_program_index(&tokens) else {
        return command.to_string();
    };
    if tokens.get(index) != Some(&"asher-settings") {
        return command.to_string();
    }
    let mut resolved = tokens;
    let replacement = settings_command_replacement(current_exe);
    let replacement = replacement.as_deref().unwrap_or("asher-settings");
    resolved[index] = replacement;
    resolved.join(" ")
}

fn settings_program_index(tokens: &[&str]) -> Option<usize> {
    let binary = tokens.first()?;
    if !binary.ends_with("dbus-run-session") {
        return Some(0);
    }
    if let Some(separator) = tokens.iter().position(|token| *token == "--") {
        return (separator + 1 < tokens.len()).then_some(separator + 1);
    }
    let index = dbus_run_session_program_index(tokens);
    (index < tokens.len()).then_some(index)
}

fn sibling_settings_binary(current_exe: Option<&Path>) -> Option<PathBuf> {
    let path = current_exe?.parent()?.join("asher-settings");
    path.is_file().then_some(path)
}

fn settings_command_replacement(current_exe: Option<&Path>) -> Option<String> {
    if let Some(binary) = sibling_settings_binary(current_exe) {
        return Some(shell_quote(&binary.to_string_lossy()));
    }
    let manifest = dev_workspace_manifest(current_exe?)?;
    Some(format!(
        "cargo run --manifest-path {} -p asher-settings --",
        shell_quote(&manifest.to_string_lossy())
    ))
}

fn dev_workspace_manifest(current_exe: &Path) -> Option<PathBuf> {
    let target = current_exe
        .ancestors()
        .find(|path| path.file_name() == Some("target".as_ref()))?;
    let manifest = target.parent()?.join("Cargo.toml");
    manifest.is_file().then_some(manifest)
}

fn shell_quote(value: &str) -> String {
    if value.bytes().all(
        |byte| matches!(byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'/' | b'.' | b'_' | b'-'),
    ) {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn normalize_dbus_run_session(command: &str) -> String {
    let tokens = command.split_whitespace().collect::<Vec<_>>();
    let Some(binary) = tokens.first() else {
        return command.to_string();
    };
    if !binary.ends_with("dbus-run-session") || tokens.contains(&"--") {
        return command.to_string();
    }

    let program_index = dbus_run_session_program_index(&tokens);
    if program_index >= tokens.len() {
        return command.to_string();
    }

    let mut normalized = Vec::with_capacity(tokens.len() + 1);
    normalized.extend_from_slice(&tokens[..program_index]);
    normalized.push("--");
    normalized.extend_from_slice(&tokens[program_index..]);
    normalized.join(" ")
}

fn dbus_run_session_program_index(tokens: &[&str]) -> usize {
    let mut index = 1;
    while index < tokens.len() {
        match tokens[index] {
            "--config-file" => index += 2,
            "--help" | "--version" => return tokens.len(),
            token if token.starts_with("--config-file=") => index += 1,
            token if token.starts_with('-') => index += 1,
            _ => return index,
        }
    }
    tokens.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_page_to_plain_settings_command() {
        assert_eq!(
            settings_command("asher-settings", "sound"),
            format!("{} --page sound", expected_settings_command())
        );
    }

    #[test]
    fn replaces_gnome_control_center_with_asher_settings() {
        assert_eq!(
            settings_command("gnome-control-center", "sound"),
            format!("{} --page sound", expected_settings_command())
        );
    }

    #[test]
    fn separates_dbus_run_session_options_from_settings_page() {
        assert_eq!(
            settings_command("dbus-run-session asher-settings", "sound"),
            format!(
                "dbus-run-session -- {} --page sound",
                expected_settings_command()
            )
        );
    }

    #[test]
    fn keeps_existing_dbus_run_session_separator() {
        assert_eq!(
            settings_command("dbus-run-session -- asher-settings", "power"),
            format!(
                "dbus-run-session -- {} --page power",
                expected_settings_command()
            )
        );
    }

    #[test]
    fn supports_dbus_run_session_config_file_option() {
        assert_eq!(
            settings_command(
                "dbus-run-session --config-file /tmp/bus.conf asher-settings",
                "display"
            ),
            format!(
                "dbus-run-session --config-file /tmp/bus.conf -- {} --page display",
                expected_settings_command()
            )
        );
    }

    #[test]
    fn resolves_settings_binary_next_to_shell() {
        let temp = std::env::temp_dir().join(format!("asher-settings-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp);
        std::fs::create_dir_all(&temp).unwrap();
        let settings = temp.join("asher-settings");
        std::fs::write(&settings, "").unwrap();
        let shell = temp.join("asher-shell");

        assert_eq!(
            resolve_asher_settings_binary("asher-settings --page sound", Some(&shell)),
            format!("{} --page sound", settings.display())
        );

        let _ = std::fs::remove_dir_all(temp);
    }

    #[test]
    fn resolves_settings_binary_inside_dbus_run_session() {
        let temp =
            std::env::temp_dir().join(format!("asher-settings-dbus-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp);
        std::fs::create_dir_all(&temp).unwrap();
        let settings = temp.join("asher-settings");
        std::fs::write(&settings, "").unwrap();
        let shell = temp.join("asher-shell");

        assert_eq!(
            resolve_asher_settings_binary(
                "dbus-run-session -- asher-settings --page power",
                Some(&shell)
            ),
            format!("dbus-run-session -- {} --page power", settings.display())
        );

        let _ = std::fs::remove_dir_all(temp);
    }

    #[test]
    fn falls_back_to_cargo_run_from_dev_target() {
        let temp =
            std::env::temp_dir().join(format!("asher-settings-cargo-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp);
        std::fs::create_dir_all(temp.join("target/debug")).unwrap();
        std::fs::write(temp.join("Cargo.toml"), "[workspace]\n").unwrap();
        let shell = temp.join("target/debug/asher-shell");

        assert_eq!(
            resolve_asher_settings_binary("asher-settings --page sound", Some(&shell)),
            format!(
                "cargo run --manifest-path {} -p asher-settings -- --page sound",
                temp.join("Cargo.toml").display()
            )
        );

        let _ = std::fs::remove_dir_all(temp);
    }

    fn expected_settings_command() -> String {
        settings_command_replacement(current_exe().as_deref())
            .unwrap_or_else(|| "asher-settings".to_string())
    }
}
