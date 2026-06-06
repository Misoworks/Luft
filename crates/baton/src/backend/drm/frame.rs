use super::{DrmError, device::SessionSurface};
use crate::{
    background::Background,
    background_effect,
    damage::{DamageTracker, blur_damage_elements, damage_area, damage_elements},
    debug_overlay::{DebugOverlayCache, DebugOverlayStats, render_debug_overlay},
    frame_clock::FrameClock,
    frame_clock::FrameTime,
    layers,
    loading_overlay::render_loading_overlay,
    render::{RenderStage, render_stage_elements, window_chrome_elements},
    scene_blur::SceneBlurCache,
    scene_render::{SceneRenderRequest, render_scene},
    state::BatonState,
    window_clip::window_elements,
};
use smithay::{
    backend::renderer::{Bind, gles::GlesRenderer},
    wayland::shell::wlr_layer::Layer,
};
use std::time::{Duration, Instant};

pub enum FrameResult {
    Idle,
    Queued { frame_time: FrameTime },
}

pub struct SessionFrameRenderer {
    background: Background,
    frame_clock: FrameClock,
    damage_tracker: DamageTracker,
    blur_damage_tracker: DamageTracker,
    blur_cache: SceneBlurCache,
    debug_overlay_cache: DebugOverlayCache,
    session_started: Instant,
    previous_frame_ms: f32,
    previous_damage_area: i32,
    fps: u32,
    fps_frames: u32,
    fps_started: Instant,
    shell_layers_seen_ready: bool,
}

impl SessionFrameRenderer {
    pub fn new(state: &BatonState, frame_interval: Duration) -> Self {
        Self {
            background: Background::new(state.config.compositor.background_image.clone()),
            frame_clock: FrameClock::new(frame_interval),
            damage_tracker: DamageTracker::new(state.output_size),
            blur_damage_tracker: DamageTracker::new(state.output_size),
            blur_cache: SceneBlurCache::default(),
            debug_overlay_cache: DebugOverlayCache::default(),
            session_started: Instant::now(),
            previous_frame_ms: 0.0,
            previous_damage_area: 0,
            fps: 0,
            fps_frames: 0,
            fps_started: Instant::now(),
            shell_layers_seen_ready: false,
        }
    }

