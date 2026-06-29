use smithay::{
    desktop::{
        LayerSurface, WindowSurfaceType, layer_map_for_output, utils::bbox_from_surface_tree,
    },
    output::Output,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Physical, Point, Rectangle},
    wayland::{
        alpha_modifier::AlphaModifierSurfaceCachedState,
        compositor,
        shell::wlr_layer::{Layer, LayerSurface as WlrLayerSurface},
    },
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
        "asher-panel",
        "asher-dock",
        "asher-dock-menu",
        "asher-date-center",
        "asher-launcher",
        "asher-quick-settings",
        "asher-sidebar",
        "asher-start-menu",
        "asher-notifications",
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
        let mut surfaces = layer_map.layers_on(layer).collect::<Vec<_>>();
        surfaces.reverse();
        for surface in surfaces {
            if !surface_accepts_input(surface)
                || !point_inside_layer_material(&layer_map, surface, point)
            {
                continue;
            }

            if surface.can_receive_keyboard_focus() {
                return Some(surface.wl_surface().clone());
            }
        }
    }

    None
}

pub fn has_layer_above_windows(output: &Output, point: Point<f64, Logical>) -> bool {
    [Layer::Overlay, Layer::Top]
        .into_iter()
        .any(|layer| pointer_focus_on_layer(output, point, layer).is_some())
}

const SHELL_CHROME_NAMESPACES: &[&str] = &["asher-panel", "asher-sidebar", "asher-dock"];

pub fn layer_surface_rects(output: &Output) -> Vec<(WlSurface, Rectangle<i32, Physical>)> {
    let layer_map = layer_map_for_output(output);
    let mut rects = Vec::new();
    for layer in [
        Layer::Background,
        Layer::Bottom,
        Layer::Top,
        Layer::Overlay,
    ] {
        for surface in layer_map.layers_on(layer) {
            let Some(geometry) = layer_map.layer_geometry(surface) else {
                continue;
            };
            let rect = Rectangle::<i32, Physical>::new(
                (geometry.loc.x, geometry.loc.y).into(),
                (geometry.size.w, geometry.size.h).into(),
            );
            rects.push((surface.wl_surface().clone(), rect));
        }
    }
    rects
}

pub fn should_close_transient_popover(output: &Output, point: Point<f64, Logical>) -> bool {
    if pointer_on_shell_chrome(output, point) {
        return false;
    }

    let layer_map = layer_map_for_output(output);
    let mut has_transient = false;
    for layer in [Layer::Overlay, Layer::Top] {
        for surface in layer_map.layers_on(layer) {
            if !matches!(
                surface.namespace(),
                "asher-quick-settings"
                    | "asher-date-center"
                    | "asher-start-menu"
                    | "asher-dock-menu"
            ) {
                continue;
            }
            if !surface_accepts_input(surface) {
                continue;
            }
            has_transient = true;
            if point_inside_layer_or_popup(&layer_map, surface, point) {
                return false;
            }
        }
    }
    has_transient
}

