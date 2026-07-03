use luft_ipc::{
    IpcRequest, IpcResponse, StatusPayload, WindowId, WindowSummary, WorkspaceId, WorkspaceSummary,
    send_request,
};
use std::{error::Error, io};

#[derive(Debug, Clone)]
pub struct ShellModel {
    pub active_workspace: WorkspaceId,
    pub xwayland_display: Option<String>,
    pub workspaces: Vec<WorkspaceSummary>,
    pub windows: Vec<WindowSummary>,
}

pub fn load_model() -> Result<ShellModel, Box<dyn Error>> {
    match send_request(&IpcRequest::ShellSnapshot)? {
        IpcResponse::ShellSnapshot {
            status,
            workspaces,
            windows,
        } => Ok(shell_model_from_parts(status, workspaces, windows)),
        IpcResponse::Error { message } => Err(message.into()),
        response => Err(unexpected_response(response).into()),
    }
}

fn shell_model_from_parts(
    status: StatusPayload,
    workspaces: Vec<WorkspaceSummary>,
    windows: Vec<WindowSummary>,
) -> ShellModel {
    ShellModel {
        active_workspace: status.active_workspace,
        xwayland_display: status.xwayland_display,
        workspaces,
        windows,
    }
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

pub fn reload_config() -> Result<ShellModel, Box<dyn Error>> {
    send_accepted(IpcRequest::Reload)?;
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
