use crate::{
    ipc_summary::{output_summaries, status_payload, window_summaries, workspace_summaries},
    state::KestrelState,
};
use asher_config::load_config;
use asher_ipc::{
    IpcRequest, IpcResponse, ensure_socket_parent, read_request, socket_path, write_response,
};
use asher_ipc::{WindowId, WorkspaceId};
use smithay::input::keyboard::KeyboardHandle;
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
        state: &mut KestrelState,
        keyboard: &KeyboardHandle<KestrelState>,
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
    state: &mut KestrelState,
    keyboard: &KeyboardHandle<KestrelState>,
    request: IpcRequest,
) -> IpcResult {
    match request {
        IpcRequest::ShellSnapshot => IpcResult::read_only(IpcResponse::ShellSnapshot {
            status: status_payload(state),
            workspaces: workspace_summaries(state),
            windows: window_summaries(state),
        }),
        IpcRequest::ListOutputs => IpcResult::read_only(IpcResponse::Outputs {
            outputs: output_summaries(state),
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
        IpcRequest::SetOutputScale { output, scale } => set_output_scale(state, output, scale),
        IpcRequest::RestartShell => restart_shell(state),
    }
}

fn set_output_scale(state: &mut KestrelState, output: Option<String>, scale: f64) -> IpcResult {
    if !(0.5..=4.0).contains(&scale) {
        return IpcResult::read_only(IpcResponse::Error {
            message: "output scale must be between 0.5 and 4.0".to_string(),
        });
    }
    if let Some(output) = output.as_deref()
        && !state.outputs.contains(output)
    {
        return IpcResult::read_only(IpcResponse::Error {
            message: format!("unknown output {output}"),
        });
    }

    let output_name = output.as_deref();
    let previous = state
        .outputs
        .scale(output_name)
        .unwrap_or(state.output_scale());
    let changed = state.set_output_scale(output_name, scale);
    debug!(
        output = output_name.unwrap_or("primary"),
        scale, "ipc set output scale"
    );
    IpcResult::accepted(changed || (previous - scale).abs() >= f64::EPSILON)
}

fn activate_window(
    state: &mut KestrelState,
    keyboard: &KeyboardHandle<KestrelState>,
    window: WindowId,
) -> IpcResponse {
    match state.activate_window(keyboard, window) {
        Ok(()) => {
            debug!(window = window.0, "ipc activated window");
            IpcResponse::Accepted
        }
        Err(asher_ipc::LayoutError::UnknownWindow(_)) => unknown_window(window),
        Err(error) => IpcResponse::Error {
            message: error.to_string(),
        },
    }
}

fn close_window(
    state: &mut KestrelState,
    keyboard: &KeyboardHandle<KestrelState>,
    window: WindowId,
) -> IpcResponse {
    match state.close_window(window) {
        Ok(()) => {
            state.focus_active_workspace(keyboard);
            debug!(window = window.0, "ipc requested window close");
            IpcResponse::Accepted
        }
        Err(asher_ipc::LayoutError::UnknownWindow(_)) => unknown_window(window),
        Err(error) => IpcResponse::Error {
            message: error.to_string(),
        },
    }
}

fn minimize_window(
    state: &mut KestrelState,
    keyboard: &KeyboardHandle<KestrelState>,
    window: WindowId,
) -> IpcResponse {
    match state.minimize_window(keyboard, window) {
        Ok(()) => {
            debug!(window = window.0, "ipc minimized window");
            IpcResponse::Accepted
        }
        Err(asher_ipc::LayoutError::UnknownWindow(_)) => unknown_window(window),
        Err(error) => IpcResponse::Error {
            message: error.to_string(),
        },
    }
}

fn toggle_maximize_window(state: &mut KestrelState, window: WindowId) -> IpcResponse {
    match state.toggle_maximize_window(window) {
        Ok(()) => {
            debug!(window = window.0, "ipc toggled window maximize");
            IpcResponse::Accepted
        }
        Err(asher_ipc::LayoutError::UnknownWindow(_)) => unknown_window(window),
        Err(error) => IpcResponse::Error {
            message: error.to_string(),
        },
    }
}

fn move_window_to_workspace(
    state: &mut KestrelState,
    window: WindowId,
    workspace: WorkspaceId,
) -> IpcResponse {
    match state.move_window_to_workspace(window, workspace.clone()) {
        Ok(()) => {
            debug!(window = window.0, workspace = %workspace.0, "ipc moved window to workspace");
            IpcResponse::Accepted
        }
        Err(asher_ipc::LayoutError::UnknownWindow(_)) => unknown_window(window),
        Err(error) => IpcResponse::Error {
            message: error.to_string(),
        },
    }
}

fn reload_config(state: &mut KestrelState) -> IpcResult {
    match load_config() {
        Ok(loaded) => {
            let mutated = state.config != loaded.config;
            if mutated {
                state.replace_config(loaded.config);
            }
            debug!("ipc reloaded configuration");
            IpcResult::accepted(mutated)
        }
        Err(error) => IpcResult::read_only(IpcResponse::Error {
            message: error.to_string(),
        }),
    }
}

fn switch_workspace(
    state: &mut KestrelState,
    keyboard: &KeyboardHandle<KestrelState>,
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
    state: &mut KestrelState,
    keyboard: &KeyboardHandle<KestrelState>,
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

fn restart_shell(state: &mut KestrelState) -> IpcResult {
    if state.shell_control_path.is_none() {
        return IpcResult::read_only(IpcResponse::Error {
            message: "shell restart is unavailable for this backend".to_string(),
        });
    }
    state.request_shell_restart();
    debug!("ipc requested shell restart");
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
