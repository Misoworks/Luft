use crate::output::DEFAULT_REFRESH_MILLIHERTZ;
use smithay::reexports::winit::window::Window as WinitWindow;
use std::{
    hint::spin_loop,
    thread,
    time::{Duration, Instant},
};

const HIGH_REFRESH_INTERVAL: Duration = Duration::from_millis(9);
const HIGH_REFRESH_SLEEP_GUARD: Duration = Duration::from_micros(1_500);
const NORMAL_REFRESH_SLEEP_GUARD: Duration = Duration::from_micros(900);
const HIGH_REFRESH_YIELD_GUARD: Duration = Duration::from_micros(220);
const NORMAL_REFRESH_YIELD_GUARD: Duration = Duration::from_micros(120);
const IDLE_WAIT: Duration = Duration::from_millis(2);

pub(super) fn pace_frame(started_at: Instant, frame_interval: Duration) {
    let target = started_at + frame_interval;
    let sleep_guard = if frame_interval <= HIGH_REFRESH_INTERVAL {
        HIGH_REFRESH_SLEEP_GUARD
    } else {
        NORMAL_REFRESH_SLEEP_GUARD
    };
    let yield_guard = if frame_interval <= HIGH_REFRESH_INTERVAL {
        HIGH_REFRESH_YIELD_GUARD
    } else {
        NORMAL_REFRESH_YIELD_GUARD
    };
    loop {
        let remaining = target.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        if remaining > sleep_guard {
            thread::sleep(remaining - sleep_guard);
            continue;
        }
        if remaining > yield_guard {
            thread::yield_now();
            continue;
        }
        while Instant::now() < target {
            spin_loop();
        }
        break;
    }
}

pub(super) fn idle_wait() {
    thread::sleep(IDLE_WAIT);
}

pub(super) fn host_refresh_millihertz(window: &WinitWindow) -> Option<i32> {
    window
        .current_monitor()
        .and_then(|monitor| monitor.refresh_rate_millihertz())
        .and_then(|refresh| i32::try_from(refresh).ok())
        .filter(|refresh| *refresh > 0)
}

pub(super) fn refresh_interval(refresh_millihertz: i32) -> Duration {
    let refresh = u64::try_from(refresh_millihertz)
        .ok()
        .filter(|refresh| *refresh > 0)
        .unwrap_or(DEFAULT_REFRESH_MILLIHERTZ as u64);
    Duration::from_nanos((1_000_000_000_000u64 + refresh / 2) / refresh)
}
