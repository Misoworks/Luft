use smithay::{
    desktop::{
        LayerSurface, WindowSurfaceType, layer_map_for_output, utils::bbox_from_surface_tree,
    },
    output::Output,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point},
    wayland::{
        alpha_modifier::AlphaModifierSurfaceCachedState,
        compositor::{self, BufferAssignment, SurfaceAttributes},
        shell::wlr_layer::{Layer, LayerSurface as WlrLayerSurface},
        shm,
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
                surface.namespace(),
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
const POPOVER_RIGHT_MARGIN: i32 = 12;
const POPOVER_TOP_MARGIN: i32 = 42;
const POPOVER_TASKBAR_BOTTOM_MARGIN: i32 = TASKBAR_BLUR_HEIGHT + 8;
const QUICK_SETTINGS_BLUR_WIDTH: i32 = 420;
const QUICK_SETTINGS_BLUR_HEIGHT: i32 = 230;
const DATE_CENTER_BLUR_WIDTH: i32 = 360;
const DATE_CENTER_BLUR_HEIGHT: i32 = 470;
const MATERIAL_FULL_BLUR_ALPHA: f32 = 120.0;
const MATERIAL_VISIBLE_ALPHA_FLOOR: f32 = 4.0;

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
    fn from_shell_layer(layer: Layer) -> Option<Self> {
        match layer {
            Layer::Top => Some(Self::Top),
            Layer::Overlay => Some(Self::Overlay),
            _ => None,
        }
    }
}

fn material_for(namespace: &str) -> Option<LayerMaterial> {
    match namespace {
        "staccato-panel" => Some(LayerMaterial::Rect),
        "staccato-dock" => Some(LayerMaterial::RoundRect {
            radius: DOCK_BLUR_RADIUS,
        }),
        "staccato-dock-menu" => Some(LayerMaterial::RoundRect { radius: 16 }),
        "staccato-date-center" => Some(LayerMaterial::RoundRect { radius: 26 }),
        "staccato-launcher" => Some(LayerMaterial::RoundRect { radius: 22 }),
        "staccato-quick-settings" => Some(LayerMaterial::RoundRect { radius: 26 }),
        "staccato-sidebar" => Some(LayerMaterial::Rect),
        "staccato-notifications" => Some(LayerMaterial::RoundRect { radius: 18 }),
        _ => None,
    }
}

fn material_geometry(
    namespace: &str,
    location: Point<i32, Logical>,
    size: smithay::utils::Size<i32, Logical>,
    panel_taskbar: bool,
) -> (Point<i32, Logical>, smithay::utils::Size<i32, Logical>) {
    if namespace == "staccato-panel" && size.h > TASKBAR_BLUR_HEIGHT {
        let vertical_inset = (size.h - TASKBAR_BLUR_HEIGHT).max(0);
        return (
            (location.x, location.y + vertical_inset).into(),
            (size.w, TASKBAR_BLUR_HEIGHT).into(),
        );
    }

    if namespace == "staccato-quick-settings" {
        return popover_material_geometry(
            location,
            size,
            QUICK_SETTINGS_BLUR_WIDTH,
            QUICK_SETTINGS_BLUR_HEIGHT,
            panel_taskbar,
        );
    }

    if namespace == "staccato-date-center" {
        return popover_material_geometry(
            location,
            size,
            DATE_CENTER_BLUR_WIDTH,
            DATE_CENTER_BLUR_HEIGHT,
            panel_taskbar,
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

fn material_opacity(
    namespace: &str,
    surface: &WlSurface,
    location: Point<i32, Logical>,
    size: smithay::utils::Size<i32, Logical>,
) -> f32 {
    let alpha = surface_alpha_multiplier(surface);
    match namespace {
        "staccato-dock-menu" | "staccato-date-center" | "staccato-quick-settings" => {
            alpha
                * sampled_surface_opacity(surface, location, size)
                    .unwrap_or(1.0)
                    .clamp(0.0, 1.0)
        }
        _ => alpha,
    }
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

fn sampled_surface_opacity(
    surface: &WlSurface,
    location: Point<i32, Logical>,
    size: smithay::utils::Size<i32, Logical>,
) -> Option<f32> {
    let buffer = compositor::with_states(surface, |states| {
        let mut attributes = states.cached_state.get::<SurfaceAttributes>();
        match attributes.current().buffer.as_ref()? {
            BufferAssignment::NewBuffer(buffer) => Some(buffer.clone()),
            BufferAssignment::Removed => None,
        }
    })?;

    shm::with_buffer_contents(&buffer, |ptr, len, data| {
        sample_argb8888_material_opacity(ptr, len, data, location, size)
    })
    .ok()
    .flatten()
}

fn sample_argb8888_material_opacity(
    ptr: *const u8,
    len: usize,
    data: shm::BufferData,
    location: Point<i32, Logical>,
    size: smithay::utils::Size<i32, Logical>,
) -> Option<f32> {
    if data.format != smithay::reexports::wayland_server::protocol::wl_shm::Format::Argb8888 {
        return None;
    }
    let left = location.x.clamp(0, data.width.saturating_sub(1));
    let top = location.y.clamp(0, data.height.saturating_sub(1));
    let right = (location.x + size.w).clamp(left + 1, data.width);
    let bottom = (location.y + size.h).clamp(top + 1, data.height);
    let sample_columns = 20.min((right - left).max(1));
    let sample_rows = 20.min((bottom - top).max(1));
    let mut alpha_total = 0_u32;
    let mut samples = 0_u32;

    for row in 0..sample_rows {
        let y = top + ((bottom - top - 1) * row / sample_rows.max(1));
        for column in 0..sample_columns {
            let x = left + ((right - left - 1) * column / sample_columns.max(1));
            let offset = data.offset as isize + (y * data.stride + x * 4 + 3) as isize;
            if offset < 0 || offset as usize >= len {
                continue;
            }
            let alpha = unsafe { ptr.offset(offset).read() };
            alpha_total += u32::from(alpha);
            samples += 1;
        }
    }

    if samples == 0 {
        return None;
    }
    Some(material_blur_opacity(alpha_total as f32 / samples as f32))
}

fn material_blur_opacity(average_alpha: f32) -> f32 {
    if average_alpha <= MATERIAL_VISIBLE_ALPHA_FLOOR {
        return 0.0;
    }
    let opacity = (average_alpha / MATERIAL_FULL_BLUR_ALPHA).clamp(0.0, 1.0);
    opacity * opacity
}

fn popover_material_geometry(
    location: Point<i32, Logical>,
    size: smithay::utils::Size<i32, Logical>,
    preferred_width: i32,
    preferred_height: i32,
    panel_taskbar: bool,
) -> (Point<i32, Logical>, smithay::utils::Size<i32, Logical>) {
    let width = preferred_width.min(size.w).max(1);
    let vertical_margin = if panel_taskbar {
        POPOVER_TASKBAR_BOTTOM_MARGIN
    } else {
        POPOVER_TOP_MARGIN
    };
    let available_height = (size.h - vertical_margin).max(1);
    let height = preferred_height.min(available_height).max(1);
    let x = location.x + (size.w - width - POPOVER_RIGHT_MARGIN).max(0);
    let y = if panel_taskbar {
        location.y + (size.h - height - POPOVER_TASKBAR_BOTTOM_MARGIN).max(0)
    } else {
        location.y + POPOVER_TOP_MARGIN.min((size.h - height).max(0))
    };

    ((x, y).into(), (width, height).into())
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
