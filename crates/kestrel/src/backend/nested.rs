use super::nested_timing::{
    host_refresh_millihertz, idle_wait, pace_frame, refresh_interval, target_hz,
};
use crate::{
    background::Background,
    background_effect,
    client::ClientState,
    compositor_damage::{CompositorDamageContext, CompositorDamagePlan, plan_compositor_damage},
    damage::{DamageTracker, LayerGeometryTracker},
    debug_overlay::{DebugOverlayCache, DebugOverlayStats, render_debug_overlay},
    frame_clock::{FrameClock, send_surface_frame_tree},
    input::handle_input_event,
    ipc::IpcServer,
    layers,
    loading_overlay::{render_loading_overlay, shell_layers_ready, should_show_loading_overlay},
    output::NestedOutput,
    render::{RenderStage, render_stage_elements, window_chrome_elements},
    scene_blur::SceneBlurCache,
    scene_render::{SceneRenderRequest, render_scene},
    session_services,
    shell::ShellProcess,
    state::KestrelState,
    submitted_damage::SubmittedDamageHistory,
    window_clip::window_elements,
    xwayland::XwaylandSatellite,
};
use asher_config::AsherConfig;
use asher_ipc::{ShellStatus, shell_socket_path};
use smithay::{
    backend::{
        renderer::gles::GlesRenderer,
        winit::{self, WinitEvent},
    },
    input::Seat,
    input::pointer::CursorImageStatus,
    reexports::{
        wayland_server::{Display, ListeningSocket},
        winit::platform::pump_events::PumpStatus,
    },
    utils::{Physical, Rectangle},
    wayland::shell::wlr_layer::Layer,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use thiserror::Error;
use tracing::{debug, info};

const REFRESH_CHECK_INTERVAL: Duration = Duration::from_millis(500);
const PENDING_REFRESH_CHECK_INTERVAL: Duration = Duration::from_millis(50);

pub struct NestedOptions {
    pub config: AsherConfig,
    pub socket_name: Option<String>,
}

pub fn run(options: NestedOptions) -> Result<(), NestedError> {
    let mut display: Display<KestrelState> = Display::new()?;
    let dh = display.handle();
    let mut state = KestrelState::new(&dh, options.config);
    let ipc = IpcServer::bind()?;
    let shell_control_socket = shell_socket_path(ipc.path());
    state.shell_control_path = Some(shell_control_socket.clone());
    let mut output = NestedOutput::default();
    let listener = bind_socket(options.socket_name.as_deref())?;
    let socket_name = listener
        .socket_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown")
        .to_string();

    let (mut backend, mut event_loop) = winit::init::<GlesRenderer>()?;
    backend.window().set_decorations(true);
    let mut host_refresh_known = match host_refresh_millihertz(backend.window()) {
        Some(refresh) => {
            output.set_refresh(refresh);
            true
        }
        None => false,
    };
    state.set_output_refresh(output.refresh_millihertz);
    let mut background = Background::new(state.config.compositor.background_image.clone());
    let keyboard = state.seat.add_keyboard(Default::default(), 200, 200)?;
    state.keyboard = Some(keyboard.clone());
    let pointer = state.seat.add_pointer();
    let mut frame_interval = refresh_interval(output.refresh_millihertz);
    let mut frame_clock = FrameClock::new(frame_interval);
    let mut last_refresh_check = Instant::now() - REFRESH_CHECK_INTERVAL;
    let mut damage_tracker = DamageTracker::from_output(state.output());
    let mut blur_damage_tracker = DamageTracker::from_output(state.output());
    let mut submitted_damage = SubmittedDamageHistory::default();
    let mut layer_geometry = LayerGeometryTracker::default();
    let mut clients = Vec::new();
    let session_started = Instant::now();
    let mut previous_frame_ms = 0.0f32;
    let mut previous_damage_area = 0i32;
    let mut fps = 0u32;
    let mut fps_frames = 0u32;
    let mut fps_started = Instant::now();
    let mut blur_cache = SceneBlurCache::default();
    let mut debug_overlay_cache = DebugOverlayCache::default();
    let mut shell_layers_seen_ready = false;

    println!("Kestrel nested compositor is running");
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
        output.refresh_millihertz,
    );
    state.shell_status = shell.status();
    info!(
        wayland_display = %socket_name,
        blur_enabled = state.config.general.enable_blur,
        ipc_socket = %ipc.path().display(),
        refresh_millihertz = output.refresh_millihertz,
        "nested compositor ready"
    );

    loop {
        let frame_started = Instant::now();
        let mut force_full_damage = false;
        let status = event_loop.dispatch_new_events(|event| match event {
            WinitEvent::Resized { size, .. } if output.resize(size) => {
                state.set_output_size(output.size);
                damage_tracker = DamageTracker::from_output(state.output());
                blur_damage_tracker = DamageTracker::from_output(state.output());
                submitted_damage.clear();
                force_full_damage = true;
            }
            WinitEvent::Input(event) => {
                handle_input_event(&mut state, &keyboard, &pointer, event, output.size);
            }
            WinitEvent::Focus(focused) => debug!(focused, "nested host focus changed"),
            WinitEvent::CloseRequested => {}
            _ => {}
        });

        if let PumpStatus::Exit(_) = status {
            return Ok(());
        }

        if output.resize(backend.window_size()) {
            state.set_output_size(output.size);
            damage_tracker = DamageTracker::from_output(state.output());
            blur_damage_tracker = DamageTracker::from_output(state.output());
            submitted_damage.clear();
            force_full_damage = true;
        }
        let refresh_check_interval = if host_refresh_known {
            REFRESH_CHECK_INTERVAL
        } else {
            PENDING_REFRESH_CHECK_INTERVAL
        };
        if last_refresh_check.elapsed() >= refresh_check_interval {
            last_refresh_check = Instant::now();
            if let Some(refresh) = host_refresh_millihertz(backend.window()) {
                host_refresh_known = true;
                if output.set_refresh(refresh) {
                    frame_interval = refresh_interval(output.refresh_millihertz);
                    frame_clock.set_refresh(frame_interval);
                    state.set_output_refresh(output.refresh_millihertz);
                    info!(
                        refresh_millihertz = output.refresh_millihertz,
                        "nested host refresh changed"
                    );
                }
            }
        }
        let removed_windows = state.remove_dead_windows();
        let finished_window_closes = state.send_finished_window_closes();
        state.cleanup_layers();
        state.cleanup_output();
        xwayland.reap(&socket_name);
        let xwayland_status = xwayland.status();
        if state.xwayland_status != xwayland_status {
            state.xwayland_status = xwayland_status;
            state.mark_scene_dirty();
        }
        let xwayland_display = xwayland.display().map(str::to_string);
        if state.xwayland_display != xwayland_display {
            state.xwayland_display = xwayland_display;
            session_services::sync_activation_environment(
                &socket_name,
                state.xwayland_display.as_deref(),
            );
            state.mark_scene_dirty();
        }
        match state.take_shell_restart_requested() {
            Some(crate::state::ShellRestartRequest::Normal) => shell.restart(),
            Some(crate::state::ShellRestartRequest::DefaultConfig) => {
                shell.restart_with_default_config()
            }
            None => shell.reap(&mut state.config),
        }
        let shell_status = shell.status();
        if state.shell_status != shell_status {
            state.shell_status = shell_status;
            if shell_status != ShellStatus::Running {
                shell_layers_seen_ready = false;
            }
            state.mark_scene_dirty();
        }
        if ipc.handle_pending(&mut state, &keyboard)? {
            state.mark_scene_dirty();
        }

        while let Some(stream) = listener.accept()? {
            let client = display
                .handle()
                .insert_client(stream, Arc::new(ClientState::default()))?;
            clients.push(client);
            debug!(connected_clients = clients.len(), "accepted wayland client");
        }

        display.dispatch_clients(&mut state)?;
        display.flush_clients()?;
        sync_host_cursor(&backend, &mut state);
        if background.set_path(state.config.compositor.background_image.clone()) {
            force_full_damage = true;
        }

        let shell_layers_ready = shell_layers_ready(state.output(), state.shell_status);
        if shell_layers_ready {
            shell_layers_seen_ready = true;
        }
        let show_loading = should_show_loading_overlay(shell_layers_ready, shell_layers_seen_ready);
        let scene_dirty = state.take_scene_dirty();
        let debug_needs_render =
            state.config.compositor.debug_overlay && debug_overlay_cache.needs_refresh();
        let layer_geometry_changed = layer_geometry
            .geometry_changed(output.size, &layers::layer_surface_rects(state.output()));
        let blur_animating = blur_cache.is_animating();
        let content_render_needed = force_full_damage
            || scene_dirty
            || layer_geometry_changed
            || removed_windows
            || finished_window_closes
            || state.animations_active()
            || blur_animating
            || show_loading;
        let render_needed = content_render_needed || debug_needs_render;
        if !render_needed {
            previous_damage_area = 0;
            idle_wait();
            continue;
        }

        let buffer_age = backend.buffer_age().unwrap_or_default();
        let mut rendered = false;
        let mut submit_damage: Vec<Rectangle<i32, Physical>> = Vec::new();
        {
            let (renderer, mut framebuffer) = backend.bind()?;
            let fullscreen_active = state
                .windows
                .fullscreen_on_workspace(state.layout.active_workspace())
                .is_some();
            let mut top_targets = if fullscreen_active {
                Vec::new()
            } else {
                layers::render_targets(state.output(), Layer::Top)
            };
            if !fullscreen_active {
                top_targets.extend(background_effect::layer_popup_blur_targets(
                    &state,
                    Layer::Top,
                ));
            }
            let mut overlay_targets = if fullscreen_active {
                Vec::new()
            } else {
                layers::render_targets(state.output(), Layer::Overlay)
            };
            if !fullscreen_active {
                overlay_targets.extend(background_effect::layer_popup_blur_targets(
                    &state,
                    Layer::Overlay,
                ));
            }
            let background_element = background.render_element(renderer, output.size)?;
            let blur_enabled = state.config.general.enable_blur && state.config.effects.blur;
            let background_layer =
                render_stage_elements(renderer, &state, RenderStage::Layer(Layer::Background));
            let bottom_layer =
                render_stage_elements(renderer, &state, RenderStage::Layer(Layer::Bottom));
            let window_effect_targets = background_effect::window_blur_targets(&state);
            if blur_enabled {
                let mut blur_targets = window_effect_targets.clone();
                blur_targets.extend(top_targets.iter().cloned());
                blur_targets.extend(overlay_targets.iter().cloned());
                blur_cache.retain_targets(&blur_targets);
            } else {
                blur_cache.clear();
            }
            let blur_animating = blur_cache.is_animating();
            let windows = window_elements(renderer, &state);
            let window_chrome = window_chrome_elements(renderer, &state)?;
            let top_layer = if fullscreen_active {
                Vec::new()
            } else {
                render_stage_elements(renderer, &state, RenderStage::Layer(Layer::Top))
            };
            let overlay_layer = if fullscreen_active {
                Vec::new()
            } else {
                render_stage_elements(renderer, &state, RenderStage::Layer(Layer::Overlay))
            };
            let blur_passes = if blur_enabled {
                window_effect_targets.len() + top_targets.len() + overlay_targets.len()
            } else {
                0
            };
            let loading_overlay = if show_loading {
                Some(render_loading_overlay(
                    renderer,
                    output.size,
                    loading_phase(session_started.elapsed()),
                )?)
            } else {
                None
            };
            let debug_overlay = if state.config.compositor.debug_overlay {
                let workspace = state.layout.active_workspace().0.as_str();
                let profile = state
                    .layout
                    .workspaces()
                    .find(|workspace| &workspace.id == state.layout.active_workspace())
                    .map(|workspace| workspace.profile_id.0.as_str())
                    .unwrap_or("-");
                let xwayland = state.xwayland_display.as_deref().unwrap_or("-");
                Some(render_debug_overlay(
                    &mut debug_overlay_cache,
                    renderer,
                    &DebugOverlayStats {
                        backend: "nested",
                        frame_ms: previous_frame_ms,
                        fps,
                        idle: !content_render_needed,
                        target_hz: target_hz(output.refresh_millihertz),
                        damage_area: previous_damage_area,
                        surfaces: state.windows.surfaces().len() + state.layer_surfaces().len(),
                        blur_passes,
                        workspace,
                        profile,
                        xwayland,
                    },
                )?)
            } else {
                debug_overlay_cache.clear();
                None
            };
            let CompositorDamagePlan {
                damage,
                blur_damage,
                damage_area: planned_damage_area,
                ..
            } = plan_compositor_damage(
                CompositorDamageContext {
                    output_size: output.size,
                    output: state.output(),
                    buffer_age,
                    force_full_damage: force_full_damage || blur_animating,
                    blur_enabled,
                    blur_animating,
                    window_effect_targets: &window_effect_targets,
                    top_targets: &top_targets,
                    overlay_targets: &overlay_targets,
                    background: background_element.as_ref(),
                    background_layer: &background_layer,
                    bottom_layer: &bottom_layer,
                    windows: &windows,
                    window_chrome: &window_chrome,
                    top_layer: &top_layer,
                    overlay_layer: &overlay_layer,
                    loading: loading_overlay.as_ref(),
                    debug: debug_overlay.as_ref(),
                },
                &mut damage_tracker,
                &mut blur_damage_tracker,
                &mut layer_geometry,
                &submitted_damage,
            );
            previous_damage_area = planned_damage_area;
            if !damage.is_empty() {
                submit_damage = damage.clone();
                render_scene(
                    &mut blur_cache,
                    renderer,
                    &mut framebuffer,
                    SceneRenderRequest {
                        state: &state,
                        output_size: output.size,
                        target_transform: state.output_transform(),
                        damage: &damage,
                        blur_damage: &blur_damage,
                        blur_enabled,
                        background: background_element,
                        background_layer: &background_layer,
                        bottom_layer: &bottom_layer,
                        windows: &windows,
                        window_chrome: &window_chrome,
                        window_targets: &window_effect_targets,
                        top_targets: &top_targets,
                        top_layer: &top_layer,
                        overlay_targets: &overlay_targets,
                        overlay_layer: &overlay_layer,
                        loading: loading_overlay,
                        debug: debug_overlay,
                    },
                )?;
                rendered = true;
            }
        }

        if rendered {
            let frame_time = frame_clock.next_frame();
            for surface in state
                .windows
                .surfaces()
                .into_iter()
                .chain(state.layer_surfaces())
            {
                send_surface_frame_tree(state.output(), &surface, frame_time);
            }

            backend.submit(Some(&submit_damage))?;
            submitted_damage.record(output.size, &submit_damage);
            fps_frames += 1;
            previous_frame_ms = frame_started.elapsed().as_secs_f32() * 1000.0;
            pace_frame(frame_started, frame_interval);
        } else {
            previous_frame_ms = 0.0;
            idle_wait();
        }

        if fps_started.elapsed() >= Duration::from_secs(1) {
            fps = fps_frames;
            fps_frames = 0;
            fps_started = Instant::now();
        }
    }
}