    pub fn render(
        &mut self,
        state: &mut BatonState,
        renderer: &mut GlesRenderer,
        surface: &mut SessionSurface,
        force_full_damage: bool,
    ) -> Result<FrameResult, DrmError> {
        let frame_started = Instant::now();
        let removed_windows = state.remove_dead_windows();
        let finished_window_closes = state.send_finished_window_closes();
        state.cleanup_layers();
        state.cleanup_output();
        let shell_layers_ready = layers::has_shell_surface(&state.output);
        if shell_layers_ready {
            self.shell_layers_seen_ready = true;
        }
        let show_loading = !shell_layers_ready
            && (state.shell_status != staccato_ipc::ShellStatus::Running
                || !self.shell_layers_seen_ready);
        let scene_dirty = state.take_scene_dirty();
        let debug_needs_render =
            state.config.compositor.debug_overlay && self.debug_overlay_cache.needs_refresh();
        let blur_animating = self.blur_cache.is_animating();
        let content_render_needed = force_full_damage
            || scene_dirty
            || removed_windows
            || finished_window_closes
            || state.animations_active()
            || blur_animating
            || show_loading
            || self
                .background
                .set_path(state.config.compositor.background_image.clone());
        let render_needed = content_render_needed || debug_needs_render;
        if !render_needed {
            self.previous_damage_area = 0;
            self.previous_frame_ms = 0.0;
            return Ok(FrameResult::Idle);
        }

        let (mut dmabuf, buffer_age) = surface.next_buffer().map_err(|error| {
            DrmError::Unsupported(format!("failed to acquire GBM buffer: {error}"))
        })?;
        let mut framebuffer = renderer.bind(&mut dmabuf).map_err(|error| {
            DrmError::Unsupported(format!("failed to bind GBM buffer: {error}"))
        })?;

        let fullscreen_active = state
            .windows
            .fullscreen_on_workspace(state.layout.active_workspace())
            .is_some();
        let panel_taskbar = state.layout.active_mode() == staccato_layout::ModeId::Panel;
        let top_targets = if fullscreen_active {
            Vec::new()
        } else {
            layers::render_targets(&state.output, Layer::Top, panel_taskbar)
        };
        let overlay_targets = if fullscreen_active {
            Vec::new()
        } else {
            layers::render_targets(&state.output, Layer::Overlay, panel_taskbar)
        };
        let background_element = if show_loading {
            self.background
                .blurred_render_element(renderer, state.output_size)
                .map_err(render_error)?
        } else {
            self.background
                .render_element(renderer, state.output_size)
                .map_err(render_error)?
        };
        let blur_enabled = state.config.general.enable_blur
            && state.config.effects.blur
            && !state.config.general.safe_mode;
        let background_layer =
            render_stage_elements(renderer, state, RenderStage::Layer(Layer::Background));
        let bottom_layer =
            render_stage_elements(renderer, state, RenderStage::Layer(Layer::Bottom));
        let window_effect_targets = background_effect::window_blur_targets(state);
        if blur_enabled {
            let mut blur_targets = window_effect_targets.clone();
            blur_targets.extend(top_targets.iter().cloned());
            blur_targets.extend(overlay_targets.iter().cloned());
            self.blur_cache.retain_targets(&blur_targets);
        } else {
            self.blur_cache.clear();
        }
        let blur_animating = self.blur_cache.is_animating();
        let windows = window_elements(renderer, state);
        let window_chrome = window_chrome_elements(renderer, state).map_err(render_error)?;
        let top_layer = if fullscreen_active {
            Vec::new()
        } else {
            render_stage_elements(renderer, state, RenderStage::Layer(Layer::Top))
        };
        let overlay_layer = if fullscreen_active {
            Vec::new()
        } else {
            render_stage_elements(renderer, state, RenderStage::Layer(Layer::Overlay))
        };
        let loading_overlay = if show_loading {
            Some(
                render_loading_overlay(
                    renderer,
                    state.output_size,
                    loading_phase(self.session_started.elapsed()),
                )
                .map_err(render_error)?,
            )
        } else {
            None
        };
        let debug_overlay = self.debug_overlay(state, renderer, content_render_needed)?;
        let damage_plan = {
            let damage_elements = damage_elements(
                background_element.as_ref(),
                &background_layer,
                &bottom_layer,
                &windows,
                &window_chrome,
                &top_layer,
                &overlay_layer,
                loading_overlay.as_ref(),
                debug_overlay.as_ref(),
            );
            self.damage_tracker.plan(
                state.output_size,
                usize::from(buffer_age),
                force_full_damage || blur_animating,
                &damage_elements,
            )
        };
        let blur_damage_plan = {
            let blur_damage_elements = blur_damage_elements(
                background_element.as_ref(),
                &background_layer,
                &bottom_layer,
                &windows,
            );
            self.blur_damage_tracker.plan(
                state.output_size,
                usize::from(buffer_age),
                force_full_damage || blur_animating,
                &blur_damage_elements,
            )
        };
        let damage = damage_plan.rectangles.clone();
        let blur_damage = blur_damage_plan.rectangles.clone();
        self.previous_damage_area = damage_area(&damage);

        if !damage.is_empty() {
            let output_size = state.output_size;
            render_scene(
                &mut self.blur_cache,
                renderer,
                &mut framebuffer,
                SceneRenderRequest {
                    state,
                    output_size,
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
            )
            .map_err(render_error)?;
            drop(framebuffer);
            surface
                .queue_buffer(None, Some(damage), ())
                .map_err(|error| {
                    DrmError::Unsupported(format!("failed to queue DRM frame: {error}"))
                })?;
            self.damage_tracker.record(damage_plan);
            self.blur_damage_tracker.record(blur_damage_plan);
            self.fps_frames += 1;
            self.previous_frame_ms = frame_started.elapsed().as_secs_f32() * 1000.0;
            self.update_fps();
            return Ok(FrameResult::Queued {
                frame_time: self.frame_clock.next_frame(),
            });
        }

        Ok(FrameResult::Idle)
    }

    pub fn mark_shell_not_ready(&mut self) {
        self.shell_layers_seen_ready = false;
    }

    fn debug_overlay(
        &mut self,
        state: &BatonState,
        renderer: &mut GlesRenderer,
        content_render_needed: bool,
    ) -> Result<
        Option<
            smithay::backend::renderer::element::memory::MemoryRenderBufferRenderElement<
                GlesRenderer,
            >,
        >,
        DrmError,
    > {
        if !state.config.compositor.debug_overlay {
            self.debug_overlay_cache.clear();
            return Ok(None);
        }

        let workspace = state.layout.active_workspace().0.as_str();
        let profile = state
            .layout
            .workspaces()
            .find(|workspace| &workspace.id == state.layout.active_workspace())
            .map(|workspace| workspace.profile_id.0.as_str())
            .unwrap_or("-");
        let xwayland = state.xwayland_display.as_deref().unwrap_or("-");
        render_debug_overlay(
            &mut self.debug_overlay_cache,
            renderer,
            &DebugOverlayStats {
                backend: "session",
                frame_ms: self.previous_frame_ms,
                fps: self.fps,
                idle: !content_render_needed,
                target_hz: super::super::nested_timing::target_hz(state.output_refresh_millihertz),
                damage_area: self.previous_damage_area,
                surfaces: state.windows.surfaces().len() + state.layer_surfaces().len(),
                blur_passes: 0,
                workspace,
                profile,
                xwayland,
            },
        )
        .map(Some)
        .map_err(render_error)
    }

    fn update_fps(&mut self) {
        if self.fps_started.elapsed() < Duration::from_secs(1) {
            return;
        }
        self.fps = self.fps_frames;
        self.fps_frames = 0;
        self.fps_started = Instant::now();
    }
}

fn loading_phase(elapsed: Duration) -> f32 {
    (elapsed.as_secs_f32() * 0.72).fract()
}

fn render_error(error: impl std::fmt::Display) -> DrmError {
    DrmError::Unsupported(format!("failed to render DRM frame: {error}"))
}
