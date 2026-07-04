use super::current_blur_region;
use crate::{
    layers::{BlurLayer, LayerMaterial, LayerRenderTarget, material_for},
    state::KestrelState,
    window::ManagedWindow,
    window_clip::WINDOW_RADIUS,
};
use luft_ipc::WorkspaceId;
use smithay::{
    desktop::{PopupManager, layer_map_for_output},
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Rectangle},
    wayland::{
        compositor::{self, RectangleKind, RegionAttributes, SurfaceAttributes},
        shell::wlr_layer::Layer,
    },
};

pub fn window_blur_targets(state: &KestrelState) -> Vec<LayerRenderTarget> {
    window_blur_targets_grouped(state)
        .into_iter()
        .flatten()
        .collect()
}

pub fn window_blur_targets_grouped(state: &KestrelState) -> Vec<Vec<LayerRenderTarget>> {
    let mut grouped = Vec::new();
    if let Some(transition) = state.workspace_transition() {
        let width = state.output_size().w as f64;
        let direction = transition.direction as f64;
        let from_offset = (-direction * width * transition.progress).round() as i32;
        let to_offset = (direction * width * (1.0 - transition.progress)).round() as i32;
        append_workspace_targets_grouped(state, &transition.from, from_offset, &mut grouped);
        append_workspace_targets_grouped(state, &transition.to, to_offset, &mut grouped);
    } else {
        append_workspace_targets_grouped(state, state.layout.active_workspace(), 0, &mut grouped);
    }
    grouped
}

pub fn layer_popup_blur_targets(state: &KestrelState, layer: Layer) -> Vec<LayerRenderTarget> {
    let layer_map = layer_map_for_output(state.output());
    let Some(blur_layer) = BlurLayer::from_shell_layer(layer) else {
        return Vec::new();
    };
    let mut targets = Vec::new();
    for parent in layer_map.layers_on(layer) {
        let Some(parent_geometry) = layer_map.layer_geometry(parent) else {
            continue;
        };
        let popup_radius = material_for(parent.namespace())
            .and_then(|material| match material {
                LayerMaterial::RoundRect { radius } => Some(radius),
                _ => None,
            })
            .unwrap_or(18);
        for (popup, popup_offset) in PopupManager::popups_for_surface(parent.wl_surface()) {
            let popup_geometry = popup.geometry();
            let popup_location = parent_geometry.loc + popup_offset - popup_geometry.loc;
            append_surface_region_targets(
                popup.wl_surface(),
                blur_layer,
                popup_location,
                popup_geometry.size,
                LayerMaterial::RoundRect {
                    radius: popup_radius,
                },
                &mut targets,
            );
        }
    }
    targets
}

fn append_workspace_targets_grouped(
    state: &KestrelState,
    workspace: &WorkspaceId,
    offset_x: i32,
    grouped: &mut Vec<Vec<LayerRenderTarget>>,
) {
    let mut opaque_above = Vec::new();
    for window in state.windows.render_windows_on_workspace(workspace) {
        let transform = window.render_transform(offset_x, state.output_size());
        let mut window_targets = Vec::new();
        append_window_targets(window, transform, &mut window_targets, &opaque_above);
        grouped.push(window_targets);
        opaque_above.extend(window_opaque_rects(window, transform));
    }
}

fn append_window_targets(
    window: &ManagedWindow,
    transform: crate::window_animation::WindowTransform,
    targets: &mut Vec<LayerRenderTarget>,
    opaque_above: &[Rectangle<i32, Logical>],
) {
    let surface = window.surface.wl_surface();
    let titlebar_height = window.titlebar_height();
    let surface_offset = window.surface_offset();
    if titlebar_height > 0 {
        let target = LayerRenderTarget {
            surface: surface.clone(),
            blur_layer: BlurLayer::Window,
            material: LayerMaterial::RoundTop {
                radius: titlebar_radius(window, transform.scale),
            },
            opacity: 1.0,
            location: Point::from((transform.x.round() as i32, transform.y.round() as i32)),
            size: (
                (window.size.w as f64 * transform.scale).round().max(1.0) as i32,
                (titlebar_height as f64 * transform.scale).round().max(1.0) as i32,
            )
                .into(),
        };
        if !target_occluded(&target, opaque_above) {
            targets.push(target);
        }
    }
    let Some(region) = current_blur_region(surface) else {
        return;
    };
    let clip = Rectangle::from_size(window.surface_geometry().size);
    for target in targets_for_region(&region, clip) {
        let location = Point::from((
            (transform.x + (surface_offset.x + target.rect.loc.x) as f64 * transform.scale).round()
                as i32,
            (transform.y
                + (titlebar_height + surface_offset.y + target.rect.loc.y) as f64 * transform.scale)
                .round() as i32,
        ));
        let size = (
            (target.rect.size.w as f64 * transform.scale)
                .round()
                .max(1.0) as i32,
            (target.rect.size.h as f64 * transform.scale)
                .round()
                .max(1.0) as i32,
        )
            .into();
        let target = LayerRenderTarget {
            surface: surface.clone(),
            blur_layer: BlurLayer::Window,
            material: scaled_material(target.material, transform.scale),
            opacity: 1.0,
            location,
            size,
        };
        if !target_occluded(&target, opaque_above) {
            targets.push(target);
        }
    }
}

