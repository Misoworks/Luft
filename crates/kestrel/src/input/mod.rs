use crate::{
    layers,
    state::KestrelState,
    window::{ResizeEdge, WindowFrameControl, WindowFrameHit, WindowGrab},
};
use asher_ipc::WorkspaceId;
use smithay::{
    backend::input::{
        AbsolutePositionEvent, Axis, Event, InputBackend, InputEvent, KeyState, KeyboardKeyEvent,
        PointerAxisEvent, PointerButtonEvent, PointerMotionEvent,
    },
    input::{
        keyboard::{FilterResult, KeyboardHandle},
        pointer::{AxisFrame, ButtonEvent, CursorIcon, MotionEvent, PointerHandle},
    },
    utils::{Physical, Size},
};

const BTN_LEFT: u32 = 0x110;
const BTN_RIGHT: u32 = 0x111;

pub fn handle_input_event<B>(
    state: &mut KestrelState,
    keyboard: &KeyboardHandle<KestrelState>,
    pointer: &PointerHandle<KestrelState>,
    event: InputEvent<B>,
    output_size: Size<i32, Physical>,
) where
    B: InputBackend,
{
    match event {
        InputEvent::Keyboard { event } => {
            let serial = state.next_serial();
            let key_state = event.state();
            let mut shortcut = ShortcutAction::Forward;
            keyboard.input::<(), _>(
                state,
                event.key_code(),
                key_state,
                serial,
                event.time_msec(),
                |state, modifiers, key| {
                    shortcut = shortcut_for_key(
                        modifiers,
                        key.raw_latin_sym_or_raw_current_sym(),
                        key_state,
                    );
                    state.super_active = modifiers.logo;
                    if shortcut.is_forward() {
                        FilterResult::Forward
                    } else {
                        FilterResult::Intercept(())
                    }
                },
            );
            if key_state == KeyState::Pressed || matches!(shortcut, ShortcutAction::SuperRelease) {
                handle_shortcut(state, keyboard, shortcut);
            }
        }
        InputEvent::PointerMotionAbsolute { event } => {
            let frame_hover_before = frame_control_hover(state);
            let location = event.position_transformed(output_size.to_logical(1));
            move_pointer(state, pointer, location, event.time_msec());
            if frame_hover_before != frame_control_hover(state) {
                state.mark_scene_dirty();
            }
        }
        InputEvent::PointerMotion { event } => {
            let frame_hover_before = frame_control_hover(state);
            let delta = event.delta();
            let max = output_size.to_logical(1);
            let location = (
                (state.pointer_location.x + delta.x).clamp(0.0, max.w as f64),
                (state.pointer_location.y + delta.y).clamp(0.0, max.h as f64),
            )
                .into();
            move_pointer(state, pointer, location, event.time_msec());
            if frame_hover_before != frame_control_hover(state) {
                state.mark_scene_dirty();
            }
        }
        InputEvent::PointerButton { event } => {
            let serial = state.next_serial();
            let mut frame_interaction = false;
            let mut forward_button_release = false;
            let left_button = event.button_code() == BTN_LEFT;
            let right_button = event.button_code() == BTN_RIGHT;
            let closes_transient = event.state().pressed()
                && (left_button || right_button)
                && layers::should_close_transient_popover(state.output(), state.pointer_location);
            let hit = if closes_transient {
                state.window_at_below_shell(state.pointer_location)
            } else {
                state.window_at_for_shell_interaction(state.pointer_location)
            };

            if closes_transient {
                state.close_shell_transient_popovers();
            }
            if event.state().pressed() && (left_button || right_button) {
                if let Some(surface) = hit.clone() {
                    state.allow_client_grab(surface, serial);
                } else {
                    state.clear_client_grab();
                }
            }

            if state.super_active
                && event.state().pressed()
                && (left_button || right_button)
                && let Some(surface) = hit.clone()
            {
                frame_interaction = true;
                state.super_used = true;
                state.activate_surface(keyboard, &surface);
                if left_button {
                    state.begin_drag(surface);
                } else if let Some((surface, edge)) = if closes_transient {
                    state.modifier_resize_at_below_shell(state.pointer_location)
                } else {
                    state.modifier_resize_at(state.pointer_location)
                } {
                    state.begin_resize(surface, edge);
                }
            } else if left_button && event.state().pressed() {
                let frame_hit = if closes_transient {
                    state.window_frame_hit_below_shell(state.pointer_location)
                } else {
                    state.window_frame_hit(state.pointer_location)
                };
                if let Some(frame_hit) = frame_hit {
                    frame_interaction = true;
                    match frame_hit {
                        WindowFrameHit::Titlebar { surface } => {
                            state.activate_surface(keyboard, &surface);
                            state.begin_drag(surface);
                        }
                        WindowFrameHit::Resize { surface, edge } => {
                            state.activate_surface(keyboard, &surface);
                            state.begin_resize(surface, edge);
                        }
                        WindowFrameHit::Control { id, control } => {
                            let _ = state.handle_window_control(keyboard, id, control);
                        }
                    }
                } else if let Some(surface) = hit.clone() {
                    if !state.activate_surface(keyboard, &surface) {
                        keyboard.set_focus(state, Some(surface.wl_surface().clone()), serial);
                    }
                    let drag_surface = if closes_transient {
                        state.client_drag_surface_at_below_shell(state.pointer_location)
                    } else {
                        state.client_drag_surface_at(state.pointer_location)
                    };
                    if drag_surface.as_ref() == Some(&surface) {
                        state.prepare_window_drag(surface);
                    }
                    refresh_pointer_focus(state, pointer, serial, event.time_msec());
                } else {
                    keyboard.set_focus(state, state.keyboard_focus(state.pointer_location), serial);
                }
            }

            if (left_button || right_button) && !event.state().pressed() {
                forward_button_release = state.drag_forwards_button_release();
                frame_interaction |= state.drag.is_some() && !forward_button_release;
                state.end_drag();
                state.clear_client_grab();
            }

            update_frame_cursor(state);

            if !frame_interaction || forward_button_release {
                pointer.button(
                    state,
                    &ButtonEvent {
                        serial,
                        time: event.time_msec(),
                        button: event.button_code(),
                        state: event.state(),
                    },
                );
                pointer.frame(state);
            }
        }
        InputEvent::PointerAxis { event } => {
            if state.super_active {
                if let Some(offset) = axis_workspace_offset::<B>(&event) {
                    state.super_used = true;
                    let _ = state.switch_relative_workspace(keyboard, offset);
                    update_frame_cursor(state);
                }
                return;
            }

            let mut frame = AxisFrame::new(event.time_msec()).source(event.source());

            for axis in [Axis::Horizontal, Axis::Vertical] {
                if let Some(amount) = event.amount(axis) {
                    frame = frame.value(axis, amount);
                }
                if let Some(v120) = event.amount_v120(axis) {
                    frame = frame.v120(axis, v120.round() as i32);
                }
                frame = frame.relative_direction(axis, event.relative_direction(axis));
            }

            pointer.axis(state, frame);
            pointer.frame(state);
        }
        _ => {}
    }
}