fn loading_phase(elapsed: Duration) -> f32 {
    let seconds = elapsed.as_secs_f32();
    (seconds * 0.72).fract()
}

fn bind_socket(socket_name: Option<&str>) -> Result<ListeningSocket, NestedError> {
    match socket_name {
        Some(name) => Ok(ListeningSocket::bind(name)?),
        None => Ok(ListeningSocket::bind_auto("asher", 1..33)?),
    }
}

fn sync_host_cursor(
    backend: &smithay::backend::winit::WinitGraphicsBackend<GlesRenderer>,
    state: &mut KestrelState,
) {
    if !state.cursor_dirty {
        return;
    }

    match &state.cursor_image {
        CursorImageStatus::Hidden => backend.window().set_cursor_visible(false),
        CursorImageStatus::Named(icon) => {
            backend.window().set_cursor_visible(true);
            backend.window().set_cursor(*icon);
        }
        CursorImageStatus::Surface(_) => {
            backend.window().set_cursor_visible(true);
        }
    }

    state.cursor_dirty = false;
}

#[derive(Debug, Error)]
pub enum NestedError {
    #[error("failed to create wayland display: {0}")]
    Display(#[from] smithay::reexports::wayland_server::backend::InitError),
    #[error("failed to initialize nested winit backend: {0}")]
    Winit(#[from] smithay::backend::winit::Error),
    #[error("failed to initialize keyboard seat: {0}")]
    Keyboard(#[from] smithay::input::keyboard::Error),
    #[error("failed to bind wayland socket: {0}")]
    Socket(#[from] smithay::reexports::wayland_server::BindError),
    #[error("failed to swap nested compositor buffer: {0}")]
    Swap(#[from] smithay::backend::SwapBuffersError),
    #[error("failed to render nested compositor frame: {0}")]
    Render(#[from] smithay::backend::renderer::gles::GlesError),
    #[error("nested compositor I/O failed: {0}")]
    Io(#[from] std::io::Error),
}

fn _keep_seat_type(_: &Seat<KestrelState>) {}