fn target_occluded(target: &LayerRenderTarget, opaque_above: &[Rectangle<i32, Logical>]) -> bool {
    let rect = Rectangle::<i32, Logical>::new(target.location, target.size);
    opaque_above
        .iter()
        .any(|opaque| rect_contains(*opaque, rect))
}

fn window_opaque_rects(
    window: &ManagedWindow,
    transform: crate::window_animation::WindowTransform,
) -> Vec<Rectangle<i32, Logical>> {
    if transform.alpha < 0.999 {
        return Vec::new();
    }

    let clip = window.surface_geometry();
    let titlebar_height = window.titlebar_height();
    let surface_offset = window.surface_offset();
    current_opaque_rects(window.surface.wl_surface())
        .into_iter()
        .filter_map(|rect| rect.intersection(clip))
        .map(|rect| {
            let x = (transform.x + (surface_offset.x + rect.loc.x) as f64 * transform.scale).round()
                as i32;
            let y = (transform.y
                + (titlebar_height + surface_offset.y + rect.loc.y) as f64 * transform.scale)
                .round() as i32;
            let width = (rect.size.w as f64 * transform.scale).round().max(1.0) as i32;
            let height = (rect.size.h as f64 * transform.scale).round().max(1.0) as i32;
            Rectangle::<i32, Logical>::new((x, y).into(), (width, height).into())
        })
        .collect()
}

fn current_opaque_rects(surface: &WlSurface) -> Vec<Rectangle<i32, Logical>> {
    let Some(region) = compositor::with_states(surface, |states| {
        states
            .cached_state
            .get::<SurfaceAttributes>()
            .current()
            .opaque_region
            .clone()
    }) else {
        return Vec::new();
    };

    let mut rects = Vec::new();
    for (kind, rect) in region.rects {
        match kind {
            RectangleKind::Add => rects.push(rect),
            RectangleKind::Subtract => {
                rects.retain(|current: &Rectangle<i32, Logical>| {
                    current.intersection(rect).is_none()
                });
            }
        }
    }
    rects
}

fn rect_contains(outer: Rectangle<i32, Logical>, inner: Rectangle<i32, Logical>) -> bool {
    inner.loc.x >= outer.loc.x
        && inner.loc.y >= outer.loc.y
        && inner.loc.x + inner.size.w <= outer.loc.x + outer.size.w
        && inner.loc.y + inner.size.h <= outer.loc.y + outer.size.h
}

fn append_surface_region_targets(
    surface: &WlSurface,
    blur_layer: BlurLayer,
    location: Point<i32, Logical>,
    size: smithay::utils::Size<i32, Logical>,
    material: LayerMaterial,
    targets: &mut Vec<LayerRenderTarget>,
) {
    let Some(region) = current_blur_region(surface) else {
        return;
    };
    for target in targets_for_region(&region, Rectangle::from_size(size)) {
        targets.push(LayerRenderTarget {
            surface: surface.clone(),
            blur_layer,
            material: if target.material == LayerMaterial::Rect {
                material
            } else {
                target.material
            },
            opacity: 1.0,
            location: (
                location.x + target.rect.loc.x,
                location.y + target.rect.loc.y,
            )
                .into(),
            size: target.rect.size,
        });
    }
}

fn titlebar_radius(window: &ManagedWindow, scale: f64) -> i32 {
    if window.flat_frame_corners() {
        0
    } else {
        ((WINDOW_RADIUS as f64) * scale).round().max(1.0) as i32
    }
}

fn scaled_material(material: LayerMaterial, scale: f64) -> LayerMaterial {
    let scale_radius = |radius: i32| ((radius as f64) * scale).round().max(1.0) as i32;
    match material {
        LayerMaterial::Rect => LayerMaterial::Rect,
        LayerMaterial::RoundRect { radius } => LayerMaterial::RoundRect {
            radius: scale_radius(radius),
        },
        LayerMaterial::RoundTop { radius } => LayerMaterial::RoundTop {
            radius: scale_radius(radius),
        },
        LayerMaterial::RoundLeft { radius } => LayerMaterial::RoundLeft {
            radius: scale_radius(radius),
        },
        LayerMaterial::RoundRight { radius } => LayerMaterial::RoundRight {
            radius: scale_radius(radius),
        },
    }
}

fn rects_for_region(
    region: &RegionAttributes,
    clip: Rectangle<i32, Logical>,
) -> Vec<Rectangle<i32, Logical>> {
    let mut rects = Vec::new();
    for (kind, rect) in &region.rects {
        let Some(rect) = rect.intersection(clip) else {
            continue;
        };
        match kind {
            RectangleKind::Add => push_rect(&mut rects, rect),
            RectangleKind::Subtract => subtract_rect(&mut rects, rect),
        }
    }
    rects
}

