use crate::{
    render::LayerElement,
    scene_blur::FramebufferBlurElement,
    window_clip::RoundedWindowElement,
};
use smithay::{
    backend::renderer::{
        element::{
            Element, Id, Kind, RenderElement, UnderlyingStorage,
            memory::MemoryRenderBufferRenderElement,
            surface::WaylandSurfaceRenderElement,
        },
        gles::{GlesError, GlesRenderer},
        utils::{CommitCounter, DamageSet, OpaqueRegions},
    },
    utils::{user_data::UserDataMap, Buffer, Physical, Rectangle, Scale, Transform},
};

type LayerSurfaceElement = LayerElement;
type WindowSurfaceElement = WaylandSurfaceRenderElement<GlesRenderer>;
type MemoryElement = MemoryRenderBufferRenderElement<GlesRenderer>;
type WindowElement = RoundedWindowElement<WindowSurfaceElement>;

pub struct WindowSceneLayerRef<'a> {
    pub chrome: &'a [MemoryElement],
    pub surfaces: &'a [WindowElement],
    pub blurs: &'a [FramebufferBlurElement],
}

#[derive(Clone, Copy)]
pub enum SceneCompositeElement<'a> {
    Surface(&'a LayerSurfaceElement),
    Memory(&'a MemoryElement),
    Window(&'a WindowElement),
    Blur(&'a FramebufferBlurElement),
}

impl Element for SceneCompositeElement<'_> {
    fn id(&self) -> &Id {
        match self {
            Self::Surface(element) => element.id(),
            Self::Memory(element) => element.id(),
            Self::Window(element) => element.id(),
            Self::Blur(element) => element.id(),
        }
    }

    fn current_commit(&self) -> CommitCounter {
        match self {
            Self::Surface(element) => element.current_commit(),
            Self::Memory(element) => element.current_commit(),
            Self::Window(element) => element.current_commit(),
            Self::Blur(element) => element.current_commit(),
        }
    }

    fn src(&self) -> Rectangle<f64, Buffer> {
        match self {
            Self::Surface(element) => element.src(),
            Self::Memory(element) => element.src(),
            Self::Window(element) => element.src(),
            Self::Blur(element) => element.src(),
        }
    }

    fn transform(&self) -> Transform {
        match self {
            Self::Surface(element) => element.transform(),
            Self::Memory(element) => element.transform(),
            Self::Window(element) => element.transform(),
            Self::Blur(element) => element.transform(),
        }
    }

    fn geometry(&self, scale: Scale<f64>) -> Rectangle<i32, Physical> {
        match self {
            Self::Surface(element) => element.geometry(scale),
            Self::Memory(element) => element.geometry(scale),
            Self::Window(element) => element.geometry(scale),
            Self::Blur(element) => element.geometry(scale),
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
            Self::Blur(element) => element.damage_since(scale, commit),
        }
    }

    fn opaque_regions(&self, scale: Scale<f64>) -> OpaqueRegions<i32, Physical> {
        match self {
            Self::Surface(element) => element.opaque_regions(scale),
            Self::Memory(element) => element.opaque_regions(scale),
            Self::Window(element) => element.opaque_regions(scale),
            Self::Blur(element) => element.opaque_regions(scale),
        }
    }

    fn alpha(&self) -> f32 {
        match self {
            Self::Surface(element) => element.alpha(),
            Self::Memory(element) => element.alpha(),
            Self::Window(element) => element.alpha(),
            Self::Blur(element) => element.alpha(),
        }
    }

    fn kind(&self) -> Kind {
        match self {
            Self::Surface(element) => element.kind(),
            Self::Memory(element) => element.kind(),
            Self::Window(element) => element.kind(),
            Self::Blur(element) => element.kind(),
        }
    }

    fn is_framebuffer_effect(&self) -> bool {
        match self {
            Self::Surface(element) => element.is_framebuffer_effect(),
            Self::Memory(element) => element.is_framebuffer_effect(),
            Self::Window(element) => element.is_framebuffer_effect(),
            Self::Blur(element) => element.is_framebuffer_effect(),
        }
    }
}

impl RenderElement<GlesRenderer> for SceneCompositeElement<'_> {
    fn capture_framebuffer(
        &self,
        frame: &mut <GlesRenderer as smithay::backend::renderer::RendererSuper>::Frame<'_, '_>,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        cache: &UserDataMap,
    ) -> Result<(), GlesError> {
        match self {
            Self::Blur(element) => {
                RenderElement::<GlesRenderer>::capture_framebuffer(element, frame, src, dst, cache)
            }
            _ => Ok(()),
        }
    }

    fn draw(
        &self,
        frame: &mut <GlesRenderer as smithay::backend::renderer::RendererSuper>::Frame<'_, '_>,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        opaque_regions: &[Rectangle<i32, Physical>],
        cache: Option<&UserDataMap>,
    ) -> Result<(), GlesError> {
        match self {
            Self::Surface(element) => RenderElement::<GlesRenderer>::draw(
                element, frame, src, dst, damage, opaque_regions, cache,
            ),
            Self::Memory(element) => RenderElement::<GlesRenderer>::draw(
                element, frame, src, dst, damage, opaque_regions, cache,
            ),
            Self::Window(element) => RenderElement::<GlesRenderer>::draw(
                element, frame, src, dst, damage, opaque_regions, cache,
            ),
            Self::Blur(element) => RenderElement::<GlesRenderer>::draw(
                element, frame, src, dst, damage, opaque_regions, cache,
            ),
        }
    }

    fn underlying_storage(
        &self,
        renderer: &mut GlesRenderer,
    ) -> Option<UnderlyingStorage<'_>> {
        match self {
            Self::Surface(element) => element.underlying_storage(renderer),
            Self::Memory(element) => element.underlying_storage(renderer),
            Self::Window(element) => element.underlying_storage(renderer),
            Self::Blur(element) => element.underlying_storage(renderer),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn scene_elements<'a>(
    background: Option<&'a MemoryElement>,
    background_layer: &'a [LayerSurfaceElement],
    bottom_layer: &'a [LayerSurfaceElement],
    window_layers: &'a [WindowSceneLayerRef<'a>],
    top_blurs: &'a [FramebufferBlurElement],
    top_layer: &'a [LayerSurfaceElement],
    overlay_blurs: &'a [FramebufferBlurElement],
    overlay_layer: &'a [LayerSurfaceElement],
) -> Vec<SceneCompositeElement<'a>> {
    let window_count = window_layers
        .iter()
        .map(|layer| layer.chrome.len() + layer.surfaces.len() + layer.blurs.len())
        .sum::<usize>();
    let mut elements = Vec::with_capacity(
        overlay_layer.len()
            + overlay_blurs.len()
            + top_layer.len()
            + top_blurs.len()
            + window_count
            + bottom_layer.len()
            + background_layer.len()
            + usize::from(background.is_some()),
    );
    elements.extend(overlay_layer.iter().map(SceneCompositeElement::Surface));
    elements.extend(overlay_blurs.iter().map(SceneCompositeElement::Blur));
    elements.extend(top_layer.iter().map(SceneCompositeElement::Surface));
    elements.extend(top_blurs.iter().map(SceneCompositeElement::Blur));
    for layer in window_layers {
        elements.extend(layer.chrome.iter().map(SceneCompositeElement::Memory));
        elements.extend(layer.surfaces.iter().map(SceneCompositeElement::Window));
        elements.extend(layer.blurs.iter().map(SceneCompositeElement::Blur));
    }
    elements.extend(bottom_layer.iter().map(SceneCompositeElement::Surface));
    elements.extend(background_layer.iter().map(SceneCompositeElement::Surface));
    if let Some(background) = background {
        elements.push(SceneCompositeElement::Memory(background));
    }
    elements
}

pub fn scene_backdrop_elements<'a>(
    background: Option<&'a MemoryElement>,
    background_layer: &'a [LayerSurfaceElement],
    bottom_layer: &'a [LayerSurfaceElement],
    window_layers: &'a [WindowSceneLayerRef<'a>],
) -> Vec<SceneCompositeElement<'a>> {
    scene_elements(
        background,
        background_layer,
        bottom_layer,
        window_layers,
        &[],
        &[],
        &[],
        &[],
    )
}
