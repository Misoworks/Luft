use crate::{
    layers,
    state::BatonState,
    titlebar::{self, TITLEBAR_OVERLAP},
    window::{ManagedWindow, TITLEBAR_HEIGHT},
    window_clip::WINDOW_RADIUS,
};
use smithay::{
    backend::renderer::{
        element::{
            Kind,
            memory::MemoryRenderBufferRenderElement,
            surface::{WaylandSurfaceRenderElement, render_elements_from_surface_tree},
        },
        gles::{GlesError, GlesRenderer},
        utils::on_commit_buffer_handler,
    },
    reexports::wayland_server::protocol::wl_surface,
    utils::{Logical, Size},
    wayland::shell::wlr_layer::Layer,
};
use staccato_layout::WorkspaceId;

pub fn handle_commit(surface: &wl_surface::WlSurface) {
    on_commit_buffer_handler::<BatonState>(surface);
}

pub fn render_stage_elements(
    renderer: &mut GlesRenderer,
    state: &BatonState,
    stage: RenderStage,
) -> Vec<WaylandSurfaceRenderElement<GlesRenderer>> {
    let mut elements = Vec::new();
    match stage {
        RenderStage::Layer(layer) => append_layer_elements(renderer, state, layer, &mut elements),
    }
    elements
}

pub fn window_chrome_elements(
    renderer: &mut GlesRenderer,
    state: &BatonState,
) -> Result<Vec<MemoryRenderBufferRenderElement<GlesRenderer>>, GlesError> {
    let mut elements = Vec::new();
    if let Some(transition) = state.workspace_transition() {
        let width = state.output_size.w as f64;
        let direction = transition.direction as f64;
        let from_offset = (-direction * width * transition.progress).round() as i32;
        let to_offset = (direction * width * (1.0 - transition.progress)).round() as i32;
        append_workspace_chrome(
            renderer,
            state,
            &mut elements,
            &transition.from,
            from_offset,
        )?;
        append_workspace_chrome(renderer, state, &mut elements, &transition.to, to_offset)?;
        return Ok(elements);
    }

    append_workspace_chrome(
        renderer,
        state,
        &mut elements,
        state.layout.active_workspace(),
        0,
    )?;
    Ok(elements)
}

pub fn window_chrome_elements_for_window(
    renderer: &mut GlesRenderer,
    state: &BatonState,
    window: &ManagedWindow,
    offset_x: i32,
) -> Result<Vec<MemoryRenderBufferRenderElement<GlesRenderer>>, GlesError> {
    if window.titlebar_height() == 0 {
        return Ok(Vec::new());
    }

    let transform = window.render_transform(offset_x, state.output_size);
    let titlebar_width = (window.size.w as f64 * transform.scale).round().max(1.0) as i32;
    let titlebar_height = ((TITLEBAR_HEIGHT + TITLEBAR_OVERLAP) as f64 * transform.scale)
        .round()
        .max(1.0) as i32;
    let pointer_x = ((state.pointer_location.x - transform.x) / transform.scale).round() as i32;
    let pointer_y = ((state.pointer_location.y - transform.y) / transform.scale).round() as i32;
    let hover = titlebar::hover_state(window.size.w, pointer_x, pointer_y);
    let radius = titlebar_radius(window);
    let mut titlebar_cache = state.titlebar_cache.borrow_mut();
    let buffer = titlebar_cache.buffer(window.size.w, hover, radius);
    let element = MemoryRenderBufferRenderElement::from_buffer(
        renderer,
        (transform.x, transform.y),
        buffer,
        Some(transform.alpha),
        None,
        Some(Size::<i32, Logical>::from((
            titlebar_width,
            titlebar_height,
        ))),
        Kind::Unspecified,
    )?;

    Ok(vec![element])
}

fn titlebar_radius(window: &ManagedWindow) -> i32 {
    if window.flat_frame_corners() {
        0
    } else {
        WINDOW_RADIUS
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RenderStage {
    Layer(Layer),
}

fn append_workspace_chrome(
    renderer: &mut GlesRenderer,
    state: &BatonState,
    elements: &mut Vec<MemoryRenderBufferRenderElement<GlesRenderer>>,
    workspace: &WorkspaceId,
    offset_x: i32,
) -> Result<(), GlesError> {
    for window in state.windows.render_windows_on_workspace(workspace) {
        elements.extend(window_chrome_elements_for_window(
            renderer, state, window, offset_x,
        )?);
    }

    Ok(())
}

fn append_layer_elements(
    renderer: &mut GlesRenderer,
    state: &BatonState,
    layer: Layer,
    elements: &mut Vec<WaylandSurfaceRenderElement<GlesRenderer>>,
) {
    for target in layers::render_surfaces(&state.output, layer) {
        elements.extend(render_elements_from_surface_tree(
            renderer,
            &target.surface,
            (target.location.x, target.location.y),
            1.0,
            1.0,
            Kind::Unspecified,
        ));
    }
}