struct RegionTarget {
    rect: Rectangle<i32, Logical>,
    material: LayerMaterial,
}

fn targets_for_region(
    region: &RegionAttributes,
    clip: Rectangle<i32, Logical>,
) -> Vec<RegionTarget> {
    let rects = rects_for_region(region, clip);
    if let Some(target) = coalesced_region_target(&rects, clip) {
        return vec![target];
    }

    rects
        .into_iter()
        .map(|rect| RegionTarget {
            rect,
            material: LayerMaterial::Rect,
        })
        .collect()
}

fn coalesced_region_target(
    rects: &[Rectangle<i32, Logical>],
    clip: Rectangle<i32, Logical>,
) -> Option<RegionTarget> {
    if rects.len() < 8 {
        return None;
    }

    let mut rows = rects.to_vec();
    rows.sort_by_key(|rect| (rect.loc.y, rect.loc.x));
    if rows.iter().any(|rect| rect.size.h != 1) {
        return None;
    }

    let bounds = bounding_rect(&rows)?;
    if rows.len() != bounds.size.h as usize {
        return None;
    }
    for (index, row) in rows.iter().enumerate() {
        if row.loc.y != bounds.loc.y + index as i32 {
            return None;
        }
    }

    let right = bounds.loc.x + bounds.size.w;
    let clip_right = clip.loc.x + clip.size.w;
    let radius = rows
        .iter()
        .map(|row| {
            let left_inset = row.loc.x - bounds.loc.x;
            let right_inset = right - (row.loc.x + row.size.w);
            left_inset.max(right_inset)
        })
        .max()
        .unwrap_or(0);
    if radius <= 0 {
        return None;
    }

    let material = if bounds == clip
        && rows.iter().all(|row| {
            let left_inset = row.loc.x - bounds.loc.x;
            let right_inset = right - (row.loc.x + row.size.w);
            left_inset == right_inset
        }) {
        LayerMaterial::RoundRect { radius }
    } else if bounds.loc.x == clip.loc.x && rows.iter().all(|row| row.loc.x + row.size.w == right) {
        LayerMaterial::RoundLeft { radius }
    } else if right == clip_right && rows.iter().all(|row| row.loc.x == bounds.loc.x) {
        LayerMaterial::RoundRight { radius }
    } else {
        return None;
    };

    Some(RegionTarget {
        rect: bounds,
        material,
    })
}

fn bounding_rect(rects: &[Rectangle<i32, Logical>]) -> Option<Rectangle<i32, Logical>> {
    let first = rects.first()?;
    let mut left = first.loc.x;
    let mut top = first.loc.y;
    let mut right = first.loc.x + first.size.w;
    let mut bottom = first.loc.y + first.size.h;
    for rect in &rects[1..] {
        left = left.min(rect.loc.x);
        top = top.min(rect.loc.y);
        right = right.max(rect.loc.x + rect.size.w);
        bottom = bottom.max(rect.loc.y + rect.size.h);
    }
    Some(Rectangle::new(
        (left, top).into(),
        (right - left, bottom - top).into(),
    ))
}

fn subtract_rect(rects: &mut Vec<Rectangle<i32, Logical>>, cut: Rectangle<i32, Logical>) {
    let current = std::mem::take(rects);
    for rect in current {
        append_subtracted(rects, rect, cut);
    }
}

fn append_subtracted(
    rects: &mut Vec<Rectangle<i32, Logical>>,
    rect: Rectangle<i32, Logical>,
    cut: Rectangle<i32, Logical>,
) {
    let Some(hit) = rect.intersection(cut) else {
        push_rect(rects, rect);
        return;
    };

    let left = rect.loc.x;
    let right = rect.loc.x + rect.size.w;
    let top = rect.loc.y;
    let bottom = rect.loc.y + rect.size.h;
    let hit_left = hit.loc.x;
    let hit_right = hit.loc.x + hit.size.w;
    let hit_top = hit.loc.y;
    let hit_bottom = hit.loc.y + hit.size.h;

    push_piece(rects, left, top, rect.size.w, hit_top - top);
    push_piece(rects, left, hit_bottom, rect.size.w, bottom - hit_bottom);
    push_piece(rects, left, hit_top, hit_left - left, hit.size.h);
    push_piece(rects, hit_right, hit_top, right - hit_right, hit.size.h);
}

fn push_piece(rects: &mut Vec<Rectangle<i32, Logical>>, x: i32, y: i32, width: i32, height: i32) {
    push_rect(rects, Rectangle::new((x, y).into(), (width, height).into()));
}

fn push_rect(rects: &mut Vec<Rectangle<i32, Logical>>, rect: Rectangle<i32, Logical>) {
    if rect.size.w > 0 && rect.size.h > 0 {
        rects.push(rect);
    }
}