pub fn render_targets(
    output: &Output,
    layer: Layer,
    panel_taskbar: bool,
) -> Vec<LayerRenderTarget> {
    let layer_map = layer_map_for_output(output);
    layer_map
        .layers_on(layer)
        .filter_map(|surface| {
            if bbox_from_surface_tree(surface.wl_surface(), (0, 0)).is_empty() {
                return None;
            }
            let geometry = layer_map.layer_geometry(surface)?;
            let material = material_for(surface.namespace())?;
            let (location, size) = material_geometry(
                surface.namespace(),
                geometry.loc,
                geometry.size,
                panel_taskbar,
            );
            let opacity = material_opacity(
                surface.wl_surface(),
                (location.x - geometry.loc.x, location.y - geometry.loc.y).into(),
                size,
            );
            Some(LayerRenderTarget {
                surface: surface.wl_surface().clone(),
                blur_layer: BlurLayer::from_shell_layer(layer)?,
                material,
                opacity,
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
            let material = material_for(surface.namespace()).unwrap_or(LayerMaterial::Rect);
            Some(LayerRenderSurface {
                surface: surface.wl_surface().clone(),
                location: geometry.loc,
                size: geometry.size,
                material,
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
    pub blur_layer: BlurLayer,
    pub material: LayerMaterial,
    pub opacity: f32,
    pub location: Point<i32, Logical>,
    pub size: smithay::utils::Size<i32, Logical>,
}

#[derive(Debug, Clone)]
pub struct LayerRenderSurface {
    pub surface: WlSurface,
    pub location: Point<i32, Logical>,
    pub size: smithay::utils::Size<i32, Logical>,
    pub material: LayerMaterial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerMaterial {
    Rect,
    RoundRect { radius: i32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlurLayer {
    Window,
    Top,
    Overlay,
}

impl BlurLayer {
    pub(crate) fn from_shell_layer(layer: Layer) -> Option<Self> {
        match layer {
            Layer::Top => Some(Self::Top),
            Layer::Overlay => Some(Self::Overlay),
            _ => None,
        }
    }
}

fn material_for(namespace: &str) -> Option<LayerMaterial> {
    match namespace {
        "asher-panel" => Some(LayerMaterial::Rect),
        "asher-dock" => Some(LayerMaterial::RoundRect {
            radius: DOCK_BLUR_RADIUS,
        }),
        "asher-dock-menu" => Some(LayerMaterial::RoundRect { radius: 16 }),
        "asher-date-center" => Some(LayerMaterial::RoundRect { radius: 26 }),
        "asher-launcher" => Some(LayerMaterial::RoundRect { radius: 22 }),
        "asher-quick-settings" => Some(LayerMaterial::RoundRect { radius: 26 }),
        "asher-sidebar" => Some(LayerMaterial::Rect),
        "asher-start-menu" => Some(LayerMaterial::RoundRect { radius: 24 }),
        "asher-notifications" => Some(LayerMaterial::RoundRect { radius: 26 }),
        _ => None,
    }
}

fn material_geometry(
    namespace: &str,
    location: Point<i32, Logical>,
    size: smithay::utils::Size<i32, Logical>,
    _panel_taskbar: bool,
) -> (Point<i32, Logical>, smithay::utils::Size<i32, Logical>) {
    if namespace == "asher-panel" && size.h > TASKBAR_BLUR_HEIGHT {
        let vertical_inset = (size.h - TASKBAR_BLUR_HEIGHT).max(0);
        return (
            (location.x, location.y + vertical_inset).into(),
            (size.w, TASKBAR_BLUR_HEIGHT).into(),
        );
    }

    if namespace == "asher-quick-settings" || namespace == "asher-date-center" {
        return (location, size);
    }

    if namespace != "asher-dock" || size.h <= DOCK_BLUR_HEIGHT {
        return (location, size);
    }

    let vertical_inset = (size.h - DOCK_BLUR_HEIGHT).max(0);
    (
        (location.x, location.y + vertical_inset).into(),
        (size.w, DOCK_BLUR_HEIGHT).into(),
    )
}

fn material_opacity(
    surface: &WlSurface,
    _location: Point<i32, Logical>,
    _size: smithay::utils::Size<i32, Logical>,
) -> f32 {
    surface_alpha_multiplier(surface).clamp(0.0, 1.0)
}

fn surface_alpha_multiplier(surface: &WlSurface) -> f32 {
    compositor::with_states(surface, |states| {
        if !states.cached_state.has::<AlphaModifierSurfaceCachedState>() {
            return 1.0;
        }
        let mut alpha_state = states.cached_state.get::<AlphaModifierSurfaceCachedState>();
        alpha_state.current().multiplier_f32().unwrap_or(1.0)
    })
}

fn surface_accepts_input(surface: &LayerSurface) -> bool {
    surface_alpha_multiplier(surface.wl_surface()) > 0.01
        && !bbox_from_surface_tree(surface.wl_surface(), (0, 0)).is_empty()
}

fn pointer_focus_on_layer(
    output: &Output,
    point: Point<f64, Logical>,
    layer: Layer,
) -> Option<LayerPointerFocus> {
    let layer_map = layer_map_for_output(output);
    let mut surfaces = layer_map.layers_on(layer).collect::<Vec<_>>();
    surfaces.reverse();
    for layer_surface in surfaces {
        if !surface_accepts_input(layer_surface)
            || !point_inside_layer_or_popup(&layer_map, layer_surface, point)
        {
            continue;
        }

        let geometry = layer_map.layer_geometry(layer_surface)?;
        let point_in_layer: Point<f64, Logical> = (
            point.x - f64::from(geometry.loc.x),
            point.y - f64::from(geometry.loc.y),
        )
            .into();
        let Some((surface, surface_location)) =
            layer_surface.surface_under(point_in_layer, WindowSurfaceType::ALL)
        else {
            continue;
        };

        return Some(LayerPointerFocus {
            surface,
            location: (geometry.loc + surface_location).to_f64(),
        });
    }

    None
}

fn point_inside_layer_material(
    layer_map: &smithay::desktop::LayerMap,
    surface: &LayerSurface,
    point: Point<f64, Logical>,
) -> bool {
    let Some(geometry) = layer_map.layer_geometry(surface) else {
        return false;
    };
    let (location, size) =
        material_geometry(surface.namespace(), geometry.loc, geometry.size, false);
    point_in_rect(point, location, size)
}

fn pointer_on_shell_chrome(output: &Output, point: Point<f64, Logical>) -> bool {
    let layer_map = layer_map_for_output(output);
    for layer in [Layer::Top, Layer::Bottom] {
        for surface in layer_map.layers_on(layer) {
            if !SHELL_CHROME_NAMESPACES.contains(&surface.namespace()) {
                continue;
            }
            if !surface_accepts_input(surface) {
                continue;
            }
            if point_inside_full_layer(&layer_map, surface, point)
                || point_inside_layer_popups(&layer_map, surface, point)
            {
                return true;
            }
        }
    }
    false
}

fn point_inside_full_layer(
    layer_map: &smithay::desktop::LayerMap,
    surface: &LayerSurface,
    point: Point<f64, Logical>,
) -> bool {
    let Some(geometry) = layer_map.layer_geometry(surface) else {
        return false;
    };
    point_in_rect(point, geometry.loc, geometry.size)
}

fn point_inside_layer_popups(
    layer_map: &smithay::desktop::LayerMap,
    surface: &LayerSurface,
    point: Point<f64, Logical>,
) -> bool {
    let Some(geometry) = layer_map.layer_geometry(surface) else {
        return false;
    };
    let point_in_layer: Point<f64, Logical> = (
        point.x - f64::from(geometry.loc.x),
        point.y - f64::from(geometry.loc.y),
    )
        .into();
    let popup_bounds = surface.bbox_with_popups();
    point_in_rect(point_in_layer, popup_bounds.loc, popup_bounds.size)
}

fn point_inside_layer_or_popup(
    layer_map: &smithay::desktop::LayerMap,
    surface: &LayerSurface,
    point: Point<f64, Logical>,
) -> bool {
    if point_inside_layer_material(layer_map, surface, point) {
        return true;
    }
    let Some(geometry) = layer_map.layer_geometry(surface) else {
        return false;
    };
    let point_in_layer: Point<f64, Logical> = (
        point.x - f64::from(geometry.loc.x),
        point.y - f64::from(geometry.loc.y),
    )
        .into();
    let popup_bounds = surface.bbox_with_popups();
    point_in_rect(point_in_layer, popup_bounds.loc, popup_bounds.size)
}

fn point_in_rect(
    point: Point<f64, Logical>,
    location: Point<i32, Logical>,
    size: smithay::utils::Size<i32, Logical>,
) -> bool {
    point.x >= f64::from(location.x)
        && point.y >= f64::from(location.y)
        && point.x < f64::from(location.x + size.w)
        && point.y < f64::from(location.y + size.h)
}
