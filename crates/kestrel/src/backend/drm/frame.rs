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
    frame_clock::FrameClock,
    frame_clock::FrameTime,
    layers,
    loading_overlay::{render_loading_overlay, shell_layers_ready, should_show_loading_overlay},
    render::{LayerElement, RenderStage, render_stage_elements, window_chrome_elements},
    scene_blur::SceneBlurCache,
    scene_render::{SceneRenderRequest, render_scene},
    state::KestrelState,
    window_clip::window_elements,
};
use smithay::{
    backend::renderer::{Bind, Color32F, Frame, Renderer, gles::GlesRenderer},
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Monotonic, Physical, Rectangle, Time, Transform},
    wayland::shell::wlr_layer::Layer,
};
use std::time::{Duration, Instant};

pub enum FrameResult {
    Idle,
    Queued {
        submitted: SubmittedFrame,
        callback_surfaces: Vec<WlSurface>,
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
    session_started: Instant,
    shell_layers_seen_ready: bool,
    previous_frame_direct: bool,
    visible_popups: bool,
    layer_geometry: LayerGeometryTracker,
}

impl SessionFrameRenderer {
    pub fn new(state: &KestrelState, frame_interval: Duration) -> Self {
        Self {
            background: Background::new(state.config.compositor.background_image.clone()),
            frame_clock: FrameClock::new(frame_interval),
            damage_tracker: DamageTracker::from_output(state.output()),
            blur_damage_tracker: DamageTracker::from_output(state.output()),
            blur_cache: SceneBlurCache::default(),
            session_started: Instant::now(),
            shell_layers_seen_ready: false,
            previous_frame_direct: false,
            visible_popups: state.has_visible_popups(),
            layer_geometry: LayerGeometryTracker::default(),
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
        let removed_windows = state.remove_dead_windows();
        let finished_window_closes = state.send_finished_window_closes();
        state.cleanup_layers();
        state.cleanup_output();
        let shell_layers_ready = shell_layers_ready(state.output(), state.shell_status);
        let show_loading =
            should_show_loading_overlay(shell_layers_ready, self.shell_layers_seen_ready);
        if shell_layers_ready {
            self.shell_layers_seen_ready = true;
        }
        let workspace_transition_active = state.workspace_transition().is_some();
        let scene_dirty = state.take_scene_dirty();
        let visible_popups = state.has_visible_popups();
        let popup_visibility_changed =
            std::mem::replace(&mut self.visible_popups, visible_popups) != visible_popups;
        let layer_geometry_changed = self.layer_geometry.geometry_changed(
            state.output_size(),
            &layers::layer_surface_rects(state.output()),
        );
        let content_render_needed = force_full_damage
            || self.previous_frame_direct
            || scene_dirty
            || popup_visibility_changed
            || layer_geometry_changed
            || removed_windows
            || finished_window_closes
            || state.animations_active()
            || workspace_transition_active
            || show_loading
            || self
                .background
                .set_path(state.config.compositor.background_image.clone());
        if !content_render_needed {
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
        let window_effect_targets = background_effect::window_blur_targets(state);
        let mut blur_targets = window_effect_targets.clone();
        blur_targets.extend(top_targets.iter().cloned());
        blur_targets.extend(overlay_targets.iter().cloned());
        self.blur_cache.retain_targets(&blur_targets);
        let blur_animating = self.blur_cache.is_animating();
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
        if can_direct_scanout(
            state,
            fullscreen_active,
            show_loading,
            blur_animating,
            &top_layer,
            &overlay_layer,
            &window_effect_targets,
        ) && direct_scanout.try_queue(state, renderer, surface)?
        {
            self.previous_frame_direct = true;
            return Ok(FrameResult::Queued {
                submitted: SubmittedFrame::Direct,
                callback_surfaces: state.frame_callback_surfaces(),
            });
        }

        let background_element = self
            .background
            .render_element(renderer, state.output_size())
            .map_err(render_error)?;
        let background_layer =
            render_stage_elements(renderer, state, RenderStage::Layer(Layer::Background));
        let bottom_layer =
            render_stage_elements(renderer, state, RenderStage::Layer(Layer::Bottom));
        let windows = window_elements(renderer, state);
        let window_chrome = window_chrome_elements(renderer, state).map_err(render_error)?;

        let (mut dmabuf, buffer_age) = surface.next_buffer().map_err(|error| {
            DrmError::Unsupported(format!("failed to acquire GBM buffer: {error}"))
        })?;
        let mut framebuffer = renderer.bind(&mut dmabuf).map_err(|error| {
            DrmError::Unsupported(format!("failed to bind GBM buffer: {error}"))
        })?;
        let force_scene_full_damage = force_full_damage
            || self.previous_frame_direct
            || show_loading
            || workspace_transition_active
            || layer_geometry_changed;
        let CompositorDamagePlan {
            damage,
            blur_damage,
            ..
        } = plan_compositor_damage(
            CompositorDamageContext {
                output_size: state.output_size(),
                output: state.output(),
                buffer_age: usize::from(buffer_age),
                force_full_damage: force_scene_full_damage,
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
            &mut self.damage_tracker,
            &mut self.blur_damage_tracker,
            &mut self.layer_geometry,
        );

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
            )
            .map_err(render_error)?;
            drop(framebuffer);
            surface
                .queue_buffer(None, Some(damage.clone()), ())
                .map_err(|error| {
                    DrmError::Unsupported(format!("failed to queue DRM frame: {error}"))
                })?;
            self.previous_frame_direct = false;
            return Ok(FrameResult::Queued {
                submitted: SubmittedFrame::Composited,
                callback_surfaces: state.frame_callback_surfaces(),
            });
        }

        Ok(FrameResult::Idle)
    }

    pub fn mark_shell_not_ready(&mut self) {
        self.shell_layers_seen_ready = false;
    }

    pub fn frame_presented(&mut self, presentation: Option<(Time<Monotonic>, u64)>) -> FrameTime {
        match presentation {
            Some((time, sequence)) => self.frame_clock.frame_at_sequence(time, sequence),
            None => self.frame_clock.next_frame(),
        }
    }

    pub fn reset_for_output(&mut self, state: &KestrelState) {
        let frame_interval = refresh_interval(state.output_refresh_millihertz());
        self.frame_clock.set_refresh(frame_interval);
        self.damage_tracker = DamageTracker::from_output(state.output());
        self.blur_damage_tracker = DamageTracker::from_output(state.output());
        self.blur_cache.retain_targets(&[]);
        self.previous_frame_direct = false;
        self.visible_popups = state.has_visible_popups();
    }

    pub fn effects_active(&self) -> bool {
        self.blur_cache.is_animating()
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
    blur_animating: bool,
    top_layer: &[LayerElement],
    overlay_layer: &[LayerElement],
    window_effect_targets: &[layers::LayerRenderTarget],
) -> bool {
    fullscreen_active
        && !show_loading
        && !blur_animating
        && !state.has_visible_popups()
        && !state.animations_active()
        && state.workspace_transition().is_none()
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