fn refresh_pointer_focus(
    state: &mut KestrelState,
    pointer: &PointerHandle<KestrelState>,
    serial: smithay::utils::Serial,
    time: u32,
) {
    let location = state.pointer_location;
    let focus = state.pointer_focus(location);
    pointer.motion(
        state,
        focus,
        &MotionEvent {
            location,
            serial,
            time,
        },
    );
}

fn move_pointer(
    state: &mut KestrelState,
    pointer: &PointerHandle<KestrelState>,
    location: smithay::utils::Point<f64, smithay::utils::Logical>,
    time: u32,
) {
    state.pointer_location = location;
    state.update_drag(location);
    update_frame_cursor(state);

    let serial = state.next_serial();
    let focus = state.pointer_focus(location);
    pointer.motion(
        state,
        focus,
        &MotionEvent {
            location,
            serial,
            time,
        },
    );
    pointer.frame(state);
}

fn frame_control_hover(state: &KestrelState) -> Option<WindowFrameControl> {
    match state.window_frame_hit(state.pointer_location) {
        Some(WindowFrameHit::Control { control, .. }) => Some(control),
        _ => None,
    }
}

fn update_frame_cursor(state: &mut KestrelState) {
    if let Some(grab) = &state.drag {
        match grab {
            WindowGrab::Move { .. } => state.set_frame_cursor(CursorIcon::Grabbing),
            WindowGrab::Resize { edge, .. } => state.set_frame_cursor(resize_cursor(*edge)),
        }
        return;
    }

    match state.window_frame_hit(state.pointer_location) {
        Some(WindowFrameHit::Titlebar { .. }) => state.set_frame_cursor(CursorIcon::Grab),
        Some(WindowFrameHit::Control { .. }) => state.set_frame_cursor(CursorIcon::Pointer),
        Some(WindowFrameHit::Resize { edge, .. }) => state.set_frame_cursor(resize_cursor(edge)),
        None => state.clear_frame_cursor(),
    }
}

fn resize_cursor(edge: ResizeEdge) -> CursorIcon {
    match (edge.left, edge.right, edge.top, edge.bottom) {
        (true, _, true, _) => CursorIcon::NwResize,
        (_, true, true, _) => CursorIcon::NeResize,
        (true, _, _, true) => CursorIcon::SwResize,
        (_, true, _, true) => CursorIcon::SeResize,
        (true, _, _, _) => CursorIcon::WResize,
        (_, true, _, _) => CursorIcon::EResize,
        (_, _, true, _) => CursorIcon::NResize,
        (_, _, _, true) => CursorIcon::SResize,
        _ => CursorIcon::Default,
    }
}

#[derive(Debug)]
enum ShortcutAction {
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
    fn is_forward(&self) -> bool {
        matches!(self, Self::Forward)
    }
}

fn shortcut_for_key(
    modifiers: &smithay::input::keyboard::ModifiersState,
    key: Option<smithay::input::keyboard::Keysym>,
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

fn handle_shortcut(
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

fn axis_workspace_offset<B: InputBackend>(event: &B::PointerAxisEvent) -> Option<i32> {
    let amount = event
        .amount_v120(Axis::Vertical)
        .or_else(|| event.amount(Axis::Vertical))?;
    if amount > 0.0 {
        Some(1)
    } else if amount < 0.0 {
        Some(-1)
    } else {
        None
    }
}

fn workspace_for_raw_key(raw: u32) -> Option<WorkspaceId> {
    match raw {
        0x31..=0x39 => Some(WorkspaceId(char::from_u32(raw)?.to_string())),
        _ => None,
    }
}

trait ButtonStateExt {
    fn pressed(self) -> bool;
}

impl ButtonStateExt for smithay::backend::input::ButtonState {
    fn pressed(self) -> bool {
        matches!(self, Self::Pressed)
    }
}
