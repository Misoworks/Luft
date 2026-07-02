use crate::state::KestrelState;
use smithay::{
    backend::input::{
        Event, GestureBeginEvent, GestureEndEvent, GesturePinchUpdateEvent as BackendPinchUpdate,
        GestureSwipeUpdateEvent as BackendSwipeUpdate, InputBackend,
    },
    input::pointer::{
        GestureHoldBeginEvent, GestureHoldEndEvent, GesturePinchBeginEvent, GesturePinchEndEvent,
        GesturePinchUpdateEvent, GestureSwipeBeginEvent, GestureSwipeEndEvent,
        GestureSwipeUpdateEvent, PointerHandle,
    },
};

pub fn swipe_begin<B: InputBackend>(
    state: &mut KestrelState,
    pointer: &PointerHandle<KestrelState>,
    event: B::GestureSwipeBeginEvent,
) {
    let serial = state.next_serial();
    pointer.gesture_swipe_begin(
        state,
        &GestureSwipeBeginEvent {
            serial,
            time: event.time_msec(),
            fingers: event.fingers(),
        },
    );
    pointer.frame(state);
}

pub fn swipe_update<B: InputBackend>(
    state: &mut KestrelState,
    pointer: &PointerHandle<KestrelState>,
    event: B::GestureSwipeUpdateEvent,
) {
    pointer.gesture_swipe_update(
        state,
        &GestureSwipeUpdateEvent {
            time: event.time_msec(),
            delta: event.delta(),
        },
    );
    pointer.frame(state);
}

pub fn swipe_end<B: InputBackend>(
    state: &mut KestrelState,
    pointer: &PointerHandle<KestrelState>,
    event: B::GestureSwipeEndEvent,
) {
    let serial = state.next_serial();
    pointer.gesture_swipe_end(
        state,
        &GestureSwipeEndEvent {
            serial,
            time: event.time_msec(),
            cancelled: event.cancelled(),
        },
    );
    pointer.frame(state);
}

pub fn pinch_begin<B: InputBackend>(
    state: &mut KestrelState,
    pointer: &PointerHandle<KestrelState>,
    event: B::GesturePinchBeginEvent,
) {
    let serial = state.next_serial();
    pointer.gesture_pinch_begin(
        state,
        &GesturePinchBeginEvent {
            serial,
            time: event.time_msec(),
            fingers: event.fingers(),
        },
    );
    pointer.frame(state);
}

pub fn pinch_update<B: InputBackend>(
    state: &mut KestrelState,
    pointer: &PointerHandle<KestrelState>,
    event: B::GesturePinchUpdateEvent,
) {
    pointer.gesture_pinch_update(
        state,
        &GesturePinchUpdateEvent {
            time: event.time_msec(),
            delta: event.delta(),
            scale: event.scale(),
            rotation: event.rotation(),
        },
    );
    pointer.frame(state);
}

pub fn pinch_end<B: InputBackend>(
    state: &mut KestrelState,
    pointer: &PointerHandle<KestrelState>,
    event: B::GesturePinchEndEvent,
) {
    let serial = state.next_serial();
    pointer.gesture_pinch_end(
        state,
        &GesturePinchEndEvent {
            serial,
            time: event.time_msec(),
            cancelled: event.cancelled(),
        },
    );
    pointer.frame(state);
}

pub fn hold_begin<B: InputBackend>(
    state: &mut KestrelState,
    pointer: &PointerHandle<KestrelState>,
    event: B::GestureHoldBeginEvent,
) {
    let serial = state.next_serial();
    pointer.gesture_hold_begin(
        state,
        &GestureHoldBeginEvent {
            serial,
            time: event.time_msec(),
            fingers: event.fingers(),
        },
    );
    pointer.frame(state);
}

pub fn hold_end<B: InputBackend>(
    state: &mut KestrelState,
    pointer: &PointerHandle<KestrelState>,
    event: B::GestureHoldEndEvent,
) {
    let serial = state.next_serial();
    pointer.gesture_hold_end(
        state,
        &GestureHoldEndEvent {
            serial,
            time: event.time_msec(),
            cancelled: event.cancelled(),
        },
    );
    pointer.frame(state);
}
