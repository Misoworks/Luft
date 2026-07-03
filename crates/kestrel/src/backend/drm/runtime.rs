use super::{
    DrmError, DrmOptions,
    device::{self, SessionDevice},
    frame::{FrameResult, SessionFrameRenderer, render_secondary_output},
    scheduler::RenderScheduler,
};
use crate::{
    client::ClientState, frame_clock::send_surface_frame_tree, input::handle_input_event,
    ipc::IpcServer, session_services, shell::ShellProcess, state::KestrelState,
    xwayland::XwaylandSatellite,
};
use ::input::{
    Device as LibinputDevice, DeviceCapability as LibinputDeviceCapability, Led as LibinputLed,
};
use calloop::{
    EventLoop,
    signals::{Signal, Signals},
};
use luft_ipc::{ShellStatus, shell_socket_path};
use smithay::{
    backend::{
        drm::{DrmEvent, DrmEventMetadata},
        input::InputEvent,
        libinput::LibinputInputBackend,
        renderer::ImportDma,
        session::Event as SessionEvent,
        udev::UdevEvent,
    },
    reexports::{
        drm::control::crtc,
        wayland_server::{Client, Display, ListeningSocket, protocol::wl_surface::WlSurface},
    },
    utils::{Clock, Monotonic},
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tracing::{debug, info, warn};

mod process;
mod syncobj;
mod timing;

use process::process_timeout;
use syncobj::{clear_ready_syncobj_blockers, register_syncobj_sources};
use timing::{presentation_time, refresh_interval};

const IDLE_DISPATCH: Duration = Duration::from_millis(16);

pub fn run(options: DrmOptions) -> Result<(), DrmError> {
    let mut display: Display<KestrelState> = Display::new().map_err(|error| {
        DrmError::Unsupported(format!("failed to create Wayland display: {error}"))
    })?;
    let dh = display.handle();
    let opened = device::open(&dh, &options.config.display)?;
    let mut device = opened.device;
    let device::SessionSources {
        session_notifier,
        udev,
        drm_notifier,
        input,
    } = opened.sources;
    let mut state = KestrelState::new_for_outputs(&dh, options.config, device.descriptors());
    state.enable_dmabuf(
        device.dmabuf_main_device(),
        device.renderer.dmabuf_formats(),
    );
    state.enable_drm_syncobj(device.drm_device_fd());
    let ipc = IpcServer::bind()
        .map_err(|error| DrmError::Unsupported(format!("failed to bind IPC socket: {error}")))?;
    let shell_control_socket = shell_socket_path(ipc.path());
    state.shell_control_path = Some(shell_control_socket.clone());
    let listener = bind_socket(options.socket_name.as_deref())?;
    let socket_name = listener
        .socket_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown")
        .to_string();
    let keyboard = state
        .seat
        .add_keyboard(Default::default(), 200, 200)
        .map_err(|error| {
            DrmError::Unsupported(format!("failed to initialize keyboard seat: {error}"))
        })?;
    state.keyboard = Some(keyboard.clone());
    let pointer = state.seat.add_pointer();
    let mut keyboard_devices = Vec::new();
    let mut keyboard_led_state = keyboard.led_state();
    let mut frame_renderer =
        SessionFrameRenderer::new(&state, refresh_interval(state.output_refresh_millihertz()));
    let mut render_scheduler =
        RenderScheduler::new(refresh_interval(state.output_refresh_millihertz()));
    let presentation_clock = Clock::<Monotonic>::new();
    let mut clients = Vec::new();
    let mut force_full_damage = true;
    let mut pending_frame_callbacks: Option<Vec<WlSurface>> = None;
    let mut active = true;
    let mut loop_events = LoopEvents::default();
    let mut event_loop = EventLoop::<LoopEvents>::try_new().map_err(|error| {
        DrmError::Unsupported(format!("failed to create session event loop: {error}"))
    })?;

    event_loop
        .handle()
        .insert_source(input, |event, _, data| data.input.push(event))
        .map_err(|error| {
            DrmError::Unsupported(format!("failed to register libinput source: {error}"))
        })?;
    event_loop
        .handle()
        .insert_source(drm_notifier, |event, metadata, data| match event {
            DrmEvent::VBlank(crtc) => data.vblank.push(VBlankEvent {
                crtc,
                metadata: metadata.take(),
            }),
            DrmEvent::Error(error) => data.drm_errors.push(error.to_string()),
        })
        .map_err(|error| {
            DrmError::Unsupported(format!("failed to register DRM source: {error}"))
        })?;
    event_loop
        .handle()
        .insert_source(udev, |event, _, data| data.udev.push(event))
        .map_err(|error| {
            DrmError::Unsupported(format!("failed to register udev source: {error}"))
        })?;
    event_loop
        .handle()
        .insert_source(session_notifier, |event, _, data| data.session.push(event))
        .map_err(|error| {
            DrmError::Unsupported(format!("failed to register libseat source: {error}"))
        })?;
    event_loop
        .handle()
        .insert_source(
            Signals::new(&[Signal::SIGCHLD]).map_err(|error| {
                DrmError::Unsupported(format!("failed to create SIGCHLD source: {error}"))
            })?,
            |event, _, data| {
                if event.signal() == Signal::SIGCHLD {
                    data.child_process_changed = true;
                }
            },
        )
        .map_err(|error| {
            DrmError::Unsupported(format!("failed to register SIGCHLD source: {error}"))
        })?;

    println!("Kestrel session compositor is running");
    println!("WAYLAND_DISPLAY={socket_name}");
    let mut xwayland = XwaylandSatellite::start(state.config.compositor.xwayland, &socket_name);
    state.xwayland_status = xwayland.status();
    state.xwayland_display = xwayland.display().map(str::to_string);
    if let Some(display) = &state.xwayland_display {
        println!("DISPLAY={display}");
    }
    session_services::start(&socket_name, state.xwayland_display.as_deref());
    let mut shell = ShellProcess::start(
        &socket_name,
        state.xwayland_display.as_deref(),
        ipc.path(),
        &shell_control_socket,
        state.output_refresh_millihertz(),
        state.output_size().w,
        state.output_size().h,
    );
    state.shell_status = shell.status();
    info!(
        wayland_display = %socket_name,
        ipc_socket = %ipc.path().display(),
        refresh_millihertz = state.output_refresh_millihertz(),
        outputs = device.descriptors().len(),
        "DRM session compositor ready"
    );

    loop {
        event_loop
            .dispatch(Some(Duration::ZERO), &mut loop_events)
            .map_err(|error| {
                DrmError::Unsupported(format!("session event dispatch failed: {error}"))
            })?;
        handle_session_events(
            &mut loop_events,
            &mut device,
            &mut active,
            &mut force_full_damage,
        )?;
        if !active {
            pending_frame_callbacks = None;
        }
        clear_ready_syncobj_blockers(&mut loop_events, &mut state, &dh);
        for event in loop_events.udev.drain(..) {
            if !device.handles_udev_event(&event) {
                continue;
            }
            match event {
                UdevEvent::Changed { .. } => {
                    if device.rescan_outputs(&state.config.display)? {
                        state.set_output_descriptors(device.descriptors());
                        frame_renderer.reset_for_output(&state);
                        render_scheduler.set_refresh_interval(refresh_interval(
                            state.output_refresh_millihertz(),
                        ));
                        pending_frame_callbacks = None;
                        force_full_damage = true;
                        let descriptor = device.primary_descriptor();
                        info!(
                            output = %descriptor.name,
                            width = descriptor.size.w,
                            height = descriptor.size.h,
                            outputs = device.descriptors().len(),
                            "DRM output graph changed"
                        );
                    }
                }
                UdevEvent::Removed { .. } => {
                    return Err(DrmError::Unsupported(
                        "active DRM device was removed".to_string(),
                    ));
                }
                UdevEvent::Added { .. } => {}
            }
        }
        for error in loop_events.drm_errors.drain(..) {
            warn!(%error, "DRM event error");
        }
        for vblank in loop_events.vblank.drain(..) {
            let primary = device.is_primary_crtc(vblank.crtc);
            device.frame_submitted(vblank.crtc)?;
            if primary {
                let (presentation, presentation_instant) =
                    presentation_time(&presentation_clock, vblank.metadata);
                render_scheduler.frame_presented(presentation_instant);
                if let Some(callback_surfaces) = pending_frame_callbacks.take() {
                    let frame_time = frame_renderer.frame_presented(presentation);
                    for surface in callback_surfaces {
                        send_surface_frame_tree(state.output(), &surface, frame_time);
                    }
                }
            }
        }
        for event in loop_events.input.drain(..) {
            if active {
                match event {
                    InputEvent::DeviceAdded { device } => {
                        register_keyboard_device(&mut keyboard_devices, device, keyboard_led_state);
                    }
                    InputEvent::DeviceRemoved { device } => {
                        unregister_keyboard_device(&mut keyboard_devices, &device);
                    }
                    event => {
                        let output_size = state.output_size();
                        handle_input_event(&mut state, &keyboard, &pointer, event, output_size);
                        if let Some(led_state) = state.take_pending_keyboard_led_state() {
                            keyboard_led_state = led_state;
                            update_keyboard_leds(&mut keyboard_devices, keyboard_led_state);
                        }
                    }
                }
            }
        }

        let process_changed = loop_events.take_child_process_changed();
        let now = Instant::now();
        if process_changed || xwayland.restart_due(now) {
            xwayland.reap(&socket_name);
            update_xwayland_state(&mut state, &xwayland, &socket_name);
        }
        if state.take_shell_restart_requested() {
            shell.restart();
        } else if process_changed || shell.restart_due(now) {
            shell.reap(&mut state.config);
        }
        let shell_status = shell.status();
        if state.shell_status != shell_status {
            state.shell_status = shell_status;
            if shell_status != ShellStatus::Running {
                frame_renderer.mark_shell_not_ready();
            }
            state.mark_scene_dirty();
        }
        if ipc
            .handle_pending(&mut state, &keyboard)
            .map_err(|error| DrmError::Unsupported(format!("IPC handling failed: {error}")))?
        {
            state.mark_scene_dirty();
        }

        while let Some(stream) = listener.accept().map_err(|error| {
            DrmError::Unsupported(format!("failed to accept Wayland client: {error}"))
        })? {
            let client = display
                .handle()
                .insert_client(stream, Arc::new(ClientState::default()))
                .map_err(|error| {
                    DrmError::Unsupported(format!("failed to insert Wayland client: {error}"))
                })?;
            clients.push(client);
            debug!(connected_clients = clients.len(), "accepted wayland client");
        }

        display
            .dispatch_clients(&mut state)
            .map_err(|error| DrmError::Unsupported(format!("Wayland dispatch failed: {error}")))?;
        register_syncobj_sources(&mut state, &event_loop)?;
        display
            .flush_clients()
            .map_err(|error| DrmError::Unsupported(format!("Wayland flush failed: {error}")))?;
        if active {
            device.sync_cursor(&mut state);
        }

        if active
            && !device.frame_pending()
            && (force_full_damage
                || state.scene_dirty()
                || state.animations_active()
                || frame_renderer.effects_active())
        {
            render_scheduler.request_repaint(Instant::now());
        }

        if active && !device.frame_pending() && render_scheduler.should_render(Instant::now()) {
            let render_started = Instant::now();
            let frame_result = {
                let (renderer, output) = device.renderer_and_primary_output();
                let frame_result = frame_renderer.render(
                    &mut state,
                    renderer,
                    &mut output.surface,
                    &mut output.direct_scanout,
                    force_full_damage,
                )?;
                if let FrameResult::Queued { submitted, .. } = &frame_result {
                    output.mark_frame_submitted(*submitted);
                }
                frame_result
            };
            if force_full_damage {
                let (renderer, primary, outputs) = device.renderer_and_outputs();
                for (index, output) in outputs.iter_mut().enumerate() {
                    if index != primary {
                        let _ = render_secondary_output(renderer, output)?;
                    }
                }
            }
            match frame_result {
                FrameResult::Queued {
                    submitted: _,
                    callback_surfaces,
                } => {
                    render_scheduler.frame_rendered(render_started.elapsed());
                    pending_frame_callbacks = Some(callback_surfaces);
                    force_full_damage = false;
                    display.flush_clients().map_err(|error| {
                        DrmError::Unsupported(format!("Wayland flush failed after frame: {error}"))
                    })?;
                }
                FrameResult::Idle => {
                    render_scheduler.cancel_repaint();
                    let timeout = process_timeout(Instant::now(), IDLE_DISPATCH, &shell, &xwayland);
                    event_loop
                        .dispatch(Some(timeout), &mut loop_events)
                        .map_err(|error| {
                            DrmError::Unsupported(format!("session idle dispatch failed: {error}"))
                        })?;
                }
            }
        } else if active && !device.frame_pending() {
            let now = Instant::now();
            let timeout = render_scheduler.dispatch_timeout(now, IDLE_DISPATCH);
            let timeout = process_timeout(now, timeout, &shell, &xwayland);
            event_loop
                .dispatch(Some(timeout), &mut loop_events)
                .map_err(|error| {
                    DrmError::Unsupported(format!("session scheduled dispatch failed: {error}"))
                })?;
        } else if active {
            event_loop
                .dispatch(None, &mut loop_events)
                .map_err(|error| {
                    DrmError::Unsupported(format!("session frame-pending dispatch failed: {error}"))
                })?;
        } else {
            let timeout = process_timeout(Instant::now(), IDLE_DISPATCH, &shell, &xwayland);
            event_loop
                .dispatch(Some(timeout), &mut loop_events)
                .map_err(|error| {
                    DrmError::Unsupported(format!("paused session dispatch failed: {error}"))
                })?;
        }
    }
}

#[derive(Default)]
struct LoopEvents {
    input: Vec<InputEvent<LibinputInputBackend>>,
    vblank: Vec<VBlankEvent>,
    drm_errors: Vec<String>,
    session: Vec<SessionEvent>,
    udev: Vec<UdevEvent>,
    syncobj_ready: Vec<Client>,
    child_process_changed: bool,
}

struct VBlankEvent {
    crtc: crtc::Handle,
    metadata: Option<DrmEventMetadata>,
}

impl LoopEvents {
    fn take_child_process_changed(&mut self) -> bool {
        std::mem::take(&mut self.child_process_changed)
    }
}

fn register_keyboard_device(
    devices: &mut Vec<LibinputDevice>,
    mut device: LibinputDevice,
    led_state: smithay::input::keyboard::LedState,
) {
    if !device.has_capability(LibinputDeviceCapability::Keyboard) {
        return;
    }
    device.led_update(LibinputLed::from(led_state));
    devices.push(device);
}

fn unregister_keyboard_device(devices: &mut Vec<LibinputDevice>, device: &LibinputDevice) {
    devices.retain(|current| current.sysname() != device.sysname());
}

fn update_keyboard_leds(
    devices: &mut [LibinputDevice],
    led_state: smithay::input::keyboard::LedState,
) {
    let leds = LibinputLed::from(led_state);
    for device in devices {
        device.led_update(leds);
    }
}

fn handle_session_events(
    events: &mut LoopEvents,
    device: &mut SessionDevice,
    active: &mut bool,
    force_full_damage: &mut bool,
) -> Result<(), DrmError> {
    for event in events.session.drain(..) {
        match event {
            SessionEvent::PauseSession => {
                *active = false;
                device.pause();
                info!("paused DRM session");
            }
            SessionEvent::ActivateSession => {
                device.activate()?;
                *active = true;
                *force_full_damage = true;
                info!("reactivated DRM session");
            }
        }
    }

    Ok(())
}

fn update_xwayland_state(
    state: &mut KestrelState,
    xwayland: &XwaylandSatellite,
    socket_name: &str,
) {
    let xwayland_status = xwayland.status();
    if state.xwayland_status != xwayland_status {
        state.xwayland_status = xwayland_status;
        state.mark_scene_dirty();
    }
    let xwayland_display = xwayland.display().map(str::to_string);
    if state.xwayland_display != xwayland_display {
        state.xwayland_display = xwayland_display;
        session_services::sync_activation_environment(
            socket_name,
            state.xwayland_display.as_deref(),
        );
        state.mark_scene_dirty();
    }
}

fn bind_socket(socket_name: Option<&str>) -> Result<ListeningSocket, DrmError> {
    match socket_name {
        Some(name) => ListeningSocket::bind(name),
        None => ListeningSocket::bind_auto("luft", 1..33),
    }
    .map_err(|error| DrmError::Unsupported(format!("failed to bind Wayland socket: {error}")))
}
