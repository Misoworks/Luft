mod diagnostics;
mod dock_cli;
mod doctor;
mod mode_cli;
mod recovery_cli;
mod workspace_cli;

use clap::{Parser, Subcommand};
use diagnostics::{
    open_config, print_config_path, print_doctor, print_logs, print_recovery_status,
    validate_config,
};
use dock_cli::{list_dock_pins, pin_dock_app, unpin_dock_app};
use mode_cli::list_modes;
use recovery_cli::{list_recovery_backups, rollback_config};
use staccato_config::{ConfigSource, load_config};
use staccato_ipc::{IpcRequest, IpcResponse, ShellStatus, StatusPayload, send_request};
use staccato_layout::{ProfileId, WindowId, WorkspaceId, mode_for_profile};
use workspace_cli::{list_profiles, list_workspaces};

#[derive(Debug, Parser)]
#[command(name = "staccatoctl", about = "Control and inspect Staccato")]
struct Cli {
    #[command(subcommand)]
    command: Command,
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Debug, Subcommand)]
enum Command {
    Status,
    Logs {
        component: Option<String>,
        #[arg(short, long)]
        lines: Option<usize>,
        #[arg(long)]
        path: bool,
    },
    Doctor,
    Reload,
    #[command(subcommand)]
    Recovery(RecoveryCommand),
    #[command(subcommand)]
    Effects(EffectsCommand),
    #[command(subcommand)]
    Debug(DebugCommand),
    #[command(subcommand)]
    SafeMode(SafeModeCommand),
    #[command(subcommand)]
    Shell(ShellCommand),
    #[command(subcommand)]
    Config(ConfigCommand),
    #[command(subcommand)]
    Dock(DockCommand),
    #[command(subcommand)]
    #[command(alias = "modes")]
    Mode(ModeCommand),
    #[command(subcommand)]
    #[command(alias = "profiles")]
    Profile(ProfileCommand),
    #[command(subcommand)]
    #[command(alias = "workspaces")]
    Workspace(WorkspaceCommand),
    #[command(subcommand)]
    Window(WindowCommand),
}

#[derive(Debug, Subcommand)]
enum ConfigCommand {
    Path,
    Validate,
    Open,
}

#[derive(Debug, Subcommand)]
enum RecoveryCommand {
    Status,
    Backups,
    Rollback,
    Defaults,
}

#[derive(Debug, Subcommand)]
enum DockCommand {
    List,
    Pin {
        command: String,
        #[arg(short, long)]
        label: Option<String>,
        #[arg(long)]
        icon: Option<String>,
    },
    Unpin {
        app: String,
    },
}

#[derive(Debug, Subcommand)]
enum ModeCommand {
    List,
}

#[derive(Debug, Subcommand)]
enum EffectsCommand {
    Status,
    Blur { state: ToggleState },
}

#[derive(Debug, Subcommand)]
enum DebugCommand {
    Overlay { state: ToggleState },
}

#[derive(Debug, Subcommand)]
enum SafeModeCommand {
    Enable,
    Disable,
    Set { state: ToggleState },
}

#[derive(Debug, Subcommand)]
enum ShellCommand {
    Restart,
}

#[derive(Debug, Subcommand)]
enum ProfileCommand {
    List,
    SetWorkspace { workspace: String, profile: String },
}

#[derive(Debug, Subcommand)]
enum WorkspaceCommand {
    List,
    Switch {
        workspace: String,
    },
    Next,
    Previous,
    #[command(alias = "set-profile")]
    Profile {
        workspace: String,
        profile: String,
    },
    Style {
        style: WorkspaceStyle,
    },
}

