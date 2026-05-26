use smithay::{
    desktop::{LayerSurface, WindowSurfaceType, layer_map_for_output},
    output::Output,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point},
    wayland::shell::wlr_layer::{Layer, LayerSurface as WlrLayerSurface},
};
use tracing::debug;

pub fn map(output: &Output, surface: WlrLayerSurface, namespace: String) {
    let layer = LayerSurface::new(surface, namespace);
    if let Err(error) = layer_map_for_output(output).map_layer(&layer) {
        debug!(
            ?error,
            namespace = layer.namespace(),
            "failed to map layer surface"
        );
    }
}

pub fn unmap(output: &Output, surface: &WlrLayerSurface) {
    let mut layer_map = layer_map_for_output(output);
    let layer = layer_map
        .layers()
        .find(|layer| layer.layer_surface() == surface)
        .cloned();

    if let Some(layer) = layer {
        layer_map.unmap_layer(&layer);
    }
}

pub fn arrange(output: &Output) {
    layer_map_for_output(output).arrange();
}

pub fn cleanup(output: &Output) {
    layer_map_for_output(output).cleanup();
}

pub fn surfaces(output: &Output) -> Vec<WlSurface> {
    layer_map_for_output(output)
        .layers()
        .map(|layer| layer.wl_surface().clone())
        .collect()
}

pub fn has_shell_surface(output: &Output) -> bool {
    const SHELL_NAMESPACES: &[&str] = &[
        "staccato-panel",
        "staccato-dock",
        "staccato-dock-menu",
        "staccato-date-center",
        "staccato-launcher",
        "staccato-overview",
        "staccato-quick-settings",
        "staccato-sidebar",
        "staccato-notifications",
    ];

    layer_map_for_output(output)
        .layers()
        .any(|layer| SHELL_NAMESPACES.contains(&layer.namespace()))
}

pub fn pointer_focus(output: &Output, point: Point<f64, Logical>) -> Option<LayerPointerFocus> {
    for layer in [Layer::Overlay, Layer::Top, Layer::Bottom, Layer::Background] {
        if let Some(focus) = pointer_focus_on_layer(output, point, layer) {
            return Some(focus);
        }
    }

    None
}

pub fn keyboard_focus(output: &Output, point: Point<f64, Logical>) -> Option<WlSurface> {
    let layer_map = layer_map_for_output(output);
    for layer in [Layer::Overlay, Layer::Top, Layer::Bottom, Layer::Background] {
        let Some(surface) = layer_map.layer_under(layer, point) else {
            continue;
        };

        if surface.can_receive_keyboard_focus() {
            return Some(surface.wl_surface().clone());
        }
    }

    None
}

pub fn has_layer_above_windows(output: &Output, point: Point<f64, Logical>) -> bool {
    [Layer::Overlay, Layer::Top]
        .into_iter()
        .any(|layer| pointer_focus_on_layer(output, point, layer).is_some())
}

pub fn render_targets(output: &Output, layer: Layer) -> Vec<LayerRenderTarget> {
    let layer_map = layer_map_for_output(output);
    layer_map
        .layers_on(layer)
        .filter_map(|surface| {
            let geometry = layer_map.layer_geometry(surface)?;
            let material = material_for(surface.namespace())?;
            let (location, size) =
                material_geometry(surface.namespace(), geometry.loc, geometry.size);
            Some(LayerRenderTarget {
                surface: surface.wl_surface().clone(),
                material,
                location,
                size,
            })
        })
        .collect()
}

pub fn render_surfaces(output: &Output, layer: Layer) -> Vec<LayerRenderSurface> {
    let layer_map = layer_map_for_output(output);
    layer_map
        .layers_on(layer)
        .filter_map(|surface| {
            let geometry = layer_map.layer_geometry(surface)?;
            Some(LayerRenderSurface {
                surface: surface.wl_surface().clone(),
                location: geometry.loc,
            })
        })
        .collect()
}

const DOCK_BLUR_HEIGHT: i32 = 50;
const DOCK_BLUR_RADIUS: i32 = 18;
const TASKBAR_BLUR_HEIGHT: i32 = 48;

#[derive(Debug, Clone)]
pub struct LayerPointerFocus {
    pub surface: WlSurface,
    pub location: Point<f64, Logical>,
}

#[derive(Debug, Clone)]
pub struct LayerRenderTarget {
    pub surface: WlSurface,
    pub material: LayerMaterial,
    pub location: Point<i32, Logical>,
    pub size: smithay::utils::Size<i32, Logical>,
}

#[derive(Debug, Clone)]
pub struct LayerRenderSurface {
    pub surface: WlSurface,
    pub location: Point<i32, Logical>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerMaterial {
    Rect,
    RoundRect { radius: i32 },
}

fn material_for(namespace: &str) -> Option<LayerMaterial> {
    match namespace {
        "staccato-panel" => Some(LayerMaterial::Rect),
        "staccato-dock" => Some(LayerMaterial::RoundRect {
            radius: DOCK_BLUR_RADIUS,
        }),
        "staccato-dock-menu" => Some(LayerMaterial::RoundRect { radius: 16 }),
        "staccato-date-center" => Some(LayerMaterial::RoundRect { radius: 28 }),
        "staccato-launcher" => Some(LayerMaterial::RoundRect { radius: 22 }),
        "staccato-overview" => Some(LayerMaterial::Rect),
        "staccato-quick-settings" => Some(LayerMaterial::RoundRect { radius: 28 }),
        "staccato-sidebar" => Some(LayerMaterial::Rect),
        "staccato-notifications" => Some(LayerMaterial::RoundRect { radius: 18 }),
        _ => None,
    }
}

fn material_geometry(
    namespace: &str,
    location: Point<i32, Logical>,
    size: smithay::utils::Size<i32, Logical>,
) -> (Point<i32, Logical>, smithay::utils::Size<i32, Logical>) {
    if namespace == "staccato-panel" && size.h > TASKBAR_BLUR_HEIGHT {
        let vertical_inset = (size.h - TASKBAR_BLUR_HEIGHT).max(0);
        return (
            (location.x, location.y + vertical_inset).into(),
            (size.w, TASKBAR_BLUR_HEIGHT).into(),
        );
    }

    if namespace != "staccato-dock" || size.h <= DOCK_BLUR_HEIGHT {
        return (location, size);
    }

    let vertical_inset = (size.h - DOCK_BLUR_HEIGHT).max(0);
    (
        (location.x, location.y + vertical_inset).into(),
        (size.w, DOCK_BLUR_HEIGHT).into(),
    )
}

fn pointer_focus_on_layer(
    output: &Output,
    point: Point<f64, Logical>,
    layer: Layer,
) -> Option<LayerPointerFocus> {
    let layer_map = layer_map_for_output(output);
    let layer_surface = layer_map.layer_under(layer, point)?;
    let geometry = layer_map.layer_geometry(layer_surface)?;
    let point_in_layer: Point<f64, Logical> = (
        point.x - f64::from(geometry.loc.x),
        point.y - f64::from(geometry.loc.y),
    )
        .into();
    let (surface, surface_location) =
        layer_surface.surface_under(point_in_layer, WindowSurfaceType::ALL)?;

    Some(LayerPointerFocus {
        surface,
        location: (geometry.loc + surface_location).to_f64(),
    })
}
