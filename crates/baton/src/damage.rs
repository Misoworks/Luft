use crate::window_clip::RoundedWindowElement;
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
    utils::{Buffer, Physical, Rectangle, Scale, Size, Transform},
};

type SurfaceElement = WaylandSurfaceRenderElement<GlesRenderer>;
type MemoryElement = MemoryRenderBufferRenderElement<GlesRenderer>;
type WindowElement = RoundedWindowElement<SurfaceElement>;

#[derive(Debug)]
pub struct DamageTracker {
    output_size: Size<i32, Physical>,
    tracker: OutputDamageTracker,
}

#[derive(Debug)]
pub struct DamagePlan {
    pub rectangles: Vec<Rectangle<i32, Physical>>,
}

impl DamageTracker {
    pub fn new(output_size: Size<i32, Physical>) -> Self {
        Self {
            output_size,
            tracker: output_tracker(output_size),
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
            self.tracker = output_tracker(output_size);
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

    pub fn record(&mut self, _plan: DamagePlan) {}
}

#[derive(Clone, Copy)]
pub(crate) enum DamageElement<'a> {
    Surface(&'a SurfaceElement),
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
    background_layer: &'a [SurfaceElement],
    bottom_layer: &'a [SurfaceElement],
    windows: &'a [WindowElement],
    window_chrome: &'a [MemoryElement],
    top_layer: &'a [SurfaceElement],
    overlay_layer: &'a [SurfaceElement],
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
    background_layer: &'a [SurfaceElement],
    bottom_layer: &'a [SurfaceElement],
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

fn full_damage(output_size: Size<i32, Physical>) -> Rectangle<i32, Physical> {
    Rectangle::from_size(output_size)
}

fn output_tracker(output_size: Size<i32, Physical>) -> OutputDamageTracker {
    OutputDamageTracker::new(output_size, 1.0, Transform::Normal)
}
