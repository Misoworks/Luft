use std::path::Path;

const WAYLAND_FLAG: &str = "--ozone-platform=wayland";
const DISABLE_VULKAN_FLAG: &str = "--disable-vulkan";
const DISABLE_ACCESSIBILITY_FLAG: &str = "--disable-renderer-accessibility";
const DISABLE_VULKAN_FEATURES: &[&str] = &["Vulkan", "DefaultANGLEVulkan", "VulkanFromANGLE"];

pub fn command_for_shell(command: &str) -> String {
    let command = command.trim();
    let Some(name) = command_name(command) else {
        return command.to_string();
    };
    if !is_chromium_command(name) {
        return command.to_string();
    }

    let mut prepared = command.to_string();
    append_flag(
        &mut prepared,
        command,
        "--user-data-dir",
        &profile_flag(name),
    );
    append_flag(&mut prepared, command, "--no-first-run", "--no-first-run");
    append_flag(&mut prepared, command, "--ozone-platform", WAYLAND_FLAG);
    append_flag(
        &mut prepared,
        command,
        "--disable-vulkan",
        DISABLE_VULKAN_FLAG,
    );
    append_disable_features(&mut prepared, command);
    append_flag(
        &mut prepared,
        command,
        "--disable-renderer-accessibility",
        DISABLE_ACCESSIBILITY_FLAG,
    );
    append_flag(&mut prepared, command, "--new-window", "--new-window");
    prepared
}

fn append_flag(prepared: &mut String, original: &str, key: &str, flag: &str) {
    if original
        .split_whitespace()
        .any(|part| part.starts_with(key))
    {
        return;
    }

    prepared.push(' ');
    prepared.push_str(flag);
}

fn append_disable_features(prepared: &mut String, original: &str) {
    let Some(existing) = original
        .split_whitespace()
        .find(|part| part.starts_with("--disable-features="))
    else {
        prepared.push(' ');
        prepared.push_str(&disable_features_flag(DISABLE_VULKAN_FEATURES));
        return;
    };

    let mut features = existing
        .trim_start_matches("--disable-features=")
        .split(',')
        .filter(|feature| !feature.is_empty())
        .collect::<Vec<_>>();
    for &feature in DISABLE_VULKAN_FEATURES {
        if !features.contains(&feature) {
            features.push(feature);
        }
    }
    let replacement = disable_features_flag(&features);
    if let Some(index) = prepared.find(existing) {
        prepared.replace_range(index..index + existing.len(), &replacement);
    }
}

fn disable_features_flag(features: &[&str]) -> String {
    format!("--disable-features={}", features.join(","))
}

fn is_chromium_command(command: &str) -> bool {
    matches!(
        command,
        "google-chrome-stable"
            | "google-chrome"
            | "chromium"
            | "chromium-browser"
            | "brave-browser"
            | "microsoft-edge"
            | "microsoft-edge-stable"
    )
}

fn profile_flag(command: &str) -> String {
    format!(
        "--user-data-dir=\"${{XDG_STATE_HOME:-$HOME/.local/state}}/staccato/{}\"",
        profile_dir(command)
    )
}

fn profile_dir(command: &str) -> &'static str {
    match command {
        "chromium" | "chromium-browser" => "chromium",
        "brave-browser" => "brave",
        "microsoft-edge" | "microsoft-edge-stable" => "edge",
        _ => "chrome",
    }
}

fn command_name(command: &str) -> Option<&str> {
    let first = command
        .split_whitespace()
        .next()?
        .trim_matches('"')
        .trim_matches('\'');
    Path::new(first).file_name()?.to_str()
}

#[cfg(test)]
mod tests {
    use super::command_for_shell;

    #[test]
    fn isolates_chrome_to_staccato_profile() {
        let command = command_for_shell("google-chrome-stable");

        assert!(command.contains("--user-data-dir="));
        assert!(command.contains("/staccato/chrome"));
        assert!(command.contains("--ozone-platform=wayland"));
        assert!(command.contains("--disable-vulkan"));
        assert!(command.contains("--disable-features=Vulkan,DefaultANGLEVulkan,VulkanFromANGLE"));
        assert!(command.contains("--disable-renderer-accessibility"));
        assert!(command.contains("--new-window"));
    }

    #[test]
    fn keeps_existing_chrome_profile_flag() {
        let command = command_for_shell("google-chrome-stable --user-data-dir=/tmp/profile");

        assert!(command.contains("--user-data-dir=/tmp/profile"));
        assert_eq!(command.matches("--user-data-dir").count(), 1);
    }

    #[test]
    fn merges_existing_chrome_disable_features_flag() {
        let command = command_for_shell("google-chrome-stable --disable-features=Foo");

        assert!(
            command.contains("--disable-features=Foo,Vulkan,DefaultANGLEVulkan,VulkanFromANGLE")
        );
        assert_eq!(command.matches("--disable-features").count(), 1);
    }

    #[test]
    fn leaves_non_browser_commands_unchanged() {
        assert_eq!(command_for_shell("nautilus"), "nautilus");
    }

    #[test]
    fn keeps_browser_profiles_separate() {
        let command = command_for_shell("brave-browser");

        assert!(command.contains("/staccato/brave"));
    }
}