#[derive(Debug, Subcommand)]
enum WindowCommand {
    List,
    Focus { window: u64 },
    Close { window: u64 },
    Minimize { window: u64 },
    Maximize { window: u64 },
    Move { window: u64, workspace: String },
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum ToggleState {
    #[value(alias = "enable", alias = "enabled")]
    On,
    #[value(alias = "disable", alias = "disabled")]
    Off,
}

impl ToggleState {
    const fn enabled(self) -> bool {
        matches!(self, Self::On)
    }
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum WorkspaceStyle {
    Dock,
    Panel,
}

impl WorkspaceStyle {
    const fn profile_id(self) -> &'static str {
        match self {
            Self::Dock => "dock-default",
            Self::Panel => "panel-default",
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .without_time()
        .with_target(false)
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Status => print_status(cli.json)?,
        Command::Logs {
            component,
            lines,
            path,
        } => print_logs(component, lines, path, cli.json)?,
        Command::Doctor => print_doctor(cli.json)?,
        Command::Reload => accepted(send_request(&IpcRequest::Reload)?)?,
        Command::Recovery(RecoveryCommand::Status) => print_recovery_status(cli.json)?,
        Command::Recovery(RecoveryCommand::Backups) => list_recovery_backups(cli.json)?,
        Command::Recovery(RecoveryCommand::Rollback) => rollback_config(cli.json)?,
        Command::Recovery(RecoveryCommand::Defaults) => {
            accepted(send_request(&IpcRequest::FallbackToDefaultConfig)?)?
        }
        Command::Effects(EffectsCommand::Status) => print_effects_status(cli.json)?,
        Command::Effects(EffectsCommand::Blur { state }) => {
            accepted(send_request(&IpcRequest::SetBlur {
                enabled: state.enabled(),
            })?)?
        }
        Command::Debug(DebugCommand::Overlay { state }) => {
            accepted(send_request(&IpcRequest::SetDebugOverlay {
                enabled: state.enabled(),
            })?)?
        }
        Command::SafeMode(SafeModeCommand::Set { state }) => {
            accepted(send_request(&IpcRequest::SetSafeMode {
                enabled: state.enabled(),
            })?)?
        }
        Command::SafeMode(SafeModeCommand::Enable) => {
            accepted(send_request(&IpcRequest::SetSafeMode { enabled: true })?)?
        }
        Command::SafeMode(SafeModeCommand::Disable) => {
            accepted(send_request(&IpcRequest::SetSafeMode { enabled: false })?)?
        }
        Command::Shell(ShellCommand::Restart) => {
            accepted(send_request(&IpcRequest::RestartShell)?)?
        }
        Command::Config(ConfigCommand::Path) => print_config_path(cli.json)?,
        Command::Config(ConfigCommand::Validate) => validate_config(cli.json)?,
        Command::Config(ConfigCommand::Open) => open_config(cli.json)?,
        Command::Dock(DockCommand::List) => list_dock_pins(cli.json)?,
        Command::Dock(DockCommand::Pin {
            command,
            label,
            icon,
        }) => pin_dock_app(command, label, icon, cli.json)?,
        Command::Dock(DockCommand::Unpin { app }) => unpin_dock_app(app, cli.json)?,
        Command::Mode(ModeCommand::List) => list_modes(cli.json)?,
        Command::Profile(ProfileCommand::List) => list_profiles(cli.json)?,
        Command::Profile(ProfileCommand::SetWorkspace { workspace, profile }) => {
            set_workspace_profile(workspace, profile)?
        }
        Command::Workspace(WorkspaceCommand::List) => list_workspaces(cli.json)?,
        Command::Workspace(WorkspaceCommand::Switch { workspace }) => switch_workspace(workspace)?,
        Command::Workspace(WorkspaceCommand::Next) => switch_relative_workspace(1)?,
        Command::Workspace(WorkspaceCommand::Previous) => switch_relative_workspace(-1)?,
        Command::Workspace(WorkspaceCommand::Profile { workspace, profile }) => {
            set_workspace_profile(workspace, profile)?
        }
        Command::Workspace(WorkspaceCommand::Style { style }) => set_active_workspace_style(style)?,
        Command::Window(WindowCommand::List) => list_windows(cli.json)?,
        Command::Window(WindowCommand::Focus { window }) => focus_window(window)?,
        Command::Window(WindowCommand::Close { window }) => close_window(window)?,
        Command::Window(WindowCommand::Minimize { window }) => minimize_window(window)?,
        Command::Window(WindowCommand::Maximize { window }) => maximize_window(window)?,
        Command::Window(WindowCommand::Move { window, workspace }) => {
            move_window(window, workspace)?
        }
    }

    Ok(())
}

fn print_status(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    if let Ok(IpcResponse::Status(status)) = send_request(&IpcRequest::Status) {
        print_status_payload(json, status, "live Baton IPC".to_string())?;
        return Ok(());
    }

    let loaded = load_config()?;
    let active_profile = ProfileId(loaded.config.general.default_profile.clone());
    let status = StatusPayload {
        compositor: "baton".to_string(),
        shell: ShellStatus::NotStarted,
        xwayland: if loaded.config.compositor.xwayland {
            staccato_ipc::XwaylandStatus::Unavailable
        } else {
            staccato_ipc::XwaylandStatus::Disabled
        },
        xwayland_display: None,
        active_workspace: WorkspaceId("1".to_string()),
        active_profile: active_profile.clone(),
        active_mode: mode_for_profile(&active_profile),
        nested: false,
        safe_mode: loaded.config.general.safe_mode,
        blur_enabled: loaded.config.general.enable_blur && loaded.config.effects.blur,
        debug_overlay: loaded.config.compositor.debug_overlay,
    };

    let source = match loaded.source {
        ConfigSource::User(path) => path.display().to_string(),
        ConfigSource::Defaults => "built-in defaults".to_string(),
    };
    print_status_payload(json, status, source)
}

fn print_effects_status(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let (blur_enabled, debug_overlay, safe_mode, source) =
        if let Ok(IpcResponse::Status(status)) = send_request(&IpcRequest::Status) {
            (
                status.blur_enabled,
                status.debug_overlay,
                status.safe_mode,
                "live Baton IPC".to_string(),
            )
        } else {
            let loaded = load_config()?;
            let source = match loaded.source {
                ConfigSource::User(path) => path.display().to_string(),
                ConfigSource::Defaults => "built-in defaults".to_string(),
            };
            (
                loaded.config.general.enable_blur && loaded.config.effects.blur,
                loaded.config.compositor.debug_overlay,
                loaded.config.general.safe_mode,
                source,
            )
        };

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "blurEnabled": blur_enabled,
                "debugOverlay": debug_overlay,
                "safeMode": safe_mode,
                "source": source,
            }))?
        );
    } else {
        println!("Blur enabled: {blur_enabled}");
        println!("Debug overlay: {debug_overlay}");
        println!("Safe mode: {safe_mode}");
        println!("Source: {source}");
    }

    Ok(())
}

