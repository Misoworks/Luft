use crate::{
    layers::{BlurLayer, LayerMaterial, LayerRenderTarget, material_for},
    state::KestrelState,
    window::ManagedWindow,
    window_clip::WINDOW_RADIUS,
};
use asher_ipc::WorkspaceId;
use smithay::{
    desktop::{PopupManager, layer_map_for_output},
    reexports::{
        wayland_protocols::ext::background_effect::v1::server::{
            ext_background_effect_manager_v1::{self, Capability, ExtBackgroundEffectManagerV1},
            ext_background_effect_surface_v1::{self, ExtBackgroundEffectSurfaceV1},
        },
        wayland_server::{
            Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource, Weak,
            backend::{ClientId, GlobalId},
            protocol::wl_surface::WlSurface,
        },
    },
    utils::{Logical, Point, Rectangle},
    wayland::{
        compositor::{self, Cacheable, RectangleKind, RegionAttributes},
        shell::wlr_layer::Layer,
    },
};
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

#[derive(Debug)]
pub struct BackgroundEffectGlobal {
    _global: GlobalId,
}

impl BackgroundEffectGlobal {
    pub fn new(display: &DisplayHandle) -> Self {
        Self {
            _global: display.create_global::<KestrelState, ExtBackgroundEffectManagerV1, _>(1, ()),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct BackgroundEffectSurfaceState {
    blur_region: Option<RegionAttributes>,
}

impl Cacheable for BackgroundEffectSurfaceState {
    fn commit(&mut self, _dh: &DisplayHandle) -> Self {
        self.clone()
    }

    fn merge_into(self, into: &mut Self, _dh: &DisplayHandle) {
        *into = self;
    }
}

#[derive(Debug)]
struct BackgroundEffectSurfaceData {
    attached: AtomicBool,
}

impl BackgroundEffectSurfaceData {
    fn new() -> Self {
        Self {
            attached: AtomicBool::new(false),
        }
    }

    fn is_attached(&self) -> bool {
        self.attached.load(Ordering::Acquire)
    }

    fn set_attached(&self, attached: bool) {
        self.attached.store(attached, Ordering::Release);
    }
}

#[derive(Debug)]
pub struct BackgroundEffectSurfaceUserData(Mutex<Weak<WlSurface>>);

impl BackgroundEffectSurfaceUserData {
    fn new(surface: WlSurface) -> Self {
        Self(Mutex::new(surface.downgrade()))
    }

    fn surface(&self) -> Option<WlSurface> {
        self.0.lock().unwrap().upgrade().ok()
    }
}

impl GlobalDispatch<ExtBackgroundEffectManagerV1, ()> for KestrelState {
    fn bind(
        _state: &mut Self,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<ExtBackgroundEffectManagerV1>,
        _global_data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        let manager = data_init.init(resource, ());
        manager.capabilities(Capability::Blur);
    }
}

impl Dispatch<ExtBackgroundEffectManagerV1, ()> for KestrelState {
    fn request(
        _state: &mut Self,
        _client: &Client,
        manager: &ExtBackgroundEffectManagerV1,
        request: ext_background_effect_manager_v1::Request,
        _data: &(),
        _handle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            ext_background_effect_manager_v1::Request::GetBackgroundEffect { id, surface } => {
                let already_attached = compositor::with_states(&surface, |states| {
                    states
                        .data_map
                        .insert_if_missing_threadsafe(BackgroundEffectSurfaceData::new);
                    let data = states
                        .data_map
                        .get::<BackgroundEffectSurfaceData>()
                        .unwrap();
                    let already_attached = data.is_attached();

                    if !already_attached {
                        data.set_attached(true);
                        drop(states.cached_state.get::<BackgroundEffectSurfaceState>());
                    }

                    already_attached
                });

                if already_attached {
                    manager.post_error(
                        ext_background_effect_manager_v1::Error::BackgroundEffectExists,
                        "wl_surface already has a background effect object",
                    );
                    return;
                }

                data_init.init(id, BackgroundEffectSurfaceUserData::new(surface));
            }
            ext_background_effect_manager_v1::Request::Destroy => {}
            _ => {}
        }
    }
}

impl Dispatch<ExtBackgroundEffectSurfaceV1, BackgroundEffectSurfaceUserData> for KestrelState {
    fn request(
        _state: &mut Self,
        _client: &Client,
        resource: &ExtBackgroundEffectSurfaceV1,
        request: ext_background_effect_surface_v1::Request,
        data: &BackgroundEffectSurfaceUserData,
        _handle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            ext_background_effect_surface_v1::Request::SetBlurRegion { region } => {
                let Some(surface) = data.surface() else {
                    resource.post_error(
                        ext_background_effect_surface_v1::Error::SurfaceDestroyed,
                        "associated wl_surface has been destroyed",
                    );
                    return;
                };
                let region = region.map(|region| compositor::get_region_attributes(&region));
                compositor::with_states(&surface, |states| {
                    states
                        .cached_state
                        .get::<BackgroundEffectSurfaceState>()
                        .pending()
                        .blur_region = region;
                });
            }
            ext_background_effect_surface_v1::Request::Destroy => {
                if let Some(surface) = data.surface() {
                    compositor::with_states(&surface, |states| {
                        if let Some(data) = states.data_map.get::<BackgroundEffectSurfaceData>() {
                            data.set_attached(false);
                        }
                        states
                            .cached_state
                            .get::<BackgroundEffectSurfaceState>()
                            .pending()
                            .blur_region = None;
                    });
                }
            }
            _ => {}
        }
    }

    fn destroyed(
        _state: &mut Self,
        _client: ClientId,
        _object: &ExtBackgroundEffectSurfaceV1,
        _data: &BackgroundEffectSurfaceUserData,
    ) {
    }
}

pub fn window_blur_targets(state: &KestrelState) -> Vec<LayerRenderTarget> {
    let mut targets = Vec::new();
    if let Some(transition) = state.workspace_transition() {
        let width = state.output_size().w as f64;
        let direction = transition.direction as f64;
        let from_offset = (-direction * width * transition.progress).round() as i32;
        let to_offset = (direction * width * (1.0 - transition.progress)).round() as i32;
        append_workspace_targets(state, &transition.from, from_offset, &mut targets);
        append_workspace_targets(state, &transition.to, to_offset, &mut targets);
    } else {
        append_workspace_targets(state, state.layout.active_workspace(), 0, &mut targets);
    }
    targets
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

fn append_workspace_targets(
    state: &KestrelState,
    workspace: &WorkspaceId,
    offset_x: i32,
    targets: &mut Vec<LayerRenderTarget>,
) {
    for window in state.windows.render_windows_on_workspace(workspace) {
        let transform = window.render_transform(offset_x, state.output_size());
        let surface = window.surface.wl_surface();
        let titlebar_height = window.titlebar_height();
        let surface_offset = window.surface_offset();
        if titlebar_height > 0 {
            targets.push(LayerRenderTarget {
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
            });
        }
        let Some(region) = current_blur_region(surface) else {
            continue;
        };
        let clip = Rectangle::from_size(window.surface_geometry().size);
        for target in targets_for_region(&region, clip) {
            let location = Point::from((
                (transform.x + (surface_offset.x + target.rect.loc.x) as f64 * transform.scale)
                    .round() as i32,
                (transform.y
                    + (titlebar_height + surface_offset.y + target.rect.loc.y) as f64
                        * transform.scale)
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
            targets.push(LayerRenderTarget {
                surface: surface.clone(),
                blur_layer: BlurLayer::Window,
                material: scaled_material(target.material, transform.scale),
                opacity: 1.0,
                location,
                size,
            });
        }
    }
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

pub(crate) fn current_blur_region(surface: &WlSurface) -> Option<RegionAttributes> {
    compositor::with_states(surface, |states| {
        if !states.cached_state.has::<BackgroundEffectSurfaceState>() {
            return None;
        }

        states
            .cached_state
            .get::<BackgroundEffectSurfaceState>()
            .current()
            .blur_region
            .clone()
    })
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
