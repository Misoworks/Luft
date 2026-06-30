use super::{
    DrmError,
    device::{SessionOutput, SessionSurface, SubmittedFrame},
    scanout::DirectScanout,
};
use crate::{
    background::Background,
    background_effect,
    compositor_damage::{CompositorDamageContext, CompositorDamagePlan, plan_compositor_damage},
    damage::{DamageTracker, LayerGeometryTracker},
    debug_overlay::{DebugOverlayCache, DebugOverlayStats, render_debug_overlay},
    frame_clock::FrameClock,
    frame_clock::FrameTime,
    layers,
    loading_overlay::{render_loading_overlay, shell_layers_ready, should_show_loading_overlay},
    render::{LayerElement, RenderStage, render_stage_elements, window_chrome_elements},
    scene_blur::SceneBlurCache,
    scene_render::{SceneRenderRequest, render_scene},
    state::KestrelState,
    submitted_damage::SubmittedDamageHistory,
    window_clip::window_elements,
};
use smithay::{
    backend::renderer::{Bind, Color32F, Frame, Renderer, gles::GlesRenderer},
    utils::{Physical, Rectangle, Transform},
    wayland::shell::wlr_layer::Layer,
};
use std::time::{Duration, Instant};

pub enum FrameResult {
    Idle,
    Queued {
        frame_time: FrameTime,
        submitted: SubmittedFrame,
    },
}

pub fn render_secondary_output(
    renderer: &mut GlesRenderer,
    output: &mut SessionOutput,
) -> Result<bool, DrmError> {
    if output.has_pending_frame() {
        return Ok(false);
    }

    let size = output.descriptor.size;
    let damage = vec![Rectangle::<i32, Physical>::from_size(size)];
    let (mut dmabuf, _) = output.surface.next_buffer().map_err(|error| {
        DrmError::Unsupported(format!("failed to acquire secondary GBM buffer: {error}"))
    })?;
    let mut framebuffer = renderer.bind(&mut dmabuf).map_err(|error| {
        DrmError::Unsupported(format!("failed to bind secondary GBM buffer: {error}"))
    })?;
    let mut frame = renderer
        .render(&mut framebuffer, size, Transform::Normal)
        .map_err(render_error)?;
    frame
        .clear(Color32F::new(0.08, 0.085, 0.09, 1.0), &damage)
        .map_err(render_error)?;
    drop(frame);
    drop(framebuffer);
    output
        .surface
        .queue_buffer(None, Some(damage), ())
        .map_err(|error| {
            DrmError::Unsupported(format!("failed to queue secondary DRM frame: {error}"))
        })?;
    output.mark_frame_submitted(SubmittedFrame::Composited);
    Ok(true)
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
    previous_frame_direct: bool,
    submitted_damage: SubmittedDamageHistory,
    layer_geometry: LayerGeometryTracker,
    last_blur_passes: usize,
}

impl SessionFrameRenderer {
    pub fn new(state: &KestrelState, frame_interval: Duration) -> Self {
        Self {
            background: Background::new(state.config.compositor.background_image.clone()),
            frame_clock: FrameClock::new(frame_interval),
            damage_tracker: DamageTracker::new(state.output_size(), Transform::Normal),
            blur_damage_tracker: DamageTracker::new(state.output_size(), Transform::Normal),
            blur_cache: SceneBlurCache::default(),
            debug_overlay_cache: DebugOverlayCache::default(),
            session_started: Instant::now(),
            previous_frame_ms: 0.0,
            previous_damage_area: 0,
            fps: 0,
            fps_frames: 0,
            fps_started: Instant::now(),
            shell_layers_seen_ready: false,
            previous_frame_direct: false,
            submitted_damage: SubmittedDamageHistory::default(),
            layer_geometry: LayerGeometryTracker::default(),
            last_blur_passes: 0,
        }
    }

