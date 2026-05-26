use staccato_ipc::XwaylandStatus;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
use tracing::{info, warn};

const RESTART_DELAY: Duration = Duration::from_secs(2);

#[derive(Debug)]
pub struct XwaylandSatellite {
    enabled: bool,
    child: Option<Child>,
    binary: Option<PathBuf>,
    display: Option<String>,
    next_spawn_after: Option<Instant>,
    failed: bool,
    unavailable_warned: bool,
}

impl XwaylandSatellite {
    pub fn start(enabled: bool, wayland_display: &str) -> Self {
        let mut satellite = Self {
            enabled,
            child: None,
            binary: find_in_path("xwayland-satellite"),
            display: None,
            next_spawn_after: None,
            failed: false,
            unavailable_warned: false,
        };
        satellite.spawn(wayland_display);
        satellite
    }

    pub fn reap(&mut self, wayland_display: &str) {
        let Some(child) = &mut self.child else {
            self.spawn_if_due(wayland_display);
            return;
        };

        match child.try_wait() {
            Ok(Some(status)) => {
                warn!(%status, "xwayland-satellite exited");
                self.child = None;
                self.failed = true;
                self.next_spawn_after = Some(Instant::now() + RESTART_DELAY);
            }
            Ok(None) => {}
            Err(error) => {
                warn!(%error, "failed to inspect xwayland-satellite process");
            }
        }
    }

    pub fn status(&self) -> XwaylandStatus {
        if !self.enabled {
            XwaylandStatus::Disabled
        } else if self.child.is_some() {
            XwaylandStatus::Running
        } else if self.next_spawn_after.is_some() {
            XwaylandStatus::Restarting
        } else if self.binary.is_none() {
            XwaylandStatus::Unavailable
        } else if self.failed {
            XwaylandStatus::Failed
        } else {
            XwaylandStatus::Unavailable
        }
    }

    pub fn display(&self) -> Option<&str> {
        self.display.as_deref().filter(|_| self.child.is_some())
    }

    fn spawn_if_due(&mut self, wayland_display: &str) {
        if self
            .next_spawn_after
            .is_some_and(|deadline| Instant::now() < deadline)
        {
            return;
        }
        self.spawn(wayland_display);
    }

    fn spawn(&mut self, wayland_display: &str) {
        if !self.enabled {
            return;
        }
        let Some(binary) = &self.binary else {
            if !self.unavailable_warned {
                warn!("xwayland-satellite is not installed; X11 app support is unavailable");
                self.unavailable_warned = true;
            }
            return;
        };
        let Some(x11_display) = self
            .display
            .clone()
            .or_else(|| available_display().map(display_name))
        else {
            warn!("no free X11 display number found for xwayland-satellite");
            self.failed = true;
            return;
        };

        self.next_spawn_after = None;
        let child = Command::new(binary)
            .arg(&x11_display)
            .env_remove("DISPLAY")
            .env("WAYLAND_DISPLAY", wayland_display)
            .env("XDG_CURRENT_DESKTOP", "Staccato")
            .env("XDG_SESSION_DESKTOP", "staccato")
            .env("DESKTOP_SESSION", "staccato")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();

        match child {
            Ok(child) => {
                info!(
                    pid = child.id(),
                    x11_display = x11_display.as_str(),
                    "started xwayland-satellite"
                );
                self.display = Some(x11_display);
                self.child = Some(child);
                self.failed = false;
            }
            Err(error) => {
                warn!(%error, path = %binary.display(), "failed to start xwayland-satellite");
                self.failed = true;
                self.next_spawn_after = Some(Instant::now() + RESTART_DELAY);
            }
        }
    }
}

impl Drop for XwaylandSatellite {
    fn drop(&mut self) {
        let Some(child) = &mut self.child else {
            return;
        };

        if child.try_wait().ok().flatten().is_none() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn display_name(number: u32) -> String {
    format!(":{number}")
}

fn available_display() -> Option<u32> {
    (12..100).find(|number| !display_in_use(*number))
}

fn display_in_use(number: u32) -> bool {
    Path::new("/tmp/.X11-unix")
        .join(format!("X{number}"))
        .exists()
        || Path::new(&format!("/tmp/.X{number}-lock")).exists()
}

fn find_in_path(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file())
        .or_else(|| fs::canonicalize(program).ok())
}
