use crate::recovery::RecoveryPolicy;
use staccato_config::{IGNORE_USER_CONFIG_ENV, StaccatoConfig};
use staccato_ipc::{SHELL_SOCKET_ENV, SOCKET_ENV, ShellStatus};
use std::{
    collections::VecDeque,
    env, io,
    path::{Path, PathBuf},
    process::{Child, Command},
    time::{Duration, Instant},
};
use tracing::{debug, info, warn};

const NORMAL_RESTART_DELAY: Duration = Duration::from_millis(500);
const SAFE_MODE_RESTART_DELAY: Duration = Duration::from_secs(2);
const PRIVATE_DBUS_ENV: &str = "STACCATO_PRIVATE_DBUS";
const USE_HOST_DBUS_ENV: &str = "STACCATO_USE_HOST_DBUS";

#[derive(Debug)]
pub struct ShellProcess {
    child: Option<Child>,
    binary: Option<PathBuf>,
    wayland_display: String,
    x11_display: Option<String>,
    ipc_socket: PathBuf,
    shell_socket: PathBuf,
    recovery: RecoveryPolicy,
    crashes: VecDeque<Instant>,
    next_spawn_after: Option<Instant>,
    default_config: bool,
}

impl ShellProcess {
    pub fn start(
        wayland_display: &str,
        x11_display: Option<&str>,
        ipc_socket: &Path,
        shell_socket: &Path,
        recovery: RecoveryPolicy,
    ) -> Self {
        if let Err(error) = ensure_dev_shell_built() {
            warn!(%error, "failed to build staccato-shell helper");
        }
        let binary = shell_binary();
        if binary.is_none() {
            warn!("staccato-shell binary was not found beside baton");
        };
        remove_stale_shell_socket(shell_socket);

        let mut shell = Self {
            child: None,
            binary,
            wayland_display: wayland_display.to_string(),
            x11_display: x11_display.map(str::to_string),
            ipc_socket: ipc_socket.to_path_buf(),
            shell_socket: shell_socket.to_path_buf(),
            recovery,
            crashes: VecDeque::new(),
            next_spawn_after: None,
            default_config: false,
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

    pub fn reap(&mut self, config: &mut StaccatoConfig) {
        let Some(child) = &mut self.child else {
            self.spawn_if_due();
            return;
        };

        match child.try_wait() {
            Ok(Some(status)) => {
                warn!(%status, "staccato shell exited");
                self.child = None;
                self.record_crash(config);
                self.spawn_if_due();
            }
            Ok(None) => {}
            Err(error) => {
                warn!(%error, "failed to inspect staccato shell process");
            }
        }
    }

    pub fn restart(&mut self) {
        self.default_config = false;
        self.restart_now();
    }

    pub fn restart_with_default_config(&mut self) {
        self.default_config = true;
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
            .env("STACCATO_WAYLAND_DISPLAY", &self.wayland_display)
            .env("NO_AT_BRIDGE", "1")
            .env("GTK_A11Y", "none")
            .env("GTK_MODULES", "")
            .env(PRIVATE_DBUS_ENV, "1")
            .env(SOCKET_ENV, &self.ipc_socket)
            .env(SHELL_SOCKET_ENV, &self.shell_socket);
        if self.default_config {
            command.env(IGNORE_USER_CONFIG_ENV, "1");
        } else {
            command.env_remove(IGNORE_USER_CONFIG_ENV);
        }
        if let Some(display) = &self.x11_display {
            command.env("DISPLAY", display);
        }
        let child = command.spawn();

        match child {
            Ok(child) => {
                debug!(pid = child.id(), path = %binary.display(), "started staccato shell");
                self.child = Some(child);
            }
            Err(error) => {
                warn!(%error, path = %binary.display(), "failed to start staccato shell");
                self.child = None;
                self.next_spawn_after = Some(Instant::now() + SAFE_MODE_RESTART_DELAY);
            }
        }
    }

    fn record_crash(&mut self, config: &mut StaccatoConfig) {
        let limit = self.recovery.limit().max(1);
        let window = self.recovery.window();
        let now = Instant::now();

        while self
            .crashes
            .front()
            .is_some_and(|crash| now.duration_since(*crash) > window)
        {
            self.crashes.pop_front();
        }
        self.crashes.push_back(now);

        if self.crashes.len() < limit {
            self.next_spawn_after = Some(now + NORMAL_RESTART_DELAY);
            return;
        }

        if !config.general.safe_mode {
            info!(
                crashes = self.crashes.len(),
                seconds = window.as_secs(),
                "staccato shell crash limit reached; restarting in safe mode"
            );
        }
        enter_safe_mode(config);
        self.crashes.clear();
        self.next_spawn_after = Some(now + SAFE_MODE_RESTART_DELAY);
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

fn enter_safe_mode(config: &mut StaccatoConfig) {
    config.general.safe_mode = true;
    config.general.enable_effects = false;
    config.general.enable_animations = false;
    config.general.enable_blur = false;
    config.effects.blur = false;
    config.compositor.debug_overlay = false;
}

fn shell_binary() -> Option<std::path::PathBuf> {
    let mut path = std::env::current_exe().ok()?;
    path.set_file_name("staccato-shell");
    path.exists().then_some(path)
}

fn ensure_dev_shell_built() -> io::Result<()> {
    let Some(workspace) = dev_workspace() else {
        return Ok(());
    };

    let mut command = Command::new("cargo");
    command
        .arg("build")
        .arg("--manifest-path")
        .arg(&workspace.manifest)
        .arg("--bin")
        .arg("staccato-shell");
    if workspace.release {
        command.arg("--release");
    }

    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "cargo failed to build staccato-shell: {status}"
        )))
    }
}

struct DevWorkspace {
    manifest: PathBuf,
    release: bool,
}

fn dev_workspace() -> Option<DevWorkspace> {
    let exe = env::current_exe().ok()?;
    let target = exe
        .ancestors()
        .find(|path| path.file_name() == Some("target".as_ref()))?;
    let manifest = target.parent()?.join("Cargo.toml");
    if !manifest.is_file() {
        return None;
    }
    let release = exe
        .strip_prefix(target)
        .ok()?
        .components()
        .any(|component| component.as_os_str() == "release");
    Some(DevWorkspace { manifest, release })
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
