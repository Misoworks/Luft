use staccato_config::{PinnedAppConfig, load_config, save_config};
use staccato_ipc::{IpcRequest, send_request};
use std::path::Path;

pub fn list_dock_pins(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let loaded = load_config()?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&loaded.config.dock.pinned)?
        );
    } else if loaded.config.dock.pinned.is_empty() && loaded.config.dock.customized {
        println!("Dock is customized with no pinned apps");
    } else if loaded.config.dock.pinned.is_empty() {
        println!("No custom dock pins configured; built-in defaults are active");
    } else {
        for app in loaded.config.dock.pinned {
            let icon = app.icon.unwrap_or_else(|| "-".to_string());
            println!("{}\t{}\t{}", app.label, app.command, icon);
        }
    }
    Ok(())
}

pub fn pin_dock_app(
    command: String,
    label: Option<String>,
    icon: Option<String>,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut loaded = load_config()?.config;
    let label = label.unwrap_or_else(|| label_from_command(&command));
    let pin = PinnedAppConfig {
        label,
        command: command.trim().to_string(),
        icon: icon.filter(|icon| !icon.trim().is_empty()),
    };
    if pin.command.is_empty() {
        return Err("dock pin command cannot be empty".into());
    }
    loaded.dock.customized = true;

    match loaded
        .dock
        .pinned
        .iter()
        .position(|app| commands_equal(&app.command, &pin.command))
    {
        Some(index) => loaded.dock.pinned[index] = pin,
        None => loaded.dock.pinned.push(pin),
    }

    let path = save_config(&loaded)?;
    reload_if_live();
    print_config_write(json, &path)
}

pub fn unpin_dock_app(app: String, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut loaded = load_config()?.config;
    let before = loaded.dock.pinned.len();
    loaded.dock.pinned.retain(|pin| {
        !commands_equal(&pin.command, &app) && !pin.label.eq_ignore_ascii_case(app.trim())
    });
    if loaded.dock.pinned.len() == before {
        return Err(format!("no dock pin matched {app:?}").into());
    }

    loaded.dock.customized = true;
    let path = save_config(&loaded)?;
    reload_if_live();
    print_config_write(json, &path)
}

fn reload_if_live() {
    let _ = send_request(&IpcRequest::Reload);
}

fn print_config_write(json: bool, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if json {
        println!(
            "{}",
            serde_json::json!({
                "saved": true,
                "config_file": path,
            })
        );
    } else {
        println!("Saved {}", path.display());
    }
    Ok(())
}

fn label_from_command(command: &str) -> String {
    let first = command
        .split_whitespace()
        .next()
        .unwrap_or(command)
        .trim_matches('"')
        .trim_matches('\'');
    Path::new(first)
        .file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("Pinned app")
        .to_string()
}

fn commands_equal(left: &str, right: &str) -> bool {
    left.trim() == right.trim()
}
