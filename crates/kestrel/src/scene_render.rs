use crate::{
    background_effect,
    damage::DamageTracker,
    render::{LayerElement, window_chrome_elements_for_window},
    scene_backdrop::SceneBackdrop,
    scene_blur::{BlurEffectManager, FramebufferBlurElement},
    scene_composite::{scene_backdrop_elements, scene_elements, WindowSceneLayerRef},
    state::KestrelState,
    window_clip::{window_elements_for_window, RoundedWindowElement},
};
use smithay::{
    backend::renderer::{
        element::{memory::MemoryRenderBufferRenderElement, surface::WaylandSurfaceRenderElement},
        gles::{GlesError, GlesRenderer, GlesTarget},
    },
    utils::{Physical, Rectangle, Size, Transform},
};

type LayerSurfaceElement = LayerElement;
type WindowSurfaceElement = WaylandSurfaceRenderElement<GlesRenderer>;
type MemoryElement = MemoryRenderBufferRenderElement<GlesRenderer>;
type WindowElement = RoundedWindowElement<WindowSurfaceElement>;

pub struct WindowSceneLayer {
    pub chrome: Vec<MemoryElement>,
    pub surfaces: Vec<WindowElement>,
    pub blurs: Vec<FramebufferBlurElement>,
}

pub struct SceneRenderRequest<'a> {
    pub output_size: Size<i32, Physical>,
    pub background: Option<MemoryElement>,
    pub background_layer: &'a [LayerSurfaceElement],
    pub bottom_layer: &'a [LayerSurfaceElement],
    pub window_layers: &'a [WindowSceneLayer],
    pub top_blurs: &'a [FramebufferBlurElement],
    pub top_layer: &'a [LayerSurfaceElement],
    pub overlay_blurs: &'a [FramebufferBlurElement],
    pub overlay_layer: &'a [LayerSurfaceElement],
}

pub fn collect_window_scene_layers(
    renderer: &mut GlesRenderer,
    state: &KestrelState,
    blur_effects: &mut BlurEffectManager,
    output_size: Size<i32, Physical>,
    target_transform: Transform,
    backdrop: Option<&SceneBackdrop>,
) -> Result<Vec<WindowSceneLayer>, GlesError> {
    let mut layers = Vec::new();
    let grouped_targets = background_effect::window_blur_targets_grouped(state);
    let mut target_groups = grouped_targets.into_iter();

    if let Some(transition) = state.workspace_transition() {
        let width = state.output_size().w as f64;
        let direction = transition.direction as f64;
        let from_offset = (-direction * width * transition.progress).round() as i32;
        let to_offset = (direction * width * (1.0 - transition.progress)).round() as i32;
        append_workspace_layers(
            renderer,
            state,
            blur_effects,
            output_size,
            target_transform,
            backdrop,
            &transition.from,
            from_offset,
            &mut target_groups,
            &mut layers,
        )?;
        append_workspace_layers(
            renderer,
            state,
            blur_effects,
            output_size,
            target_transform,
            backdrop,
            &transition.to,
            to_offset,
            &mut target_groups,
            &mut layers,
        )?;
    } else {
        append_workspace_layers(
            renderer,
            state,
            blur_effects,
            output_size,
            target_transform,
            backdrop,
            state.layout.active_workspace(),
            0,
            &mut target_groups,
            &mut layers,
        )?;
    }

    Ok(layers)
}

#[allow(clippy::too_many_arguments)]
fn append_workspace_layers(
    renderer: &mut GlesRenderer,
    state: &KestrelState,
    blur_effects: &mut BlurEffectManager,
    output_size: Size<i32, Physical>,
    target_transform: Transform,
    backdrop: Option<&SceneBackdrop>,
    workspace: &luft_ipc::WorkspaceId,
    offset_x: i32,
    target_groups: &mut impl Iterator<Item = Vec<crate::layers::LayerRenderTarget>>,
    layers: &mut Vec<WindowSceneLayer>,
) -> Result<(), GlesError> {
    for window in state.windows.render_windows_on_workspace(workspace) {
        let targets = target_groups.next().unwrap_or_default();
        layers.push(WindowSceneLayer {
            chrome: window_chrome_elements_for_window(renderer, state, window, offset_x)?,
            surfaces: window_elements_for_window(renderer, window, offset_x, output_size),
            blurs: blur_effects.elements_for(
                output_size,
                target_transform,
                &targets,
                backdrop,
            ),
        });
    }
    Ok(())
}

pub fn render_scene(
    damage_tracker: &mut DamageTracker,
    backdrop: &mut SceneBackdrop,
    renderer: &mut GlesRenderer,
    framebuffer: &mut GlesTarget<'_>,
    request: SceneRenderRequest<'_>,
    buffer_age: usize,
) -> Result<Option<Vec<Rectangle<i32, Physical>>>, GlesError> {
    let window_layer_refs = window_layer_refs(request.window_layers);
    let backdrop_elements = scene_backdrop_elements(
        request.background.as_ref(),
        request.background_layer,
        request.bottom_layer,
        &window_layer_refs,
    );
    backdrop.render(renderer, request.output_size, &backdrop_elements)?;

    let elements = scene_elements(
        request.background.as_ref(),
        request.background_layer,
        request.bottom_layer,
        &window_layer_refs,
        request.top_blurs,
        request.top_layer,
        request.overlay_blurs,
        request.overlay_layer,
    );

    damage_tracker.render_output(
        renderer,
        framebuffer,
        request.output_size,
        buffer_age,
        &elements,
    )
}

pub fn window_layer_refs(window_layers: &[WindowSceneLayer]) -> Vec<WindowSceneLayerRef<'_>> {
    window_layers
        .iter()
        .map(|layer| WindowSceneLayerRef {
            chrome: &layer.chrome,
            surfaces: &layer.surfaces,
            blurs: &layer.blurs,
        })
        .collect()
}
