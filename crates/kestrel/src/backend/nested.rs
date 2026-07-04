use super::nested_timing::{host_refresh_millihertz, idle_wait, pace_frame, refresh_interval};
use crate::{
    background::Background,
    background_effect,
    client::ClientState,
    compositor_damage::{CompositorDamageContext, plan_compositor_damage},
    damage::{DamageTracker, LayerGeometryTracker, resolve_render_damage},
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
    window_clip::window_elements,
    xwayland::XwaylandSatellite,
};
use luft_config::LuftConfig;
use luft_ipc::{ShellStatus, shell_socket_path};
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
const PROCESS_CHECK_INTERVAL: Duration = Duration::from_millis(250);

pub struct NestedOptions {
    pub config: LuftConfig,
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
    let mut last_process_check = Instant::now() - PROCESS_CHECK_INTERVAL;
    let mut damage_tracker = DamageTracker::from_output(state.output());
    let mut blur_damage_tracker = DamageTracker::from_output(state.output());
    let mut layer_geometry = LayerGeometryTracker::default();
    let mut clients = Vec::new();
    let session_started = Instant::now();
    let mut blur_cache = SceneBlurCache::default();
    let mut shell_layers_seen_ready = false;
    let mut visible_popups = state.has_visible_popups();

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
        output.size.w,
        output.size.h,
    );
    state.shell_status = shell.status();
    info!(
        wayland_display = %socket_name,
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
        let process_check_due = last_process_check.elapsed() >= PROCESS_CHECK_INTERVAL
            || xwayland.restart_due(frame_started)
            || shell.restart_due(frame_started);
        if process_check_due {
            last_process_check = Instant::now();
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
        }
        let shell_restarted = state.take_shell_restart_requested();
        if shell_restarted {
            shell.restart();
        } else if process_check_due {
            shell.reap(&mut state.config);
        }
        if process_check_due || shell_restarted {
            let shell_status = shell.status();
            if state.shell_status != shell_status {
                state.shell_status = shell_status;
                if shell_status != ShellStatus::Running {
                    shell_layers_seen_ready = false;
                }
                state.mark_scene_dirty();
            }
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
        let show_loading = should_show_loading_overlay(shell_layers_ready, shell_layers_seen_ready);
        if shell_layers_ready {
            shell_layers_seen_ready = true;
        }
        let workspace_transition_active = state.workspace_transition().is_some();
        let scene_dirty = state.scene_dirty();
        let current_popups = state.has_visible_popups();
        let popup_visibility_changed =
            std::mem::replace(&mut visible_popups, current_popups) != current_popups;
        let layer_geometry_changed = layer_geometry
            .geometry_changed(output.size, &layers::layer_surface_rects(state.output()));
        let content_render_needed = force_full_damage
            || scene_dirty
            || popup_visibility_changed
            || layer_geometry_changed
            || removed_windows
            || finished_window_closes
            || state.animations_active()
            || workspace_transition_active
            || show_loading;
        if !content_render_needed {
            idle_wait();
            continue;
        }

        let buffer_age = backend.buffer_age().unwrap_or_default();
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
        let background_layer =
            render_stage_elements(renderer, &state, RenderStage::Layer(Layer::Background));
        let bottom_layer =
            render_stage_elements(renderer, &state, RenderStage::Layer(Layer::Bottom));
        let window_effect_targets = background_effect::window_blur_targets(&state);
        let mut blur_targets = window_effect_targets.clone();
        blur_targets.extend(top_targets.iter().cloned());
        blur_targets.extend(overlay_targets.iter().cloned());
        blur_cache.retain_targets(&blur_targets);
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
        let loading_overlay = if show_loading {
            Some(render_loading_overlay(
                renderer,
                output.size,
                loading_phase(session_started.elapsed()),
            )?)
        } else {
            None
        };
        let force_scene_full_damage =
            force_full_damage || workspace_transition_active;
        let mut plan_damage = |buffer_age: usize, force_full: bool| {
            plan_compositor_damage(
                CompositorDamageContext {
                    output_size: output.size,
                    output: state.output(),
                    buffer_age,
                    force_full_damage: force_full,
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
                },
                &mut damage_tracker,
                &mut blur_damage_tracker,
                &mut layer_geometry,
            )
        };
        let mut compositor_plan = plan_damage(buffer_age, force_scene_full_damage);
        let mut damage = resolve_render_damage(
            output.size,
            u32::try_from(buffer_age).unwrap_or(u32::MAX),
            force_scene_full_damage,
            compositor_plan.damage,
        );
        if damage.is_none() && scene_dirty {
            compositor_plan = plan_damage(0, false);
            damage = resolve_render_damage(output.size, 0, false, compositor_plan.damage);
        }
        let Some(damage) = damage else {
            idle_wait();
            continue;
        };
        let blur_damage = compositor_plan.blur_damage;
        state.take_scene_dirty();
        let submit_damage = damage.clone();
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
            },
        )?;
        drop(framebuffer);

        let frame_time = frame_clock.next_frame();
        for surface in state.frame_callback_surfaces() {
            send_surface_frame_tree(state.output(), &surface, frame_time);
        }

        backend.submit(Some(&submit_damage))?;
        pace_frame(frame_started, frame_interval);
    }
}

fn loading_phase(elapsed: Duration) -> f32 {
    let seconds = elapsed.as_secs_f32();
    (seconds * 0.72).fract()
}

fn bind_socket(socket_name: Option<&str>) -> Result<ListeningSocket, NestedError> {
    match socket_name {
        Some(name) => Ok(ListeningSocket::bind(name)?),
        None => Ok(ListeningSocket::bind_auto("luft", 1..33)?),
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