fn print_status_payload(
    json: bool,
    status: StatusPayload,
    source: String,
) -> Result<(), Box<dyn std::error::Error>> {
    if json {
        println!("{}", serde_json::to_string_pretty(&status)?);
    } else {
        println!("Compositor: {}", status.compositor);
        println!("Shell: {:?}", status.shell);
        println!("XWayland: {:?}", status.xwayland);
        if let Some(display) = status.xwayland_display {
            println!("XWayland display: {display}");
        }
        println!("Active workspace: {}", status.active_workspace.0);
        println!("Active profile: {}", status.active_profile.0);
        println!("Safe mode: {}", status.safe_mode);
        println!("Blur enabled: {}", status.blur_enabled);
        println!("Debug overlay: {}", status.debug_overlay);
        println!("Source: {source}");
    }

    Ok(())
}

fn switch_workspace(workspace: String) -> Result<(), Box<dyn std::error::Error>> {
    match send_request(&IpcRequest::SwitchWorkspace {
        workspace: WorkspaceId(workspace),
    })? {
        IpcResponse::Accepted => Ok(()),
        IpcResponse::Error { message } => Err(message.into()),
        response => Err(format!("unexpected response: {response:?}").into()),
    }
}

fn switch_relative_workspace(offset: i32) -> Result<(), Box<dyn std::error::Error>> {
    accepted(send_request(&IpcRequest::SwitchRelativeWorkspace {
        offset,
    })?)
}

fn set_workspace_profile(
    workspace: String,
    profile: String,
) -> Result<(), Box<dyn std::error::Error>> {
    accepted(send_request(&IpcRequest::SetWorkspaceProfile {
        workspace: WorkspaceId(workspace),
        profile: ProfileId(profile),
    })?)
}

fn set_active_workspace_style(style: WorkspaceStyle) -> Result<(), Box<dyn std::error::Error>> {
    let status = match send_request(&IpcRequest::Status)? {
        IpcResponse::Status(status) => status,
        IpcResponse::Error { message } => return Err(message.into()),
        response => return Err(format!("unexpected response: {response:?}").into()),
    };

    accepted(send_request(&IpcRequest::SetWorkspaceProfile {
        workspace: status.active_workspace,
        profile: ProfileId(style.profile_id().to_string()),
    })?)
}

fn list_windows(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    match send_request(&IpcRequest::ListWindows)? {
        IpcResponse::Windows { windows } => {
            if json {
                println!("{}", serde_json::to_string_pretty(&windows)?);
            } else {
                for window in windows {
                    let marker = if window.is_active { "*" } else { " " };
                    let title = window
                        .title
                        .or(window.app_id)
                        .unwrap_or_else(|| "Untitled".to_string());
                    println!(
                        "{}{}\t{}\t{}\t{}x{}+{}+{}\t{:?}",
                        marker,
                        window.id.0,
                        window.workspace.0,
                        title,
                        window.geometry.width,
                        window.geometry.height,
                        window.geometry.x,
                        window.geometry.y,
                        window.state
                    );
                }
            }
            Ok(())
        }
        IpcResponse::Error { message } => Err(message.into()),
        response => Err(format!("unexpected response: {response:?}").into()),
    }
}

fn focus_window(window: u64) -> Result<(), Box<dyn std::error::Error>> {
    accepted(send_request(&IpcRequest::ActivateWindow {
        window: WindowId(window),
    })?)
}

fn close_window(window: u64) -> Result<(), Box<dyn std::error::Error>> {
    accepted(send_request(&IpcRequest::CloseWindow {
        window: WindowId(window),
    })?)
}

fn minimize_window(window: u64) -> Result<(), Box<dyn std::error::Error>> {
    accepted(send_request(&IpcRequest::MinimizeWindow {
        window: WindowId(window),
    })?)
}

fn maximize_window(window: u64) -> Result<(), Box<dyn std::error::Error>> {
    accepted(send_request(&IpcRequest::ToggleMaximizeWindow {
        window: WindowId(window),
    })?)
}

fn move_window(window: u64, workspace: String) -> Result<(), Box<dyn std::error::Error>> {
    accepted(send_request(&IpcRequest::MoveWindowToWorkspace {
        window: WindowId(window),
        workspace: WorkspaceId(workspace),
    })?)
}

fn accepted(response: IpcResponse) -> Result<(), Box<dyn std::error::Error>> {
    match response {
        IpcResponse::Accepted => Ok(()),
        IpcResponse::Error { message } => Err(message.into()),
        response => Err(format!("unexpected response: {response:?}").into()),
    }
}
