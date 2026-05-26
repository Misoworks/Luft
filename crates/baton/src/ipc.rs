use crate::{
    ipc_summary::{
        known_profiles, profile_summaries, status_payload, window_summaries, workspace_summaries,
    },
    state::BatonState,
};
use smithay::input::keyboard::KeyboardHandle;
use staccato_config::load_config;
use staccato_ipc::{
    IpcRequest, IpcResponse, ensure_socket_parent, read_request, socket_path, write_response,
};
use staccato_layout::{ProfileId, WindowId, WorkspaceId};
use std::{
    fs, io,
    os::unix::net::UnixListener,
    path::{Path, PathBuf},
    time::Duration,
};
use tracing::debug;

#[derive(Debug)]
pub struct IpcServer {
    listener: UnixListener,
    path: PathBuf,
}

impl IpcServer {
    pub fn bind() -> io::Result<Self> {
        let path = socket_path();
        ensure_socket_parent(&path)?;
        remove_stale_socket(&path)?;
        let listener = UnixListener::bind(&path)?;
        listener.set_nonblocking(true)?;
        Ok(Self { listener, path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn handle_pending(
        &self,
        state: &mut BatonState,
        keyboard: &KeyboardHandle<BatonState>,
    ) -> io::Result<bool> {
        let mut mutated = false;
        loop {
            match self.listener.accept() {
                Ok((mut stream, _)) => {
                    stream.set_read_timeout(Some(Duration::from_millis(50)))?;
                    let result = match read_request(&mut stream) {
                        Ok(request) => handle_request(state, keyboard, request),
                        Err(error) => IpcResult::read_only(IpcResponse::Error {
                            message: error.to_string(),
                        }),
                    };
                    mutated |= result.mutated;
                    let _ = write_response(&mut stream, &result.response);
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(mutated),
                Err(error) => return Err(error),
            }
        }
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

struct IpcResult {
    response: IpcResponse,
    mutated: bool,
}

impl IpcResult {
    fn read_only(response: IpcResponse) -> Self {
        Self {
            response,
            mutated: false,
        }
    }

    fn mutating(response: IpcResponse) -> Self {
        let mutated = matches!(response, IpcResponse::Accepted);
        Self { response, mutated }
    }

    fn accepted(mutated: bool) -> Self {
        Self {
            response: IpcResponse::Accepted,
            mutated,
        }
    }
}

fn handle_request(
    state: &mut BatonState,
    keyboard: &KeyboardHandle<BatonState>,
    request: IpcRequest,
) -> IpcResult {
    match request {
        IpcRequest::Status => IpcResult::read_only(IpcResponse::Status(status_payload(state))),
        IpcRequest::ListWorkspaces => IpcResult::read_only(IpcResponse::Workspaces {
            workspaces: workspace_summaries(state),
        }),
        IpcRequest::ListProfiles => IpcResult::read_only(IpcResponse::Profiles {
            profiles: profile_summaries(state),
        }),
        IpcRequest::ListWindows => IpcResult::read_only(IpcResponse::Windows {
            windows: window_summaries(state),
        }),
        IpcRequest::ActivateWindow { window } => {
            IpcResult::mutating(activate_window(state, keyboard, window))
        }
        IpcRequest::CloseWindow { window } => {
            IpcResult::mutating(close_window(state, keyboard, window))
        }
        IpcRequest::MinimizeWindow { window } => {
            IpcResult::mutating(minimize_window(state, keyboard, window))
        }
        IpcRequest::ToggleMaximizeWindow { window } => {
            IpcResult::mutating(toggle_maximize_window(state, window))
        }
        IpcRequest::MoveWindowToWorkspace { window, workspace } => {
            IpcResult::mutating(move_window_to_workspace(state, window, workspace))
        }
        IpcRequest::SwitchWorkspace { workspace } => switch_workspace(state, keyboard, workspace),
        IpcRequest::SwitchRelativeWorkspace { offset } => {
            switch_relative_workspace(state, keyboard, offset)
        }
        IpcRequest::Reload => reload_config(state),
        IpcRequest::SetDebugOverlay { enabled } => {
            let mutated = state.config.compositor.debug_overlay != enabled
                || (enabled && state.config.general.safe_mode);
            if enabled {
                state.config.general.safe_mode = false;
            }
            state.config.compositor.debug_overlay = enabled;
            debug!(enabled, "ipc set debug overlay");
            IpcResult::accepted(mutated)
        }
        IpcRequest::SetBlur { enabled } => {
            let mutated = state.config.general.enable_blur != enabled
                || state.config.effects.blur != enabled
                || (enabled
                    && (!state.config.general.enable_effects || state.config.general.safe_mode));
            if enabled {
                state.config.general.safe_mode = false;
                state.config.general.enable_effects = true;
            }
            state.config.general.enable_blur = enabled;
            state.config.effects.blur = enabled;
            debug!(enabled, "ipc set blur");
            IpcResult::accepted(mutated)
        }
        IpcRequest::SetSafeMode { enabled } => {
            let mutated = state.config.general.safe_mode != enabled
                || (enabled
                    && (state.config.general.enable_effects
                        || state.config.general.enable_animations
                        || state.config.general.enable_blur
                        || state.config.effects.blur
                        || state.config.compositor.debug_overlay));
            state.config.general.safe_mode = enabled;
            if enabled {
                state.config.general.enable_effects = false;
                state.config.general.enable_animations = false;
                state.config.general.enable_blur = false;
                state.config.effects.blur = false;
                state.config.compositor.debug_overlay = false;
            }
            debug!(enabled, "ipc set safe mode");
            IpcResult::accepted(mutated)
        }
        IpcRequest::RestartShell => restart_shell(state),
        IpcRequest::FallbackToDefaultConfig => fallback_to_default_config(state),
        IpcRequest::SetWorkspaceProfile { workspace, profile } => {
            IpcResult::mutating(set_workspace_profile(state, workspace, profile))
        }
    }
}

fn activate_window(
    state: &mut BatonState,
    keyboard: &KeyboardHandle<BatonState>,
    window: WindowId,
) -> IpcResponse {
    match state.activate_window(keyboard, window) {
        Ok(()) => {
            debug!(window = window.0, "ipc activated window");
            IpcResponse::Accepted
        }
        Err(staccato_layout::LayoutError::UnknownWindow(_)) => unknown_window(window),
        Err(error) => IpcResponse::Error {
            message: error.to_string(),
        },
    }
}

fn close_window(
    state: &mut BatonState,
    keyboard: &KeyboardHandle<BatonState>,
    window: WindowId,
) -> IpcResponse {
    match state.close_window(window) {
        Ok(()) => {
            state.focus_active_workspace(keyboard);
            debug!(window = window.0, "ipc requested window close");
            IpcResponse::Accepted
        }
        Err(staccato_layout::LayoutError::UnknownWindow(_)) => unknown_window(window),
        Err(error) => IpcResponse::Error {
            message: error.to_string(),
        },
    }
}

fn minimize_window(
    state: &mut BatonState,
    keyboard: &KeyboardHandle<BatonState>,
    window: WindowId,
) -> IpcResponse {
    match state.minimize_window(keyboard, window) {
        Ok(()) => {
            debug!(window = window.0, "ipc minimized window");
            IpcResponse::Accepted
        }
        Err(staccato_layout::LayoutError::UnknownWindow(_)) => unknown_window(window),
        Err(error) => IpcResponse::Error {
            message: error.to_string(),
        },
    }
}

fn toggle_maximize_window(state: &mut BatonState, window: WindowId) -> IpcResponse {
    match state.toggle_maximize_window(window) {
        Ok(()) => {
            debug!(window = window.0, "ipc toggled window maximize");
            IpcResponse::Accepted
        }
        Err(staccato_layout::LayoutError::UnknownWindow(_)) => unknown_window(window),
        Err(error) => IpcResponse::Error {
            message: error.to_string(),
        },
    }
}

fn move_window_to_workspace(
    state: &mut BatonState,
    window: WindowId,
    workspace: WorkspaceId,
) -> IpcResponse {
    match state.move_window_to_workspace(window, workspace.clone()) {
        Ok(()) => {
            debug!(window = window.0, workspace = %workspace.0, "ipc moved window to workspace");
            IpcResponse::Accepted
        }
        Err(staccato_layout::LayoutError::UnknownWindow(_)) => unknown_window(window),
        Err(error) => IpcResponse::Error {
            message: error.to_string(),
        },
    }
}

fn reload_config(state: &mut BatonState) -> IpcResult {
    match load_config() {
        Ok(loaded) => {
            let mutated = state.config != loaded.config;
            state.config = loaded.config;
            debug!("ipc reloaded configuration");
            IpcResult::accepted(mutated)
        }
        Err(error) => IpcResult::read_only(IpcResponse::Error {
            message: error.to_string(),
        }),
    }
}

fn switch_workspace(
    state: &mut BatonState,
    keyboard: &KeyboardHandle<BatonState>,
    workspace: WorkspaceId,
) -> IpcResult {
    let mutated = state.layout.active_workspace() != &workspace;
    match state.switch_workspace(keyboard, &workspace) {
        Ok(()) => {
            debug!(workspace = %workspace.0, "ipc switched workspace");
            IpcResult::accepted(mutated)
        }
        Err(error) => IpcResult::read_only(IpcResponse::Error {
            message: error.to_string(),
        }),
    }
}

fn switch_relative_workspace(
    state: &mut BatonState,
    keyboard: &KeyboardHandle<BatonState>,
    offset: i32,
) -> IpcResult {
    let active = state.layout.active_workspace().clone();
    let count = state.layout.workspaces().count();
    match state.switch_relative_workspace(keyboard, offset) {
        Ok(()) => {
            let mutated = state.layout.active_workspace() != &active
                || state.layout.workspaces().count() != count;
            debug!(offset, workspace = %state.layout.active_workspace().0, "ipc switched relative workspace");
            IpcResult::accepted(mutated)
        }
        Err(error) => IpcResult::read_only(IpcResponse::Error {
            message: error.to_string(),
        }),
    }
}

fn set_workspace_profile(
    state: &mut BatonState,
    workspace: WorkspaceId,
    profile: ProfileId,
) -> IpcResponse {
    if profile.0.trim().is_empty() {
        return IpcResponse::Error {
            message: "workspace profile cannot be empty".to_string(),
        };
    }
    if !known_profiles(state).iter().any(|known| known == &profile) {
        return IpcResponse::Error {
            message: format!("unknown profile {}", profile.0),
        };
    }

    match state
        .layout
        .set_workspace_profile(&workspace, profile.clone())
    {
        Ok(()) => {
            if &workspace == state.layout.active_workspace() {
                state.apply_active_arrangement();
            }
            debug!(workspace = %workspace.0, profile = %profile.0, "ipc set workspace profile");
            IpcResponse::Accepted
        }
        Err(error) => IpcResponse::Error {
            message: error.to_string(),
        },
    }
}

fn restart_shell(state: &mut BatonState) -> IpcResult {
    if state.shell_control_path.is_none() {
        return IpcResult::read_only(IpcResponse::Error {
            message: "shell restart is unavailable for this backend".to_string(),
        });
    }
    state.request_shell_restart();
    debug!("ipc requested shell restart");
    IpcResult::accepted(true)
}

fn fallback_to_default_config(state: &mut BatonState) -> IpcResult {
    state.fallback_to_default_config();
    debug!("ipc requested built-in default config fallback");
    IpcResult::accepted(true)
}

fn unknown_window(window: WindowId) -> IpcResponse {
    IpcResponse::Error {
        message: format!("unknown window {}", window.0),
    }
}

fn remove_stale_socket(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}
