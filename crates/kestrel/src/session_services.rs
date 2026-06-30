use asher_config::cursor_environment_entries;
use std::{
    env,
    path::PathBuf,
    process::{Command, Stdio},
};
use tracing::{debug, info, warn};

const PRIVATE_DBUS_ENV: &str = "ASHER_PRIVATE_DBUS";
const DISABLE_SERVICES_ENV: &str = "ASHER_NO_SESSION_SERVICES";

pub fn start(wayland_display: &str, x11_display: Option<&str>) {
    if !owns_private_session_bus() {
        return;
    }

    sync_activation_environment(wayland_display, x11_display);

    if env::var_os(DISABLE_SERVICES_ENV).is_some() {
        debug!("session service startup disabled");
        return;
    }

    start_portal_broker();
    start_secret_service();
    start_policykit_agent();
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
        "XDG_CURRENT_DESKTOP=Asher".to_string(),
        "XDG_SESSION_DESKTOP=asher".to_string(),
        "DESKTOP_SESSION=asher".to_string(),
        "XDG_SESSION_TYPE=wayland".to_string(),
    ];
    entries.extend(cursor_environment_entries().map(|(name, value)| format!("{name}={value}")));
    if let Some(display) = x11_display {
        entries.push(format!("DISPLAY={display}"));
    }

    if update_activation_environment(&binary, &entries, true)
        || update_activation_environment(&binary, &entries, false)
    {
        debug!("updated D-Bus activation environment");
    }
}

fn start_secret_service() {
    let Some(binary) =
        find_known_program("gnome-keyring-daemon", &["/usr/bin/gnome-keyring-daemon"])
    else {
        debug!("gnome-keyring-daemon is not installed; Secret Service is unavailable");
        return;
    };

    match Command::new(&binary)
        .arg("--start")
        .arg("--components=secrets")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(status) if status.success() => {
            info!("started Secret Service provider");
        }
        Ok(status) => {
            warn!(%status, path = %binary.display(), "Secret Service provider exited unsuccessfully");
        }
        Err(error) => {
            warn!(%error, path = %binary.display(), "failed to start Secret Service provider");
        }
    }
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

fn start_policykit_agent() {
    let Some(binary) = policykit_agent() else {
        debug!("no PolicyKit authentication agent is installed");
        return;
    };

    let mut command = Command::new(binary);
    spawn_session_helper("PolicyKit authentication agent", &mut command);
}

fn policykit_agent() -> Option<PathBuf> {
    [
        "/usr/libexec/polkit-gnome-authentication-agent-1",
        "/usr/lib/polkit-gnome/polkit-gnome-authentication-agent-1",
        "/usr/libexec/polkit-kde-authentication-agent-1",
        "/usr/lib64/libexec/polkit-kde-authentication-agent-1",
        "/usr/bin/lxpolkit",
        "/usr/libexec/lxqt-policykit-agent",
        "/usr/bin/lxqt-policykit-agent",
        "/usr/lib/mate-polkit/polkit-mate-authentication-agent-1",
        "/usr/libexec/xfce-polkit",
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|path| path.is_file())
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
