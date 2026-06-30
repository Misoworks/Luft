use super::{
    DrmError, DrmOptions,
    device::{self, SessionDevice},
    frame::{FrameResult, SessionFrameRenderer, render_secondary_output},
};
use crate::{
    client::ClientState,
    frame_clock::send_surface_frame_tree,
    input::handle_input_event,
    ipc::IpcServer,
    output::DEFAULT_REFRESH_MILLIHERTZ,
    session_services,
    shell::ShellProcess,
    state::{KestrelState, ShellRestartRequest},
    xwayland::XwaylandSatellite,
};
use asher_ipc::{ShellStatus, shell_socket_path};
use smithay::{
    backend::{
        drm::DrmEvent, input::InputEvent, libinput::LibinputInputBackend, renderer::ImportDma,
        session::Event as SessionEvent, udev::UdevEvent,
    },
    reexports::{
        calloop::EventLoop,
        drm::control::crtc,
        wayland_server::{Display, ListeningSocket},
    },
};
use std::{sync::Arc, time::Duration};
use tracing::{debug, info, warn};

const IDLE_DISPATCH: Duration = Duration::from_millis(4);

pub fn run(options: DrmOptions) -> Result<(), DrmError> {
    let mut display: Display<KestrelState> = Display::new().map_err(|error| {
        DrmError::Unsupported(format!("failed to create Wayland display: {error}"))
    })?;
    let dh = display.handle();
    let opened = device::open(&dh)?;
    let mut device = opened.device;
    let device::SessionSources {
        session_notifier,
        udev,
        drm_notifier,
        input,
    } = opened.sources;
    let mut state = KestrelState::new_for_outputs(&dh, options.config, device.descriptors());
    state.enable_dmabuf(device.renderer.dmabuf_formats());
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
    let mut frame_renderer =
        SessionFrameRenderer::new(&state, refresh_interval(state.output_refresh_millihertz()));
    let mut clients = Vec::new();
    let mut force_full_damage = true;
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
        .insert_source(drm_notifier, |event, _, data| match event {
            DrmEvent::VBlank(crtc) => data.vblank.push(crtc),
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
    );
    state.shell_status = shell.status();
    info!(
        wayland_display = %socket_name,
        ipc_socket = %ipc.path().display(),
        blur_enabled = state.config.general.enable_blur,
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
        for event in loop_events.udev.drain(..) {
            if !device.handles_udev_event(&event) {
                continue;
            }
            match event {
                UdevEvent::Changed { .. } => {
                    if device.rescan_outputs()? {
                        state.set_output_descriptors(device.descriptors());
                        frame_renderer.reset_for_output(&state);
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
        for crtc in loop_events.vblank.drain(..) {
            device.frame_submitted(crtc)?;
        }
        for event in loop_events.input.drain(..) {
            if active {
                let output_size = state.output_size();
                handle_input_event(&mut state, &keyboard, &pointer, event, output_size);
            }
        }

        xwayland.reap(&socket_name);
        update_xwayland_state(&mut state, &xwayland, &socket_name);
        match state.take_shell_restart_requested() {
            Some(ShellRestartRequest::Normal) => shell.restart(),
            Some(ShellRestartRequest::DefaultConfig) => shell.restart_with_default_config(),
            None => shell.reap(&mut state.config),
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
        display
            .flush_clients()
            .map_err(|error| DrmError::Unsupported(format!("Wayland flush failed: {error}")))?;
        if active {
            device.sync_cursor(&mut state);
        }

        if active && !device.direct_scanout_pending() {
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
                    frame_time,
                    submitted: _,
                } => {
                    force_full_damage = false;
                    for surface in state
                        .windows
                        .surfaces()
                        .into_iter()
                        .chain(state.layer_surfaces())
                    {
                        send_surface_frame_tree(state.output(), &surface, frame_time);
                    }
                    display.flush_clients().map_err(|error| {
                        DrmError::Unsupported(format!("Wayland flush failed after frame: {error}"))
                    })?;
                }
                FrameResult::Idle => {
                    event_loop
                        .dispatch(Some(IDLE_DISPATCH), &mut loop_events)
                        .map_err(|error| {
                            DrmError::Unsupported(format!("session idle dispatch failed: {error}"))
                        })?;
                }
            }
        } else if active {
            event_loop
                .dispatch(Some(IDLE_DISPATCH), &mut loop_events)
                .map_err(|error| {
                    DrmError::Unsupported(format!(
                        "session direct-scanout dispatch failed: {error}"
                    ))
                })?;
        } else {
            event_loop
                .dispatch(Some(IDLE_DISPATCH), &mut loop_events)
                .map_err(|error| {
                    DrmError::Unsupported(format!("paused session dispatch failed: {error}"))
                })?;
        }
    }
}

#[derive(Default)]
struct LoopEvents {
    input: Vec<InputEvent<LibinputInputBackend>>,
    vblank: Vec<crtc::Handle>,
    drm_errors: Vec<String>,
    session: Vec<SessionEvent>,
    udev: Vec<UdevEvent>,
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
        None => ListeningSocket::bind_auto("asher", 1..33),
    }
    .map_err(|error| DrmError::Unsupported(format!("failed to bind Wayland socket: {error}")))
}

fn refresh_interval(refresh_millihertz: i32) -> Duration {
    let refresh = u64::try_from(refresh_millihertz)
        .ok()
        .filter(|refresh| *refresh > 0)
        .unwrap_or(DEFAULT_REFRESH_MILLIHERTZ as u64);
    Duration::from_nanos((1_000_000_000_000u64 + refresh / 2) / refresh)
}
