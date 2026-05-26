use smithay::{
    desktop::utils::{OutputPresentationFeedback, take_presentation_feedback_surface_tree},
    output::Output,
    reexports::{
        wayland_protocols::wp::presentation_time::server::wp_presentation_feedback,
        wayland_server::protocol::wl_surface::WlSurface,
    },
    utils::{Clock, Monotonic, Time},
    wayland::{
        compositor::{SurfaceAttributes, TraversalAction, with_surface_tree_downward},
        presentation::Refresh,
    },
};
use std::time::Duration;

#[derive(Debug)]
pub struct FrameClock {
    clock: Clock<Monotonic>,
    refresh: Refresh,
    sequence: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct FrameTime {
    time: Time<Monotonic>,
    refresh: Refresh,
    sequence: u64,
}

impl FrameClock {
    pub fn new(refresh: Duration) -> Self {
        Self {
            clock: Clock::new(),
            refresh: Refresh::fixed(refresh),
            sequence: 1,
        }
    }

    pub fn set_refresh(&mut self, refresh: Duration) {
        self.refresh = Refresh::fixed(refresh);
    }

    pub fn next_frame(&mut self) -> FrameTime {
        let frame = FrameTime {
            time: self.clock.now(),
            refresh: self.refresh,
            sequence: self.sequence,
        };
        self.sequence = self.sequence.wrapping_add(1).max(1);
        frame
    }
}

impl FrameTime {
    fn millis(self) -> u32 {
        self.time.as_millis()
    }
}

pub fn send_surface_frame_tree(output: &Output, surface: &WlSurface, frame: FrameTime) {
    let mut feedback = OutputPresentationFeedback::new(output);
    take_presentation_feedback_surface_tree(
        surface,
        &mut feedback,
        |_, _| Some(output.clone()),
        |_, _| wp_presentation_feedback::Kind::empty(),
    );
    feedback.presented(
        frame.time,
        frame.refresh,
        frame.sequence,
        wp_presentation_feedback::Kind::Vsync,
    );
    send_frame_callbacks(surface, frame.millis());
}

fn send_frame_callbacks(surface: &WlSurface, time: u32) {
    with_surface_tree_downward(
        surface,
        (),
        |_, _, &()| TraversalAction::DoChildren(()),
        |_surface, states, &()| {
            for callback in states
                .cached_state
                .get::<SurfaceAttributes>()
                .current()
                .frame_callbacks
                .drain(..)
            {
                callback.done(time);
            }
        },
        |_, _, &()| true,
    );
}
