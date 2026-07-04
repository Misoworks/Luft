use luft_config::cursor_environment_entries;
use std::{
    env,
    path::PathBuf,
    process::{Command, Stdio},
};
use tracing::{debug, info, warn};

const PRIVATE_DBUS_ENV: &str = "LUFT_PRIVATE_DBUS";
const DISABLE_SERVICES_ENV: &str = "LUFT_NO_SESSION_SERVICES";

pub fn start(wayland_display: &str, x11_display: Option<&str>) {
    if !owns_private_session_bus() {
        return;
    }

    sync_activation_environment(wayland_display, x11_display);

    if env::var_os(DISABLE_SERVICES_ENV).is_some() {
        debug!("session service startup disabled");
        return;
    }

    start_luft_portal();
    start_portal_broker();
}

pub fn sync_activation_environment(wayland_display: &str, x11_display: Option<&str>) {
    if !owns_private_session_bus() {
        return;
    }

    let Some(binary) = find_in_path("dbus-update-activation-environment") else {
        debug!("dbus-update-activation-environment is not installed");
        return;
    };

    let mut entries = vec![
        format!("WAYLAND_DISPLAY={wayland_display}"),
        "XDG_CURRENT_DESKTOP=Luft".to_string(),
        "XDG_SESSION_DESKTOP=luft".to_string(),
        "DESKTOP_SESSION=luft".to_string(),
        "XDG_SESSION_TYPE=wayland".to_string(),
    ];
    entries.extend(cursor_environment_entries().map(|(name, value)| format!("{name}={value}")));
    if let Some(display) = x11_display {
        entries.push(format!("DISPLAY={display}"));
    }
    if let Some(address) = env::var_os("DBUS_SESSION_BUS_ADDRESS") {
        entries.push(format!(
            "DBUS_SESSION_BUS_ADDRESS={}",
            address.to_string_lossy()
        ));
    }
    if let Some(runtime_dir) = env::var_os("XDG_RUNTIME_DIR") {
        entries.push(format!("XDG_RUNTIME_DIR={}", runtime_dir.to_string_lossy()));
    }

    if update_activation_environment(&binary, &entries, true)
        || update_activation_environment(&binary, &entries, false)
    {
        debug!("updated D-Bus activation environment");
    }
}

fn start_luft_portal() {
    let Some(binary) = luft_portal_binary() else {
        warn!("luft-portal is not installed; portal Settings will be unavailable");
        return;
    };

    let mut command = Command::new(binary);
    spawn_session_helper("luft-portal", &mut command);
}

fn start_portal_broker() {
    let Some(binary) =
        find_known_program("xdg-desktop-portal", &["/usr/libexec/xdg-desktop-portal"])
    else {
        debug!("xdg-desktop-portal is not installed");
        return;
    };

    let mut command = Command::new(binary);
    spawn_session_helper("xdg-desktop-portal", &mut command);
}

fn luft_portal_binary() -> Option<PathBuf> {
    if let Some(path) = env::var_os("LUFT_PORTAL") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }

    find_in_path("luft-portal").or_else(sibling_binary)
}

fn sibling_binary() -> Option<PathBuf> {
    let mut path = env::current_exe().ok()?;
    path.set_file_name("luft-portal");
    path.is_file().then_some(path)
}

fn spawn_session_helper(label: &'static str, command: &mut Command) {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    match command.spawn() {
        Ok(mut child) => {
            info!(pid = child.id(), label, "started session helper");
            std::thread::spawn(move || match child.wait() {
                Ok(status) if status.success() => {
                    debug!(%status, label, "session helper exited");
                }
                Ok(status) => {
                    warn!(%status, label, "session helper exited unsuccessfully");
                }
                Err(error) => {
                    warn!(%error, label, "failed to wait for session helper");
                }
            });
        }
        Err(error) => {
            warn!(%error, label, "failed to start session helper");
        }
    }
}

fn update_activation_environment(binary: &PathBuf, entries: &[String], systemd: bool) -> bool {
    let mut command = Command::new(binary);
    if systemd {
        command.arg("--systemd");
    }
    command
        .args(entries)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    match command.status() {
        Ok(status) if status.success() => true,
        Ok(status) => {
            debug!(%status, systemd, "failed to update D-Bus activation environment");
            false
        }
        Err(error) => {
            debug!(%error, systemd, "failed to run dbus-update-activation-environment");
            false
        }
    }
}

fn owns_private_session_bus() -> bool {
    env::var_os(PRIVATE_DBUS_ENV).is_some() && env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some()
}

fn find_in_path(program: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file())
}

fn find_known_program(program: &str, paths: &[&str]) -> Option<PathBuf> {
    find_in_path(program).or_else(|| {
        paths
            .iter()
            .map(PathBuf::from)
            .find(|candidate| candidate.is_file())
    })
}
