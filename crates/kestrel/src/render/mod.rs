use crate::{
    layers,
    state::KestrelState,
    titlebar::{self, TITLEBAR_OVERLAP},
    window::{ManagedWindow, TITLEBAR_HEIGHT},
    window_clip::{ClipShape, RoundedWindowElement, WINDOW_RADIUS},
};
use smithay::{
    backend::renderer::{
        element::{
            Element, Kind,
            memory::MemoryRenderBufferRenderElement,
            surface::{WaylandSurfaceRenderElement, render_elements_from_surface_tree},
        },
        gles::{GlesError, GlesRenderer},
        utils::on_commit_buffer_handler,
    },
    desktop::PopupManager,
    reexports::wayland_server::protocol::wl_surface,
    utils::{Logical, Physical, Point, Rectangle, Scale, Size},
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

fn append_layer_elements(
    renderer: &mut GlesRenderer,
    state: &KestrelState,
    layer: Layer,
    elements: &mut Vec<LayerElement>,
) {
    for target in layers::render_surfaces(state.output(), layer) {
        let location = Point::<i32, Physical>::from((target.location.x, target.location.y));
        let clip = Rectangle::<i32, Physical>::new(location, (target.size.w, target.size.h).into());
        let surface_elements = render_elements_from_surface_tree(
            renderer,
            &target.surface,
            (target.location.x, target.location.y),
            1.0,
            1.0,
            Kind::Unspecified,
        )
        .into_iter()
        .map(|element| {
            RoundedWindowElement::new_with_shape(
                element,
                clip,
                layer_material_shape(target.material, target.size),
            )
        });
        let popup_elements =
            PopupManager::popups_for_surface(&target.surface).flat_map(|(popup, popup_offset)| {
                let offset = popup_offset - popup.geometry().loc;
                let popup_location = Point::<i32, Physical>::from((
                    target.location.x + offset.x,
                    target.location.y + offset.y,
                ));
                render_elements_from_surface_tree(
                    renderer,
                    popup.wl_surface(),
                    (popup_location.x, popup_location.y),
                    1.0,
                    1.0,
                    Kind::Unspecified,
                )
                .into_iter()
                .map(move |element: WaylandSurfaceRenderElement<GlesRenderer>| {
                    let clip = element.geometry(Scale::from(1.0));
                    RoundedWindowElement::new(element, clip, 0)
                })
            });

        elements.extend(surface_elements);
        elements.extend(popup_elements);
    }
}

fn layer_material_shape(
    material: layers::LayerMaterial,
    size: smithay::utils::Size<i32, Logical>,
) -> ClipShape {
    let clamp = |radius: i32| radius.max(0).min(size.w / 2).min(size.h / 2);
    match material {
        layers::LayerMaterial::Rect => ClipShape::Rect,
        layers::LayerMaterial::RoundRect { radius } => ClipShape::RoundRect {
            radius: clamp(radius),
        },
        layers::LayerMaterial::RoundTop { radius } => ClipShape::RoundTop {
            radius: clamp(radius),
        },
        layers::LayerMaterial::RoundLeft { radius } => ClipShape::RoundLeft {
            radius: clamp(radius),
        },
        layers::LayerMaterial::RoundRight { radius } => ClipShape::RoundRight {
            radius: clamp(radius),
        },
    }
}
