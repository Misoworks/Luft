use super::{
    DrmError, DrmOptions,
    device::{self, SessionDevice},
    frame::{FrameResult, SessionFrameRenderer},
};
use crate::{
    client::ClientState,
    frame_clock::send_surface_frame_tree,
    input::handle_input_event,
    ipc::IpcServer,
    output::DEFAULT_REFRESH_MILLIHERTZ,
    recovery::RecoveryPolicy,
    session_services,
    shell::ShellProcess,
    state::{BatonState, ShellRestartRequest},
    xwayland::XwaylandSatellite,
};
use smithay::{
    backend::{
        drm::DrmEvent, input::InputEvent, libinput::LibinputInputBackend,
        session::Event as SessionEvent,
    },
    reexports::{
        calloop::EventLoop,
        drm::control::crtc,
        wayland_server::{Display, ListeningSocket},
    },
};
use staccato_ipc::{ShellStatus, shell_socket_path};
use std::{sync::Arc, time::Duration};
use tracing::{debug, info, warn};

const IDLE_DISPATCH: Duration = Duration::from_millis(4);

pub fn run(options: DrmOptions) -> Result<(), DrmError> {
    let mut display: Display<BatonState> = Display::new().map_err(|error| {
        DrmError::Unsupported(format!("failed to create Wayland display: {error}"))
    })?;
    let dh = display.handle();
    let device = device::open(&dh)?;
    let SessionDevice {
        session: _session,
        session_notifier,
        mut drm,
        drm_notifier,
        mut surface,
        mut renderer,
        input,
        descriptor,
    } = device;
    let mut state = BatonState::new_for_output(&dh, options.config, descriptor);
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
    let recovery = RecoveryPolicy::new(
        state.config.recovery.crash_limit as usize,
        Duration::from_secs(state.config.recovery.crash_window_seconds),
    );
    let keyboard = state
        .seat
        .add_keyboard(Default::default(), 200, 200)
        .map_err(|error| {
            DrmError::Unsupported(format!("failed to initialize keyboard seat: {error}"))
        })?;
    state.keyboard = Some(keyboard.clone());
    let pointer = state.seat.add_pointer();
    let mut frame_renderer =
        SessionFrameRenderer::new(&state, refresh_interval(state.output_refresh_millihertz));
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
        .insert_source(session_notifier, |event, _, data| data.session.push(event))
        .map_err(|error| {
            DrmError::Unsupported(format!("failed to register libseat source: {error}"))
        })?;

    println!("Baton session compositor is running");
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
        recovery.clone(),
    );
    state.shell_status = shell.status();
    info!(
        wayland_display = %socket_name,
        ipc_socket = %ipc.path().display(),
        safe_mode = state.config.general.safe_mode,
        blur_enabled = state.config.general.enable_blur,
        refresh_millihertz = state.output_refresh_millihertz,
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
            &mut drm,
            &mut surface,
            &mut active,
            &mut force_full_damage,
        )?;
        for error in loop_events.drm_errors.drain(..) {
            warn!(%error, "DRM event error");
        }
        for crtc in loop_events.vblank.drain(..) {
            if crtc == surface.crtc() {
                let _ = surface.frame_submitted().map_err(|error| {
                    DrmError::Unsupported(format!("failed to retire submitted DRM frame: {error}"))
                })?;
            }
        }
        for event in loop_events.input.drain(..) {
            if active {
                let output_size = state.output_size;
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
            match frame_renderer.render(
                &mut state,
                &mut renderer,
                &mut surface,
                force_full_damage,
            )? {
                FrameResult::Queued { frame_time } => {
                    force_full_damage = false;
                    for surface in state
                        .windows
                        .surfaces()
                        .into_iter()
                        .chain(state.layer_surfaces())
                    {
                        send_surface_frame_tree(&state.output, &surface, frame_time);
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
}

fn handle_session_events(
    events: &mut LoopEvents,
    drm: &mut smithay::backend::drm::DrmDevice,
    surface: &mut super::device::SessionSurface,
    active: &mut bool,
    force_full_damage: &mut bool,
) -> Result<(), DrmError> {
    for event in events.session.drain(..) {
        match event {
            SessionEvent::PauseSession => {
                *active = false;
                drm.pause();
                info!("paused DRM session");
            }
            SessionEvent::ActivateSession => {
                drm.activate(true).map_err(|error| {
                    DrmError::Unsupported(format!("failed to reactivate DRM device: {error}"))
                })?;
                surface.surface().reset_state().map_err(|error| {
                    DrmError::Unsupported(format!("failed to reset DRM surface: {error}"))
                })?;
                surface.reset_buffer_ages();
                *active = true;
                *force_full_damage = true;
                info!("reactivated DRM session");
            }
        }
    }

    Ok(())
}

fn update_xwayland_state(state: &mut BatonState, xwayland: &XwaylandSatellite, socket_name: &str) {
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
        None => ListeningSocket::bind_auto("staccato", 1..33),
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
