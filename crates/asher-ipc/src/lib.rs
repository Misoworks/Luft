use serde::{Deserialize, Serialize};
use std::{
    env, fs, io,
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
};

mod layout;

pub use layout::{
    ActionResult, AppId, Arrangement, ChromeSpec, GroupId, GroupNode, LayoutContext, LayoutEngine,
    LayoutError, LayoutNode, ModeContext, ModeId, PanelMode, ProfileId, Rect, ShellAction,
    ShellMode, ShellProfile, SplitAxis, SplitNode, SplitNodeId, TabStack, TabStackId, WindowId,
    WindowInfo, WindowState, Workspace, WorkspaceId, WorkspaceRule, mode_for_profile, shell_mode,
    state_for_mode,
};

pub const SOCKET_ENV: &str = "ASHER_IPC_SOCKET";
pub const SHELL_SOCKET_ENV: &str = "ASHER_SHELL_SOCKET";

pub fn socket_path() -> PathBuf {
    if let Some(path) = env::var_os(SOCKET_ENV) {
        return PathBuf::from(path);
    }

    runtime_dir().join("asher").join("kestrel.sock")
}

pub fn shell_socket_path(ipc_socket: &Path) -> PathBuf {
    let mut path = ipc_socket.to_path_buf();
    let file_name = ipc_socket
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("kestrel.sock");
    path.set_file_name(format!("{file_name}.shell"));
    path
}

pub fn ensure_socket_parent(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

pub fn send_request(request: &IpcRequest) -> io::Result<IpcResponse> {
    send_request_to(&socket_path(), request)
}

pub fn send_request_to(path: &Path, request: &IpcRequest) -> io::Result<IpcResponse> {
    let mut stream = UnixStream::connect(path)?;
    write_json(&mut stream, request)?;
    stream.shutdown(std::net::Shutdown::Write)?;
    read_json(&mut stream)
}

pub fn send_shell_control_to(path: &Path, request: &ShellControlRequest) -> io::Result<()> {
    let mut stream = UnixStream::connect(path)?;
    write_json(&mut stream, request)?;
    stream.shutdown(std::net::Shutdown::Write)
}

pub fn read_request(stream: &mut UnixStream) -> io::Result<IpcRequest> {
    read_json(stream)
}

pub fn read_shell_control(stream: &mut UnixStream) -> io::Result<ShellControlRequest> {
    read_json(stream)
}

pub fn write_response(stream: &mut UnixStream, response: &IpcResponse) -> io::Result<()> {
    write_json(stream, response)
}

fn runtime_dir() -> PathBuf {
    env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| env::temp_dir().join(format!("asher-{}", current_user())))
}

fn current_user() -> String {
    env::var("USER").unwrap_or_else(|_| "user".to_string())
}

fn read_json<T: for<'de> Deserialize<'de>>(stream: &mut UnixStream) -> io::Result<T> {
    serde_json::from_reader(stream).map_err(json_error)
}

fn write_json<T: Serialize>(stream: &mut UnixStream, value: &T) -> io::Result<()> {
    serde_json::to_writer(stream, value).map_err(json_error)
}

fn json_error(error: serde_json::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum IpcRequest {
    Status,
    ShellSnapshot,
    Reload,
    ListProfiles,
    ListWorkspaces,
    ListWindows,
    ListOutputs,
    ActivateWindow {
        window: WindowId,
    },
    CloseWindow {
        window: WindowId,
    },
    MinimizeWindow {
        window: WindowId,
    },
    ToggleMaximizeWindow {
        window: WindowId,
    },
    MoveWindowToWorkspace {
        window: WindowId,
        workspace: WorkspaceId,
    },
    SetWorkspaceProfile {
        workspace: WorkspaceId,
        profile: ProfileId,
    },
    SwitchWorkspace {
        workspace: WorkspaceId,
    },
    SwitchRelativeWorkspace {
        offset: i32,
    },
    SetDebugOverlay {
        enabled: bool,
    },
    SetBlur {
        enabled: bool,
    },
    SetOutputScale {
        output: Option<String>,
        scale: f64,
    },
    RestartShell,
    FallbackToDefaultConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ShellControlRequest {
    LaunchDefaultApp { app: DefaultAppKind },
    OpenLauncher,
    ToggleStartMenu,
    CloseTransientPopovers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DefaultAppKind {
    Terminal,
    FileManager,
    Browser,
    Settings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum IpcResponse {
    Status(StatusPayload),
    ShellSnapshot {
        status: StatusPayload,
        workspaces: Vec<WorkspaceSummary>,
        profiles: Vec<ProfileSummary>,
        windows: Vec<WindowSummary>,
    },
    Profiles {
        profiles: Vec<ProfileSummary>,
    },
    Workspaces {
        workspaces: Vec<WorkspaceSummary>,
    },
    Windows {
        windows: Vec<WindowSummary>,
    },
    Outputs {
        outputs: Vec<OutputSummary>,
    },
    Accepted,
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusPayload {
    pub compositor: String,
    pub shell: ShellStatus,
    pub xwayland: XwaylandStatus,
    pub xwayland_display: Option<String>,
    pub active_workspace: WorkspaceId,
    pub active_profile: ProfileId,
    pub active_mode: ModeId,
    pub nested: bool,
    pub blur_enabled: bool,
    pub debug_overlay: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputSummary {
    pub name: String,
    pub make: String,
    pub model: String,
    pub width: i32,
    pub height: i32,
    pub refresh_millihertz: i32,
    pub scale: f64,
    pub primary: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShellStatus {
    NotStarted,
    Running,
    Restarting,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum XwaylandStatus {
    Disabled,
    Unavailable,
    Running,
    Restarting,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileSummary {
    pub id: ProfileId,
    pub name: String,
    pub mode: ModeId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceSummary {
    pub id: WorkspaceId,
    pub name: String,
    pub profile: ProfileId,
    pub mode: ModeId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowSummary {
    pub id: WindowId,
    pub title: Option<String>,
    pub app_id: Option<String>,
    pub pid: Option<u32>,
    pub workspace: WorkspaceId,
    pub state: WindowState,
    pub geometry: Rect,
    pub is_active: bool,
    pub is_visible: bool,
}
