use crate::{
    layers,
    state::KestrelState,
    titlebar::{self, TITLEBAR_OVERLAP},
    window::{ManagedWindow, TITLEBAR_HEIGHT},
    window_clip::{RoundedWindowElement, WINDOW_RADIUS},
};
use asher_ipc::WorkspaceId;
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
    desktop::PopupManager,
    reexports::wayland_server::protocol::wl_surface,
    utils::{Logical, Physical, Point, Rectangle, Size},
    wayland::shell::wlr_layer::Layer,
};

pub fn handle_commit(surface: &wl_surface::WlSurface) {
    on_commit_buffer_handler::<KestrelState>(surface);
}

pub type LayerElement = RoundedWindowElement<WaylandSurfaceRenderElement<GlesRenderer>>;

pub fn render_stage_elements(
    renderer: &mut GlesRenderer,
    state: &KestrelState,
    stage: RenderStage,
) -> Vec<LayerElement> {
    let mut elements = Vec::new();
    match stage {
        RenderStage::Layer(layer) => append_layer_elements(renderer, state, layer, &mut elements),
    }
    elements
}

pub fn window_chrome_elements(
    renderer: &mut GlesRenderer,
    state: &KestrelState,
) -> Result<Vec<MemoryRenderBufferRenderElement<GlesRenderer>>, GlesError> {
    let mut elements = Vec::new();
    if let Some(transition) = state.workspace_transition() {
        let width = state.output_size().w as f64;
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
    state: &KestrelState,
    window: &ManagedWindow,
    offset_x: i32,
) -> Result<Vec<MemoryRenderBufferRenderElement<GlesRenderer>>, GlesError> {
    if window.titlebar_height() == 0 {
        return Ok(Vec::new());
    }

    let transform = window.render_transform(offset_x, state.output_size());
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
    state: &KestrelState,
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
    state: &KestrelState,
    layer: Layer,
    elements: &mut Vec<LayerElement>,
) {
    for target in layers::render_surfaces(state.output(), layer) {
        let location = Point::<i32, Physical>::from((target.location.x, target.location.y));
        let clip = Rectangle::<i32, Physical>::new(location, (target.size.w, target.size.h).into());
        elements.extend(
            render_elements_from_surface_tree(
                renderer,
                &target.surface,
                (target.location.x, target.location.y),
                1.0,
                1.0,
                Kind::Unspecified,
            )
            .into_iter()
            .map(|element| {
                RoundedWindowElement::new(
                    element,
                    clip,
                    layer_material_radius(target.material, target.size),
                )
            }),
        );

        for (popup, popup_offset) in PopupManager::popups_for_surface(&target.surface) {
            let offset = popup_offset - popup.geometry().loc;
            let popup_location = Point::<i32, Physical>::from((
                target.location.x + offset.x,
                target.location.y + offset.y,
            ));
            let popup_size = popup.geometry().size;
            let popup_clip = Rectangle::<i32, Physical>::new(
                popup_location,
                (popup_size.w, popup_size.h).into(),
            );
            elements.extend(
                render_elements_from_surface_tree(
                    renderer,
                    popup.wl_surface(),
                    (popup_location.x, popup_location.y),
                    1.0,
                    1.0,
                    Kind::Unspecified,
                )
                .into_iter()
                .map(|element| RoundedWindowElement::new(element, popup_clip, 0)),
            );
        }
    }
}

fn layer_material_radius(
    material: layers::LayerMaterial,
    size: smithay::utils::Size<i32, Logical>,
) -> i32 {
    match material {
        layers::LayerMaterial::Rect => 0,
        layers::LayerMaterial::RoundRect { radius }
        | layers::LayerMaterial::RoundLeft { radius }
        | layers::LayerMaterial::RoundRight { radius } => {
            radius.max(0).min(size.w / 2).min(size.h / 2)
        }
    }
}
