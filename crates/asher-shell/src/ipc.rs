use asher_ipc::{
    IpcRequest, IpcResponse, ProfileSummary, StatusPayload, WindowSummary, WorkspaceSummary,
    send_request,
};
use asher_ipc::{ModeId, ProfileId, WindowId, WorkspaceId};
use std::{
    error::Error,
    io,
    sync::atomic::{AtomicBool, Ordering},
};

static SHELL_SNAPSHOT_SUPPORTED: AtomicBool = AtomicBool::new(true);

#[derive(Debug, Clone)]
pub struct ShellModel {
    pub active_workspace: WorkspaceId,
    pub active_profile: ProfileId,
    pub active_mode: ModeId,
    pub xwayland_display: Option<String>,
    pub blur_enabled: bool,
    pub debug_overlay: bool,
    pub workspaces: Vec<WorkspaceSummary>,
    pub profiles: Vec<ProfileSummary>,
    pub windows: Vec<WindowSummary>,
}

pub fn load_model() -> Result<ShellModel, Box<dyn Error>> {
    if SHELL_SNAPSHOT_SUPPORTED.load(Ordering::Relaxed) {
        match send_request(&IpcRequest::ShellSnapshot)? {
            IpcResponse::ShellSnapshot {
                status,
                workspaces,
                profiles,
                windows,
            } => Ok(shell_model_from_parts(
                status, workspaces, profiles, windows,
            )),
            IpcResponse::Error { message } if shell_snapshot_unsupported(&message) => {
                SHELL_SNAPSHOT_SUPPORTED.store(false, Ordering::Relaxed);
                load_model_legacy()
            }
            IpcResponse::Error { message } => Err(message.into()),
            response => Err(unexpected_response(response).into()),
        }
    } else {
        load_model_legacy()
    }
}

fn load_model_legacy() -> Result<ShellModel, Box<dyn Error>> {
    let status = match send_request(&IpcRequest::Status)? {
        IpcResponse::Status(status) => status,
        IpcResponse::Error { message } => return Err(message.into()),
        response => return Err(unexpected_response(response).into()),
    };
    let workspaces = match send_request(&IpcRequest::ListWorkspaces)? {
        IpcResponse::Workspaces { workspaces } => workspaces,
        IpcResponse::Error { message } => return Err(message.into()),
        response => return Err(unexpected_response(response).into()),
    };
    let profiles = match send_request(&IpcRequest::ListProfiles)? {
        IpcResponse::Profiles { profiles } => profiles,
        IpcResponse::Error { message } => return Err(message.into()),
        response => return Err(unexpected_response(response).into()),
    };
    let windows = match send_request(&IpcRequest::ListWindows)? {
        IpcResponse::Windows { windows } => windows,
        IpcResponse::Error { message } => return Err(message.into()),
        response => return Err(unexpected_response(response).into()),
    };

    Ok(shell_model_from_parts(
        status, workspaces, profiles, windows,
    ))
}

fn shell_model_from_parts(
    status: StatusPayload,
    workspaces: Vec<WorkspaceSummary>,
    profiles: Vec<ProfileSummary>,
    windows: Vec<WindowSummary>,
) -> ShellModel {
    ShellModel {
        active_workspace: status.active_workspace,
        active_profile: status.active_profile,
        active_mode: status.active_mode,
        xwayland_display: status.xwayland_display,
        blur_enabled: status.blur_enabled,
        debug_overlay: status.debug_overlay,
        workspaces,
        profiles,
        windows,
    }
}

fn shell_snapshot_unsupported(message: &str) -> bool {
    message.contains("shell-snapshot") || message.contains("unknown variant")
}

pub fn switch_workspace(workspace: WorkspaceId) -> Result<ShellModel, Box<dyn Error>> {
    match send_request(&IpcRequest::SwitchWorkspace { workspace })? {
        IpcResponse::Accepted => load_model(),
        IpcResponse::Error { message } => Err(message.into()),
        response => Err(unexpected_response(response).into()),
    }
}

pub fn switch_relative_workspace(offset: i32) -> Result<ShellModel, Box<dyn Error>> {
    match send_request(&IpcRequest::SwitchRelativeWorkspace { offset })? {
        IpcResponse::Accepted => load_model(),
        IpcResponse::Error { message } => Err(message.into()),
        response => Err(unexpected_response(response).into()),
    }
}

pub fn set_debug_overlay(enabled: bool) -> Result<ShellModel, Box<dyn Error>> {
    send_accepted(IpcRequest::SetDebugOverlay { enabled })?;
    load_model()
}

pub fn reload_config() -> Result<ShellModel, Box<dyn Error>> {
    send_accepted(IpcRequest::Reload)?;
    load_model()
}

pub fn set_workspace_profile(
    workspace: WorkspaceId,
    profile: ProfileId,
) -> Result<ShellModel, Box<dyn Error>> {
    send_accepted(IpcRequest::SetWorkspaceProfile { workspace, profile })?;
    load_model()
}

pub fn activate_window(window: WindowId) -> Result<ShellModel, Box<dyn Error>> {
    send_accepted(IpcRequest::ActivateWindow { window })?;
    load_model()
}

pub fn minimize_window(window: WindowId) -> Result<ShellModel, Box<dyn Error>> {
    send_accepted(IpcRequest::MinimizeWindow { window })?;
    load_model()
}

pub fn close_window(window: WindowId) -> Result<ShellModel, Box<dyn Error>> {
    send_accepted(IpcRequest::CloseWindow { window })?;
    load_model()
}

pub fn move_window_to_workspace(
    window: WindowId,
    workspace: WorkspaceId,
) -> Result<ShellModel, Box<dyn Error>> {
    send_accepted(IpcRequest::MoveWindowToWorkspace { window, workspace })?;
    load_model()
}

fn send_accepted(request: IpcRequest) -> Result<(), Box<dyn Error>> {
    match send_request(&request)? {
        IpcResponse::Accepted => Ok(()),
        IpcResponse::Error { message } => Err(message.into()),
        response => Err(unexpected_response(response).into()),
    }
}

fn unexpected_response(response: IpcResponse) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("unexpected ipc response: {response:?}"),
    )
}

#[cfg(test)]
mod tests {
    use super::shell_snapshot_unsupported;

    #[test]
    fn detects_unsupported_shell_snapshot_errors() {
        let message = "unknown variant `shell-snapshot`, expected one of `status`";
        assert!(shell_snapshot_unsupported(message));
    }
}
