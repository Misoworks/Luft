use serde::Serialize;
use staccato_config::{ConfigPaths, ConfigSource, load_config};
use staccato_ipc::{IpcRequest, IpcResponse, ShellStatus, XwaylandStatus, send_request};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

pub(crate) fn doctor_checks() -> Vec<DoctorCheck> {
    let mut checks = Vec::new();
    match ConfigPaths::discover() {
        Ok(paths) => {
            checks.push(check_config());
            checks.push(check_logs_dir(&paths));
            checks.push(check_binary("baton"));
            checks.push(check_binary("staccato-shell"));
            checks.push(check_session_file());
            checks.push(check_ipc());
            checks.push(check_xwayland());
            checks.extend(check_session_services());
            checks.push(check_session_backend_dependencies());
            checks.push(DoctorCheck::ok(
                "session-backend",
                "DRM/KMS backend has initial single-output GBM/GLES modeset rendering",
            ));
        }
        Err(error) => checks.push(DoctorCheck::fail("xdg-paths", error.to_string())),
    }
    checks
}

fn check_config() -> DoctorCheck {
    match load_config() {
        Ok(loaded) => {
            let source = match loaded.source {
                ConfigSource::User(path) => path.display().to_string(),
                ConfigSource::Defaults => "built-in defaults".to_string(),
            };
            DoctorCheck::ok("config", format!("valid ({source})"))
        }
        Err(error) => DoctorCheck::fail("config", error.to_string()),
    }
}

fn check_logs_dir(paths: &ConfigPaths) -> DoctorCheck {
    match fs::create_dir_all(paths.logs_dir()) {
        Ok(()) => DoctorCheck::ok("logs", paths.logs_dir().display().to_string()),
        Err(error) => DoctorCheck::fail("logs", format!("{}: {error}", paths.logs_dir().display())),
    }
}

fn check_binary(name: &str) -> DoctorCheck {
    if sibling_binary(name).is_some() || binary_in_path(name).is_some() {
        DoctorCheck::ok(format!("binary:{name}"), "found")
    } else {
        DoctorCheck::fail(
            format!("binary:{name}"),
            "not found beside staccatoctl or in PATH",
        )
    }
}

fn check_session_file() -> DoctorCheck {
    let installed = Path::new("/usr/share/wayland-sessions/staccato.desktop");
    if installed.exists() || Path::new("data/sessions/staccato.desktop").exists() {
        DoctorCheck::ok("session-file", "found")
    } else {
        DoctorCheck::warning(
            "session-file",
            "not installed in /usr/share/wayland-sessions",
        )
    }
}

fn check_ipc() -> DoctorCheck {
    match send_request(&IpcRequest::Status) {
        Ok(IpcResponse::Status(status)) => {
            let message = format!(
                "Baton reachable, workspace {}, shell {:?}",
                status.active_workspace.0, status.shell
            );
            match status.shell {
                ShellStatus::Running => DoctorCheck::ok("ipc", message),
                ShellStatus::Restarting => DoctorCheck::warning("ipc", message),
                ShellStatus::NotStarted | ShellStatus::Failed => {
                    DoctorCheck::warning("ipc", message)
                }
            }
        }
        Ok(response) => DoctorCheck::fail("ipc", format!("unexpected response: {response:?}")),
        Err(error) => DoctorCheck::warning("ipc", format!("Baton is not reachable: {error}")),
    }
}

fn check_xwayland() -> DoctorCheck {
    let enabled = load_config().is_ok_and(|loaded| loaded.config.compositor.xwayland);
    if !enabled {
        return DoctorCheck::ok("xwayland", "disabled in config");
    }

    if let Ok(IpcResponse::Status(status)) = send_request(&IpcRequest::Status) {
        return match status.xwayland {
            XwaylandStatus::Running => DoctorCheck::ok(
                "xwayland",
                format!(
                    "xwayland-satellite running on {}",
                    status
                        .xwayland_display
                        .unwrap_or_else(|| "unknown display".to_string())
                ),
            ),
            XwaylandStatus::Restarting => {
                DoctorCheck::warning("xwayland", "xwayland-satellite is restarting")
            }
            XwaylandStatus::Disabled => DoctorCheck::warning("xwayland", "disabled at runtime"),
            XwaylandStatus::Unavailable => {
                DoctorCheck::warning("xwayland", "xwayland-satellite is unavailable")
            }
            XwaylandStatus::Failed => {
                DoctorCheck::warning("xwayland", "xwayland-satellite failed to start")
            }
        };
    }

    if binary_in_path("xwayland-satellite").is_some() {
        DoctorCheck::ok("xwayland", "xwayland-satellite found")
    } else {
        DoctorCheck::warning("xwayland", "install xwayland-satellite for X11 app support")
    }
}

fn check_session_backend_dependencies() -> DoctorCheck {
    if binary_in_path("pkg-config").is_none() {
        return DoctorCheck::warning(
            "session-deps",
            "pkg-config is needed to build Baton with --features session-backend",
        );
    }

    match std::process::Command::new("pkg-config")
        .arg("--exists")
        .arg("libseat")
        .status()
    {
        Ok(status) if status.success() => DoctorCheck::ok("session-deps", "libseat found"),
        Ok(_) => DoctorCheck::warning(
            "session-deps",
            "libseat development files are missing; install seatd/libseat to build the DRM/KMS session backend",
        ),
        Err(error) => DoctorCheck::warning("session-deps", error.to_string()),
    }
}

fn check_session_services() -> Vec<DoctorCheck> {
    let mut checks = Vec::new();
    if binary_in_path("dbus-run-session").is_some() {
        checks.push(DoctorCheck::ok("dbus-session", "dbus-run-session found"));
    } else {
        checks.push(DoctorCheck::warning(
            "dbus-session",
            "install dbus-run-session for a private Staccato session bus",
        ));
    }

    if binary_in_path("dbus-update-activation-environment").is_some() {
        checks.push(DoctorCheck::ok(
            "dbus-activation",
            "dbus-update-activation-environment found",
        ));
    } else {
        checks.push(DoctorCheck::warning(
            "dbus-activation",
            "install dbus-update-activation-environment so activated services inherit the session display",
        ));
    }

    if binary_in_path("gnome-keyring-daemon").is_some() {
        checks.push(DoctorCheck::ok(
            "secret-service",
            "gnome-keyring-daemon found",
        ));
    } else {
        checks.push(DoctorCheck::warning(
            "secret-service",
            "install gnome-keyring-daemon or another Secret Service provider to avoid org.freedesktop.secrets timeouts",
        ));
    }
    checks
}

fn sibling_binary(name: &str) -> Option<PathBuf> {
    let mut path = env::current_exe().ok()?;
    path.set_file_name(name);
    path.exists().then_some(path)
}

fn binary_in_path(name: &str) -> Option<PathBuf> {
    env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| env::split_paths(&paths).collect::<Vec<_>>())
        .map(|dir| dir.join(name))
        .find(|path| path.exists())
}

#[derive(Debug, Serialize)]
pub(crate) struct DoctorCheck {
    pub(crate) name: String,
    pub(crate) severity: Severity,
    pub(crate) message: String,
}

impl DoctorCheck {
    fn ok(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            severity: Severity::Ok,
            message: message.into(),
        }
    }

    fn warning(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            severity: Severity::Warning,
            message: message.into(),
        }
    }

    fn fail(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            severity: Severity::Fail,
            message: message.into(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Severity {
    Ok,
    Warning,
    Fail,
}

impl Severity {
    pub(crate) const fn label(&self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warning => "warn",
            Self::Fail => "fail",
        }
    }
}