    pub fn render(
        &mut self,
        state: &mut KestrelState,
        renderer: &mut GlesRenderer,
        surface: &mut SessionSurface,
        direct_scanout: &mut DirectScanout,
        force_full_damage: bool,
    ) -> Result<FrameResult, DrmError> {
        let frame_started = Instant::now();
        let removed_windows = state.remove_dead_windows();
        let finished_window_closes = state.send_finished_window_closes();
        state.cleanup_layers();
        state.cleanup_output();
        let shell_layers_ready = shell_layers_ready(state.output(), state.shell_status);
        if shell_layers_ready {
            self.shell_layers_seen_ready = true;
        }
        let show_loading =
            should_show_loading_overlay(shell_layers_ready, self.shell_layers_seen_ready);
        let scene_dirty = state.take_scene_dirty();
        let debug_needs_render =
            state.config.compositor.debug_overlay && self.debug_overlay_cache.needs_refresh();
        let layer_geometry_changed = self.layer_geometry.geometry_changed(
            state.output_size(),
            &layers::layer_surface_rects(state.output()),
        );
        let blur_animating = self.blur_cache.is_animating();
        let content_render_needed = force_full_damage
            || self.previous_frame_direct
            || scene_dirty
            || layer_geometry_changed
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
                state,
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
                state,
                Layer::Overlay,
            ));
        }
        let background_element = self
            .background
            .render_element(renderer, state.output_size())
            .map_err(render_error)?;
        let blur_enabled = state.config.general.enable_blur && state.config.effects.blur;
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
                    state.output_size(),
                    loading_phase(self.session_started.elapsed()),
                )
                .map_err(render_error)?,
            )
        } else {
            None
        };
        self.last_blur_passes = if blur_enabled {
            window_effect_targets.len() + top_targets.len() + overlay_targets.len()
        } else {
            0
        };
        if can_direct_scanout(
            state,
            fullscreen_active,
            show_loading,
            debug_needs_render,
            blur_animating,
            &background_layer,
            &bottom_layer,
            &top_layer,
            &overlay_layer,
            &window_effect_targets,
        ) && direct_scanout.try_queue(state, renderer, surface)?
        {
            self.previous_damage_area = state.output_size().w.saturating_mul(state.output_size().h);
            self.previous_frame_ms = frame_started.elapsed().as_secs_f32() * 1000.0;
            self.fps_frames += 1;
            self.previous_frame_direct = true;
            self.submitted_damage.clear();
            self.update_fps();
            return Ok(FrameResult::Queued {
                frame_time: self.frame_clock.next_frame(),
                submitted: SubmittedFrame::Direct,
            });
        }

        let (mut dmabuf, buffer_age) = surface.next_buffer().map_err(|error| {
            DrmError::Unsupported(format!("failed to acquire GBM buffer: {error}"))
        })?;
        let mut framebuffer = renderer.bind(&mut dmabuf).map_err(|error| {
            DrmError::Unsupported(format!("failed to bind GBM buffer: {error}"))
        })?;
        let debug_overlay = self.debug_overlay(state, renderer, content_render_needed)?;
        let CompositorDamagePlan {
            damage,
            blur_damage,
            damage_area: planned_damage_area,
            ..
        } = plan_compositor_damage(
            CompositorDamageContext {
                output_size: state.output_size(),
                output: state.output(),
                buffer_age: usize::from(buffer_age),
                force_full_damage: force_full_damage
                    || blur_animating
                    || self.previous_frame_direct,
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
            &mut self.damage_tracker,
            &mut self.blur_damage_tracker,
            &mut self.layer_geometry,
            &self.submitted_damage,
        );
        self.previous_damage_area = planned_damage_area;

        if !damage.is_empty() {
            let output_size = state.output_size();
            render_scene(
                &mut self.blur_cache,
                renderer,
                &mut framebuffer,
                SceneRenderRequest {
                    state,
                    output_size,
                    target_transform: Transform::Normal,
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
                .queue_buffer(None, Some(damage.clone()), ())
                .map_err(|error| {
                    DrmError::Unsupported(format!("failed to queue DRM frame: {error}"))
                })?;
            self.submitted_damage.record(state.output_size(), &damage);
            self.fps_frames += 1;
            self.previous_frame_ms = frame_started.elapsed().as_secs_f32() * 1000.0;
            self.previous_frame_direct = false;
            self.update_fps();
            return Ok(FrameResult::Queued {
                frame_time: self.frame_clock.next_frame(),
                submitted: SubmittedFrame::Composited,
            });
        }

        Ok(FrameResult::Idle)
    }

    pub fn mark_shell_not_ready(&mut self) {
        self.shell_layers_seen_ready = false;
    }

    pub fn reset_for_output(&mut self, state: &KestrelState) {
        let frame_interval = refresh_interval(state.output_refresh_millihertz());
        self.frame_clock.set_refresh(frame_interval);
        self.damage_tracker = DamageTracker::new(state.output_size(), Transform::Normal);
        self.blur_damage_tracker = DamageTracker::new(state.output_size(), Transform::Normal);
        self.blur_cache.clear();
        self.debug_overlay_cache.clear();
        self.submitted_damage.clear();
        self.previous_damage_area = 0;
        self.previous_frame_ms = 0.0;
        self.previous_frame_direct = false;
    }

    fn debug_overlay(
        &mut self,
        state: &KestrelState,
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
                target_hz: super::super::nested_timing::target_hz(
                    state.output_refresh_millihertz(),
                ),
                damage_area: self.previous_damage_area,
                surfaces: state.windows.surfaces().len() + state.layer_surfaces().len(),
                blur_passes: self.last_blur_passes,
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

#[allow(clippy::too_many_arguments)]
fn can_direct_scanout(
    state: &KestrelState,
    fullscreen_active: bool,
    show_loading: bool,
    debug_needs_render: bool,
    blur_animating: bool,
    background_layer: &[LayerElement],
    bottom_layer: &[LayerElement],
    top_layer: &[LayerElement],
    overlay_layer: &[LayerElement],
    window_effect_targets: &[layers::LayerRenderTarget],
) -> bool {
    fullscreen_active
        && !show_loading
        && !debug_needs_render
        && !blur_animating
        && !state.animations_active()
        && state.workspace_transition().is_none()
        && background_layer.is_empty()
        && bottom_layer.is_empty()
        && top_layer.is_empty()
        && overlay_layer.is_empty()
        && window_effect_targets.is_empty()
}

fn refresh_interval(refresh_millihertz: i32) -> Duration {
    let refresh = u64::try_from(refresh_millihertz)
        .ok()
        .filter(|refresh| *refresh > 0)
        .unwrap_or(crate::output::DEFAULT_REFRESH_MILLIHERTZ as u64);
    Duration::from_nanos((1_000_000_000_000u64 + refresh / 2) / refresh)
}
