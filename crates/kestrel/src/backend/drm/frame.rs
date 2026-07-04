use super::{
    DrmError,
    device::{SessionCompositor, SessionOutput},
};
use crate::{
    background::Background,
    background_effect,
    damage::SCENE_CLEAR_COLOR,
    frame_clock::FrameClock,
    frame_clock::FrameTime,
    layers,
    render::{RenderStage, render_stage_elements},
    scene_backdrop::SceneBackdrop,
    scene_blur::BlurEffectManager,
    scene_composite::SceneCompositeElement,
    scene_composite::{scene_backdrop_elements, scene_elements},
    scene_render::{collect_window_scene_layers, window_layer_refs},
    state::KestrelState,
};
use smithay::{
    backend::{
        drm::compositor::FrameFlags,
        renderer::gles::GlesRenderer,
    },
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Monotonic, Time, Transform},
    wayland::shell::wlr_layer::Layer,
};
use std::time::Duration;

pub enum FrameResult {
    Idle,
    Queued {
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

    output.compositor.reset_buffer_ages();
    let elements: &[SceneCompositeElement] = &[];
    let frame = output
        .compositor
        .render_frame(renderer, elements, SCENE_CLEAR_COLOR, FrameFlags::DEFAULT)
        .map_err(compositor_error)?;
    if frame.is_empty {
        return Ok(false);
    }
    output
        .compositor
        .queue_frame(())
        .map_err(compositor_error)?;
    output.mark_frame_queued();
    Ok(true)
}

pub struct SessionFrameRenderer {
    background: Background,
    frame_clock: FrameClock,
    scene_backdrop: SceneBackdrop,
    blur_effects: BlurEffectManager,
    visible_popups: bool,
}

impl SessionFrameRenderer {
    pub fn new(state: &KestrelState, frame_interval: Duration) -> Self {
        Self {
            background: Background::new(state.config.compositor.background_image.clone()),
            frame_clock: FrameClock::new(frame_interval),
            scene_backdrop: SceneBackdrop::default(),
            blur_effects: BlurEffectManager::default(),
            visible_popups: state.has_visible_popups(),
        }
    }

    pub fn render(
        &mut self,
        state: &mut KestrelState,
        renderer: &mut GlesRenderer,
        compositor: &mut SessionCompositor,
        force_full_damage: bool,
    ) -> Result<FrameResult, DrmError> {
        let removed_windows = state.remove_dead_windows();
        let finished_window_closes = state.send_finished_window_closes();
        state.cleanup_layers();
        state.cleanup_output();
        let workspace_transition_active = state.workspace_transition().is_some();
        let visible_popups = state.has_visible_popups();
        let popup_visibility_changed =
            std::mem::replace(&mut self.visible_popups, visible_popups) != visible_popups;
        let content_render_needed = force_full_damage
            || state.scene_dirty()
            || popup_visibility_changed
            || removed_windows
            || finished_window_closes
            || state.animations_active()
            || workspace_transition_active
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
        self.blur_effects.retain_targets(&blur_targets);

        let background_element = self
            .background
            .render_element(renderer, state.output_size())
            .map_err(render_error)?;
        let background_layer =
            render_stage_elements(renderer, state, RenderStage::Layer(Layer::Background));
        let bottom_layer =
            render_stage_elements(renderer, state, RenderStage::Layer(Layer::Bottom));

        if force_full_damage || removed_windows || finished_window_closes {
            compositor.reset_buffer_ages();
            self.scene_backdrop.reset(state.output());
        }

        let output_size = state.output_size();
        let target_transform = Transform::Normal;
        let window_layers = collect_window_scene_layers(
            renderer,
            state,
            &mut self.blur_effects,
            output_size,
            target_transform,
            Some(&self.scene_backdrop),
        )
        .map_err(render_error)?;
        let top_blurs = self.blur_effects.elements_for(
            output_size,
            target_transform,
            &top_targets,
            Some(&self.scene_backdrop),
        );
        let overlay_blurs = self.blur_effects.elements_for(
            output_size,
            target_transform,
            &overlay_targets,
            Some(&self.scene_backdrop),
        );
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

        let window_layer_refs = window_layer_refs(&window_layers);
        let backdrop_elements = scene_backdrop_elements(
            background_element.as_ref(),
            &background_layer,
            &bottom_layer,
            &window_layer_refs,
        );
        self.scene_backdrop
            .render(renderer, output_size, &backdrop_elements)
            .map_err(render_error)?;
        let elements = scene_elements(
            background_element.as_ref(),
            &background_layer,
            &bottom_layer,
            &window_layer_refs,
            &top_blurs,
            &top_layer,
            &overlay_blurs,
            &overlay_layer,
        );

        let frame = compositor
            .render_frame(renderer, &elements, SCENE_CLEAR_COLOR, compositor_frame_flags())
            .map_err(compositor_error)?;
        if frame.is_empty {
            return Ok(FrameResult::Idle);
        }

        compositor.queue_frame(()).map_err(compositor_error)?;
        state.take_scene_dirty();
        Ok(FrameResult::Queued {
            callback_surfaces: state.frame_callback_surfaces(),
        })
    }

    pub fn reset_damage(&mut self, state: &KestrelState) {
        self.scene_backdrop.reset(state.output());
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
        self.reset_damage(state);
        self.blur_effects.retain_targets(&[]);
        self.visible_popups = state.has_visible_popups();
    }
}

fn compositor_frame_flags() -> FrameFlags {
    FrameFlags::ALLOW_PRIMARY_PLANE_SCANOUT
        | FrameFlags::ALLOW_PRIMARY_PLANE_SCANOUT_ANY
        | FrameFlags::ALLOW_CURSOR_PLANE_SCANOUT
}

fn compositor_error<E: std::fmt::Display>(error: E) -> DrmError {
    DrmError::Unsupported(format!("DRM compositor error: {error}"))
}

fn render_error(error: impl std::fmt::Display) -> DrmError {
    DrmError::Unsupported(format!("failed to render DRM frame: {error}"))
}

fn refresh_interval(refresh_millihertz: i32) -> Duration {
    let refresh = u64::try_from(refresh_millihertz)
        .ok()
        .filter(|refresh| *refresh > 0)
        .unwrap_or(crate::output::DEFAULT_REFRESH_MILLIHERTZ as u64);
    Duration::from_nanos((1_000_000_000_000u64 + refresh / 2) / refresh)
}
