use crate::state::KestrelState;
use asher_ipc::WorkspaceId;
use smithay::{
    backend::input::KeyState,
    input::keyboard::{KeyboardHandle, Keysym, ModifiersState},
};

#[derive(Debug)]
pub(super) enum ShortcutAction {
    Forward,
    SwitchWorkspace(WorkspaceId),
    SwitchRelativeWorkspace(i32),
    MoveWindowToWorkspace(WorkspaceId),
    CloseActiveWindow,
    CycleWindow { previous: bool },
    OpenDefaultApp(asher_ipc::DefaultAppKind),
    OpenLauncher,
    RestartShell,
    SuperPress,
    SuperRelease,
}

impl ShortcutAction {
    pub(super) fn is_forward(&self) -> bool {
        matches!(self, Self::Forward)
    }
}

pub(super) fn shortcut_for_key(
    modifiers: &ModifiersState,
    key: Option<Keysym>,
    state: KeyState,
) -> ShortcutAction {
    let Some(raw) = key.map(|key| key.raw()) else {
        return ShortcutAction::Forward;
    };

    if is_super_key(raw) {
        return if state == KeyState::Pressed {
            ShortcutAction::SuperPress
        } else {
            ShortcutAction::SuperRelease
        };
    }

    if modifiers.alt && !modifiers.logo && !modifiers.ctrl && matches!(raw, 0xff09 | 0xfe20) {
        return ShortcutAction::CycleWindow {
            previous: modifiers.shift,
        };
    }

    if !modifiers.logo || modifiers.ctrl || modifiers.alt {
        return ShortcutAction::Forward;
    }

    if let Some(workspace) = workspace_for_raw_key(raw) {
        if modifiers.shift {
            return ShortcutAction::MoveWindowToWorkspace(workspace);
        }

        return ShortcutAction::SwitchWorkspace(workspace);
    }

    match raw {
        0x20 => ShortcutAction::OpenLauncher,
        0xff0d => ShortcutAction::OpenDefaultApp(asher_ipc::DefaultAppKind::Terminal),
        0x65 | 0x45 => ShortcutAction::OpenDefaultApp(asher_ipc::DefaultAppKind::FileManager),
        0x72 | 0x52 if modifiers.shift => ShortcutAction::RestartShell,
        0x71 | 0x51 => ShortcutAction::CloseActiveWindow,
        0xff09 | 0xfe20 => ShortcutAction::CycleWindow {
            previous: modifiers.shift,
        },
        0xff51 | 0xff52 => ShortcutAction::SwitchRelativeWorkspace(-1),
        0xff53 | 0xff54 => ShortcutAction::SwitchRelativeWorkspace(1),
        _ => ShortcutAction::Forward,
    }
}

pub(super) fn handle_shortcut(
    state: &mut KestrelState,
    keyboard: &KeyboardHandle<KestrelState>,
    shortcut: ShortcutAction,
) {
    match shortcut {
        ShortcutAction::Forward => {}
        ShortcutAction::SwitchWorkspace(workspace) => {
            state.super_used = true;
            let _ = state.switch_workspace(keyboard, &workspace);
        }
        ShortcutAction::SwitchRelativeWorkspace(offset) => {
            state.super_used = true;
            let _ = state.switch_relative_workspace(keyboard, offset);
        }
        ShortcutAction::MoveWindowToWorkspace(workspace) => {
            state.super_used = true;
            let _ = state.move_active_window_to_workspace(keyboard, workspace);
        }
        ShortcutAction::CloseActiveWindow => {
            state.super_used = true;
            if state.close_active_window().is_some() {
                state.focus_active_workspace(keyboard);
            }
        }
        ShortcutAction::CycleWindow { previous } => {
            state.super_used = true;
            let _ = state.cycle_active_window(keyboard, previous);
        }
        ShortcutAction::OpenLauncher => {
            state.super_used = true;
            state.send_shell_launcher_open();
        }
        ShortcutAction::OpenDefaultApp(app) => {
            state.super_used = true;
            state.send_shell_default_app_launch(app);
        }
        ShortcutAction::RestartShell => {
            state.super_used = true;
            state.request_shell_restart();
        }
        ShortcutAction::SuperPress => {
            state.super_active = true;
            state.super_used = false;
        }
        ShortcutAction::SuperRelease => {
            if !state.super_used {
                state.send_shell_start_menu_toggle();
            }
            state.super_active = false;
            state.super_used = false;
        }
    }
}

fn is_super_key(raw: u32) -> bool {
    matches!(raw, 0xffeb | 0xffec)
}

fn workspace_for_raw_key(raw: u32) -> Option<WorkspaceId> {
    match raw {
        0x31..=0x39 => Some(WorkspaceId(char::from_u32(raw)?.to_string())),
        _ => None,
    }
}
