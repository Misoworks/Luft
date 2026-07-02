use asher_config::{AsherConfig, cursor_environment_entries};
use asher_ipc::{SHELL_SOCKET_ENV, SOCKET_ENV, ShellStatus};
use std::{
    env,
    path::{Path, PathBuf},
    process::{Child, Command},
    time::{Duration, Instant},
};
use tracing::{debug, warn};

const NORMAL_RESTART_DELAY: Duration = Duration::from_millis(500);
const PRIVATE_DBUS_ENV: &str = "ASHER_PRIVATE_DBUS";
const USE_HOST_DBUS_ENV: &str = "ASHER_USE_HOST_DBUS";
const OUTPUT_REFRESH_ENV: &str = "ASHER_OUTPUT_REFRESH_MILLIHERTZ";

#[derive(Debug)]
pub struct ShellProcess {
    child: Option<Child>,
    binary: Option<PathBuf>,
    wayland_display: String,
    x11_display: Option<String>,
    ipc_socket: PathBuf,
    shell_socket: PathBuf,
    output_refresh_millihertz: i32,
    next_spawn_after: Option<Instant>,
}

impl ShellProcess {
    pub fn start(
        wayland_display: &str,
        x11_display: Option<&str>,
        ipc_socket: &Path,
        shell_socket: &Path,
        output_refresh_millihertz: i32,
    ) -> Self {
        let binary = shell_binary();
        if binary.is_none() {
            warn!("asher-shell binary was not found beside kestrel");
        };
        remove_stale_shell_socket(shell_socket);

        let mut shell = Self {
            child: None,
            binary,
            wayland_display: wayland_display.to_string(),
            x11_display: x11_display.map(str::to_string),
            ipc_socket: ipc_socket.to_path_buf(),
            shell_socket: shell_socket.to_path_buf(),
            output_refresh_millihertz,
            next_spawn_after: None,
        };
        shell.spawn();
        shell
    }

    pub fn status(&self) -> ShellStatus {
        if self.child.is_some() {
            ShellStatus::Running
        } else if self.next_spawn_after.is_some() {
            ShellStatus::Restarting
        } else if self.binary.is_some() {
            ShellStatus::NotStarted
        } else {
            ShellStatus::Failed
        }
    }

    pub fn reap(&mut self, config: &mut AsherConfig) {
        let Some(child) = &mut self.child else {
            self.spawn_if_due();
            return;
        };

        match child.try_wait() {
            Ok(Some(status)) => {
                warn!(%status, "asher shell exited");
                self.child = None;
                let _ = config;
                self.next_spawn_after = Some(Instant::now() + NORMAL_RESTART_DELAY);
                self.spawn_if_due();
            }
            Ok(None) => {}
            Err(error) => {
                warn!(%error, "failed to inspect asher shell process");
            }
        }
    }

    pub fn restart(&mut self) {
        self.restart_now();
    }

    fn restart_now(&mut self) {
        if let Some(mut child) = self.child.take() {
            if child.try_wait().ok().flatten().is_none() {
                let _ = child.kill();
            }
            let _ = child.wait();
        }
        self.next_spawn_after = None;
        remove_stale_shell_socket(&self.shell_socket);
        self.spawn();
    }

    fn spawn_if_due(&mut self) {
        if self
            .next_spawn_after
            .is_some_and(|deadline| Instant::now() < deadline)
        {
            return;
        }
        self.spawn();
    }

    fn spawn(&mut self) {
        let Some(binary) = &self.binary else {
            return;
        };

        self.next_spawn_after = None;
        let mut command = shell_command(binary);
        command
            .env_remove("DISPLAY")
            .env("GDK_BACKEND", "wayland")
            .env("WAYLAND_DISPLAY", &self.wayland_display)
            .env("ASHER_WAYLAND_DISPLAY", &self.wayland_display)
            .env("NO_AT_BRIDGE", "1")
            .env("GTK_A11Y", "none")
            .env("GTK_MODULES", "")
            .env(PRIVATE_DBUS_ENV, "1")
            .env(
                OUTPUT_REFRESH_ENV,
                self.output_refresh_millihertz.max(1).to_string(),
            )
            .env(SOCKET_ENV, &self.ipc_socket)
            .env(SHELL_SOCKET_ENV, &self.shell_socket);
        for (name, value) in cursor_environment_entries() {
            command.env(name, value);
        }
        if let Some(display) = &self.x11_display {
            command.env("DISPLAY", display);
        }
        let child = command.spawn();

        match child {
            Ok(child) => {
                debug!(pid = child.id(), path = %binary.display(), "started asher shell");
                self.child = Some(child);
            }
            Err(error) => {
                warn!(%error, path = %binary.display(), "failed to start asher shell");
                self.child = None;
                self.next_spawn_after = Some(Instant::now() + NORMAL_RESTART_DELAY);
            }
        }
    }
}

impl Drop for ShellProcess {
    fn drop(&mut self) {
        let Some(child) = &mut self.child else {
            remove_stale_shell_socket(&self.shell_socket);
            return;
        };

        if child.try_wait().ok().flatten().is_none() {
            let _ = child.kill();
            let _ = child.wait();
        }
        remove_stale_shell_socket(&self.shell_socket);
    }
}

fn shell_binary() -> Option<std::path::PathBuf> {
    let mut path = std::env::current_exe().ok()?;
    path.set_file_name("asher-shell");
    path.exists().then_some(path)
}

fn shell_command(binary: &Path) -> Command {
    if should_wrap_shell_dbus()
        && let Some(dbus_run_session) = find_in_path("dbus-run-session")
    {
        let mut command = Command::new(dbus_run_session);
        command.arg("--").arg(binary);
        return command;
    }

    Command::new(binary)
}

fn should_wrap_shell_dbus() -> bool {
    env::var_os(USE_HOST_DBUS_ENV).is_none() && env::var_os(PRIVATE_DBUS_ENV).is_none()
}

fn find_in_path(program: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file())
}

fn remove_stale_shell_socket(path: &Path) {
    match std::fs::remove_file(path) {
        Ok(()) => debug!(path = %path.display(), "removed stale shell control socket"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            warn!(%error, path = %path.display(), "failed to remove stale shell control socket")
        }
    }
}
