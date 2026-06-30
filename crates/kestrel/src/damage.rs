use crate::{
    layers::LayerRenderTarget, render::LayerElement, scene_blur::blur_sample_rect,
    window_clip::RoundedWindowElement,
};
use smithay::{
    backend::renderer::{
        damage::OutputDamageTracker,
        element::{
            Element, Id, Kind, memory::MemoryRenderBufferRenderElement,
            surface::WaylandSurfaceRenderElement,
        },
        gles::GlesRenderer,
        utils::{CommitCounter, DamageSet, OpaqueRegions},
    },
    reexports::wayland_server::{Resource, protocol::wl_surface::WlSurface},
    utils::{Buffer, Physical, Rectangle, Scale, Size, Transform},
};
use std::collections::HashMap;

type LayerSurfaceElement = LayerElement;
type WindowSurfaceElement = WaylandSurfaceRenderElement<GlesRenderer>;
type MemoryElement = MemoryRenderBufferRenderElement<GlesRenderer>;
type WindowElement = RoundedWindowElement<WindowSurfaceElement>;

#[derive(Debug)]
pub struct DamageTracker {
    output_size: Size<i32, Physical>,
    transform: Transform,
    tracker: OutputDamageTracker,
}

#[derive(Debug)]
pub struct DamagePlan {
    pub rectangles: Vec<Rectangle<i32, Physical>>,
}

impl DamageTracker {
    #[allow(dead_code)]
    pub fn new(output_size: Size<i32, Physical>, transform: Transform) -> Self {
        Self {
            output_size,
            transform,
            tracker: output_tracker(output_size, transform),
        }
    }

    pub fn from_output(output: &smithay::output::Output) -> Self {
        let output_size = output
            .current_mode()
            .map(|mode| mode.size)
            .unwrap_or_default();
        Self {
            output_size,
            transform: output.current_transform(),
            tracker: OutputDamageTracker::from_output(output),
        }
    }

