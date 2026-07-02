use crate::{
    layers,
    state::KestrelState,
    window::{ResizeEdge, WindowFrameControl, WindowFrameHit, WindowGrab},
};
use smithay::{
    backend::input::{
        AbsolutePositionEvent, Axis, Event, InputBackend, InputEvent, KeyState, KeyboardKeyEvent,
        PointerAxisEvent, PointerButtonEvent, PointerMotionEvent,
    },
    input::{
        keyboard::{FilterResult, KeyboardHandle},
        pointer::{
            AxisFrame, ButtonEvent, CursorIcon, MotionEvent, PointerHandle, RelativeMotionEvent,
        },
    },
    utils::{Physical, Size},
    wayland::keyboard_shortcuts_inhibit::KeyboardShortcutsInhibitorSeat,
};

mod gestures;
mod shortcuts;

use shortcuts::{ShortcutAction, handle_shortcut, shortcut_for_key};

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
            let shortcuts_inhibited = shortcuts_inhibited(state, keyboard);
            let mut hotkey_consumed = false;
            keyboard.input::<(), _>(
                state,
                event.key_code(),
                key_state,
                serial,
                event.time_msec(),
                |state, modifiers, key| {
                    let key = key.raw_latin_sym_or_raw_current_sym();
                    if state.handle_vicinae_hotkey(
                        key,
                        modifiers,
                        key_state,
                        serial,
                        event.time_msec(),
                    ) {
                        hotkey_consumed = true;
                        if modifiers.logo {
                            state.super_used = true;
                        }
                        return FilterResult::Intercept(());
                    }

                    if shortcuts_inhibited {
                        state.super_active = false;
                        return FilterResult::Forward;
                    }

                    shortcut = shortcut_for_key(modifiers, key, key_state);
                    state.super_active = modifiers.logo;
                    if shortcut.is_forward() {
                        FilterResult::Forward
                    } else {
                        FilterResult::Intercept(())
                    }
                },
            );
            if !hotkey_consumed
                && (key_state == KeyState::Pressed
                    || matches!(shortcut, ShortcutAction::SuperRelease))
            {
                handle_shortcut(state, keyboard, shortcut);
            }
        }
        InputEvent::PointerMotionAbsolute { event } => {
            let frame_hover_before = frame_control_hover(state);
            let location = event.position_transformed(output_size.to_logical(1));
            move_pointer(state, pointer, location, event.time_msec(), None);
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
            move_pointer(
                state,
                pointer,
                location,
                event.time_msec(),
                Some(RelativeMotion {
                    delta,
                    delta_unaccel: event.delta_unaccel(),
                    utime: event.time(),
                }),
            );
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
            let button_pressed = button_pressed(event.state());
            let closes_transient = button_pressed
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
            if button_pressed && (left_button || right_button) {
                if let Some(surface) = hit.clone() {
                    state.allow_client_grab(surface, serial);
                } else {
                    state.clear_client_grab();
                }
            }

            if state.super_active
                && button_pressed
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
            } else if left_button && button_pressed {
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

            if (left_button || right_button) && !button_pressed {
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
        InputEvent::GestureSwipeBegin { event } => {
            gestures::swipe_begin::<B>(state, pointer, event)
        }
        InputEvent::GestureSwipeUpdate { event } => {
            gestures::swipe_update::<B>(state, pointer, event)
        }
        InputEvent::GestureSwipeEnd { event } => gestures::swipe_end::<B>(state, pointer, event),
        InputEvent::GesturePinchBegin { event } => {
            gestures::pinch_begin::<B>(state, pointer, event)
        }
        InputEvent::GesturePinchUpdate { event } => {
            gestures::pinch_update::<B>(state, pointer, event)
        }
        InputEvent::GesturePinchEnd { event } => gestures::pinch_end::<B>(state, pointer, event),
        InputEvent::GestureHoldBegin { event } => gestures::hold_begin::<B>(state, pointer, event),
        InputEvent::GestureHoldEnd { event } => gestures::hold_end::<B>(state, pointer, event),
        _ => {}
    }
}

struct RelativeMotion {
    delta: smithay::utils::Point<f64, smithay::utils::Logical>,
    delta_unaccel: smithay::utils::Point<f64, smithay::utils::Logical>,
    utime: u64,
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
    relative: Option<RelativeMotion>,
) {
    state.pointer_location = location;
    state.update_drag(location);
    update_frame_cursor(state);

    let serial = state.next_serial();
    let focus = state.pointer_focus(location);
    pointer.motion(
        state,
        focus.clone(),
        &MotionEvent {
            location,
            serial,
            time,
        },
    );
    if let Some(relative) = relative {
        pointer.relative_motion(
            state,
            focus,
            &RelativeMotionEvent {
                delta: relative.delta,
                delta_unaccel: relative.delta_unaccel,
                utime: relative.utime,
            },
        );
    }
    pointer.frame(state);
}

fn shortcuts_inhibited(state: &KestrelState, keyboard: &KeyboardHandle<KestrelState>) -> bool {
    keyboard
        .current_focus()
        .and_then(|surface| {
            state
                .seat
                .keyboard_shortcuts_inhibitor_for_surface(&surface)
        })
        .is_some_and(|inhibitor| inhibitor.is_active())
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

fn button_pressed(state: smithay::backend::input::ButtonState) -> bool {
    matches!(state, smithay::backend::input::ButtonState::Pressed)
}