    pub fn plan(
        &mut self,
        output_size: Size<i32, Physical>,
        buffer_age: usize,
        force_full: bool,
        elements: &[DamageElement<'_>],
    ) -> DamagePlan {
        if self.output_size != output_size {
            self.output_size = output_size;
            self.tracker = output_tracker(output_size, self.transform);
        }

        let rectangles = self
            .tracker
            .damage_output(buffer_age, elements)
            .ok()
            .and_then(|(damage, _states)| damage.cloned())
            .filter(|damage| !damage.is_empty())
            .unwrap_or_default();

        DamagePlan {
            rectangles: if force_full {
                vec![full_damage(output_size)]
            } else {
                rectangles
            },
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum DamageElement<'a> {
    Surface(&'a LayerSurfaceElement),
    Memory(&'a MemoryElement),
    Window(&'a WindowElement),
}

impl Element for DamageElement<'_> {
    fn id(&self) -> &Id {
        match self {
            Self::Surface(element) => element.id(),
            Self::Memory(element) => element.id(),
            Self::Window(element) => element.id(),
        }
    }

    fn current_commit(&self) -> CommitCounter {
        match self {
            Self::Surface(element) => element.current_commit(),
            Self::Memory(element) => element.current_commit(),
            Self::Window(element) => element.current_commit(),
        }
    }

    fn src(&self) -> Rectangle<f64, Buffer> {
        match self {
            Self::Surface(element) => element.src(),
            Self::Memory(element) => element.src(),
            Self::Window(element) => element.src(),
        }
    }

    fn transform(&self) -> Transform {
        match self {
            Self::Surface(element) => element.transform(),
            Self::Memory(element) => element.transform(),
            Self::Window(element) => element.transform(),
        }
    }

    fn geometry(&self, scale: Scale<f64>) -> Rectangle<i32, Physical> {
        match self {
            Self::Surface(element) => element.geometry(scale),
            Self::Memory(element) => element.geometry(scale),
            Self::Window(element) => element.geometry(scale),
        }
    }

    fn damage_since(
        &self,
        scale: Scale<f64>,
        commit: Option<CommitCounter>,
    ) -> DamageSet<i32, Physical> {
        match self {
            Self::Surface(element) => element.damage_since(scale, commit),
            Self::Memory(element) => element.damage_since(scale, commit),
            Self::Window(element) => element.damage_since(scale, commit),
        }
    }

    fn opaque_regions(&self, scale: Scale<f64>) -> OpaqueRegions<i32, Physical> {
        match self {
            Self::Surface(element) => element.opaque_regions(scale),
            Self::Memory(element) => element.opaque_regions(scale),
            Self::Window(element) => element.opaque_regions(scale),
        }
    }

    fn alpha(&self) -> f32 {
        match self {
            Self::Surface(element) => element.alpha(),
            Self::Memory(element) => element.alpha(),
            Self::Window(element) => element.alpha(),
        }
    }

    fn kind(&self) -> Kind {
        match self {
            Self::Surface(element) => element.kind(),
            Self::Memory(element) => element.kind(),
            Self::Window(element) => element.kind(),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn damage_elements<'a>(
    background: Option<&'a MemoryElement>,
    background_layer: &'a [LayerSurfaceElement],
    bottom_layer: &'a [LayerSurfaceElement],
    windows: &'a [WindowElement],
    window_chrome: &'a [MemoryElement],
    top_layer: &'a [LayerSurfaceElement],
    overlay_layer: &'a [LayerSurfaceElement],
    loading: Option<&'a MemoryElement>,
    debug: Option<&'a MemoryElement>,
) -> Vec<DamageElement<'a>> {
    let mut elements = Vec::new();
    if let Some(debug) = debug {
        elements.push(DamageElement::Memory(debug));
    }
    if let Some(loading) = loading {
        elements.push(DamageElement::Memory(loading));
    }
    elements.extend(overlay_layer.iter().map(DamageElement::Surface));
    elements.extend(top_layer.iter().map(DamageElement::Surface));
    elements.extend(window_chrome.iter().map(DamageElement::Memory));
    elements.extend(windows.iter().map(DamageElement::Window));
    elements.extend(bottom_layer.iter().map(DamageElement::Surface));
    elements.extend(background_layer.iter().map(DamageElement::Surface));
    if let Some(background) = background {
        elements.push(DamageElement::Memory(background));
    }
    elements
}

pub(crate) fn blur_damage_elements<'a>(
    background: Option<&'a MemoryElement>,
    background_layer: &'a [LayerSurfaceElement],
    bottom_layer: &'a [LayerSurfaceElement],
    windows: &'a [WindowElement],
) -> Vec<DamageElement<'a>> {
    let mut elements = Vec::new();
    elements.extend(windows.iter().map(DamageElement::Window));
    elements.extend(bottom_layer.iter().map(DamageElement::Surface));
    elements.extend(background_layer.iter().map(DamageElement::Surface));
    if let Some(background) = background {
        elements.push(DamageElement::Memory(background));
    }
    elements
}

pub fn damage_area(rectangles: &[Rectangle<i32, Physical>]) -> i32 {
    rectangles
        .iter()
        .map(|rect| rect.size.w.saturating_mul(rect.size.h))
        .fold(0, i32::saturating_add)
}

pub(crate) fn expand_damage_for_blur_targets(
    output_size: Size<i32, Physical>,
    damage: &[Rectangle<i32, Physical>],
    blur_damage: &[Rectangle<i32, Physical>],
    target_groups: &[&[LayerRenderTarget]],
) -> Vec<Rectangle<i32, Physical>> {
    let mut expanded = damage.to_vec();
    let output = Rectangle::<i32, Physical>::from_size(output_size);
    for target in target_groups.iter().flat_map(|targets| targets.iter()) {
        if let Some(rect) =
            blur_sample_rect(output_size, target).and_then(|rect| rect.intersection(output))
        {
            let target_changed = damage
                .iter()
                .any(|damage| damage.intersection(rect).is_some());
            let behind_target_changed = blur_damage
                .iter()
                .any(|damage| damage.intersection(rect).is_some());
            if target_changed || behind_target_changed {
                expanded.push(rect);
            }
        }
    }
    expanded
}

fn full_damage(output_size: Size<i32, Physical>) -> Rectangle<i32, Physical> {
    Rectangle::from_size(output_size)
}

fn output_tracker(output_size: Size<i32, Physical>, transform: Transform) -> OutputDamageTracker {
    OutputDamageTracker::new(output_size, 1.0, transform)
}

#[derive(Debug, Default)]
pub struct LayerGeometryTracker {
    previous: HashMap<u32, Rectangle<i32, Physical>>,
}

impl LayerGeometryTracker {
    pub fn geometry_changed(
        &self,
        output_size: Size<i32, Physical>,
        surfaces: &[(WlSurface, Rectangle<i32, Physical>)],
    ) -> bool {
        let output = Rectangle::<i32, Physical>::from_size(output_size);
        surfaces.iter().any(|(surface, rect)| {
            self.previous
                .get(&surface.id().protocol_id())
                .is_some_and(|previous| {
                    previous != rect
                        && (previous.intersection(output).is_some()
                            || rect.intersection(output).is_some())
                })
        })
    }

    pub fn expand_damage(
        &mut self,
        output_size: Size<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        surfaces: &[(WlSurface, Rectangle<i32, Physical>)],
    ) -> (Vec<Rectangle<i32, Physical>>, bool) {
        let output = Rectangle::<i32, Physical>::from_size(output_size);
        let mut expanded = damage.to_vec();
        let mut geometry_changed = false;
        let mut active = std::collections::HashSet::new();

        for (surface, rect) in surfaces {
            let id = surface.id().protocol_id();
            active.insert(id);
            if let Some(previous) = self.previous.get(&id)
                && previous != rect
            {
                geometry_changed = true;
                if let Some(previous) = previous.intersection(output) {
                    expanded.push(previous);
                }
                if let Some(current) = rect.intersection(output) {
                    expanded.push(current);
                }
            }
            self.previous.insert(id, *rect);
        }

        for (id, previous) in &self.previous {
            if active.contains(id) {
                continue;
            }
            geometry_changed = true;
            if let Some(previous) = previous.intersection(output) {
                expanded.push(previous);
            }
        }
        self.previous.retain(|id, _| active.contains(id));
        (merge_damage_rectangles(output, expanded), geometry_changed)
    }
}

pub(crate) fn merge_damage_rectangles(
    output: Rectangle<i32, Physical>,
    damage: Vec<Rectangle<i32, Physical>>,
) -> Vec<Rectangle<i32, Physical>> {
    let mut shaped: Vec<Rectangle<i32, Physical>> = Vec::new();
    for rect in damage {
        let Some(mut rect) = rect.intersection(output) else {
            continue;
        };
        if rect.is_empty() {
            continue;
        }
        let mut index = 0;
        while index < shaped.len() {
            if shaped[index].overlaps_or_touches(rect) {
                rect = rect.merge(shaped.swap_remove(index));
            } else {
                index += 1;
            }
        }
        shaped.push(rect);
    }
    shaped
}
