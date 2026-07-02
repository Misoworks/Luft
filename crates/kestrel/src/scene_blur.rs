use crate::{
    layers::{BlurLayer, LayerMaterial, LayerRenderTarget},
    window_clip::RoundedWindowElement,
};
use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            Bind, Blit, Offscreen, Renderer, TextureFilter,
            element::{Id, Kind, texture::TextureRenderElement},
            gles::{
                GlesError, GlesRenderer, GlesTarget, GlesTexProgram, GlesTexture, UniformName,
                UniformType,
            },
        },
    },
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Buffer, Logical, Physical, Point, Rectangle, Size, Transform},
};
use std::time::Instant;

mod geometry;
mod render_pass;

use geometry::{
    blur_texture_size, clipped_target_rect, material_clip_shape, padded_target_rect,
    source_rect_for_visible_target, target_is_damaged,
};
use render_pass::{BlurRenderPass, render_blur_texture};

const BLUR_SHADER: &str = include_str!("shaders/scene_blur.frag");

pub(crate) fn blur_sample_rect(
    output_size: Size<i32, Physical>,
    target: &LayerRenderTarget,
) -> Option<Rectangle<i32, Physical>> {
    let rect = clipped_target_rect(output_size, target)?;
    Some(padded_target_rect(output_size, target, rect))
}

#[derive(Default)]
pub struct SceneBlurCache {
    entries: Vec<SceneBlurCacheEntry>,
    program: Option<GlesTexProgram>,
    animating: bool,
}

impl SceneBlurCache {
    pub fn retain_targets(&mut self, targets: &[LayerRenderTarget]) {
        self.animating = false;
        for entry in &mut self.entries {
            if let Some(target) = targets
                .iter()
                .find(|target| target.size.w > 1 && target.size.h > 1 && entry.matches(target))
            {
                if entry.location != target.location || entry.target_opacity != target.opacity {
                    self.animating = true;
                    entry.rect = Rectangle::<i32, Physical>::new((0, 0).into(), (0, 0).into());
                }
                entry.location = target.location;
                entry.target_opacity = target.opacity;
            }
        }
        self.entries
            .retain(|entry| targets.iter().any(|target| entry.matches(target)));
    }

    pub fn is_animating(&self) -> bool {
        self.animating
    }

    pub fn has_cached_elements(&self) -> bool {
        !self.entries.is_empty()
    }

    pub fn targets_need_capture(
        &self,
        output_size: Size<i32, Physical>,
        targets: &[LayerRenderTarget],
        damage: &[Rectangle<i32, Physical>],
    ) -> bool {
        targets.iter().any(|target| {
            self.target_capture_sample_rect(output_size, target, damage)
                .is_some()
        })
    }

    pub(crate) fn target_capture_sample_rects(
        &self,
        output_size: Size<i32, Physical>,
        targets: &[LayerRenderTarget],
        damage: &[Rectangle<i32, Physical>],
    ) -> Vec<Rectangle<i32, Physical>> {
        targets
            .iter()
            .filter_map(|target| self.target_capture_sample_rect(output_size, target, damage))
            .collect()
    }

    pub fn cached_elements(
        &self,
        renderer: &mut GlesRenderer,
        output_size: Size<i32, Physical>,
        _display_transform: Transform,
        _blur_layer: BlurLayer,
        targets: &[LayerRenderTarget],
    ) -> Result<Vec<BlurElement>, GlesError> {
        let now = Instant::now();
        let mut elements = Vec::new();
        for target in targets {
            let Some(rect) = clipped_target_rect(output_size, target) else {
                continue;
            };
            let Some(entry) = self.cached_entry(target) else {
                continue;
            };
            elements.push(render_element(
                renderer,
                rect,
                entry,
                entry.opacity(now, target.opacity),
            ));
        }

        Ok(elements)
    }

    fn program(&mut self, renderer: &mut GlesRenderer) -> Result<GlesTexProgram, GlesError> {
        if let Some(program) = &self.program {
            return Ok(program.clone());
        }

        let program = renderer.compile_custom_texture_shader(
            BLUR_SHADER,
            &[
                UniformName::new("texel", UniformType::_2f),
                UniformName::new("target_size", UniformType::_2f),
                UniformName::new("radius", UniformType::_1f),
                UniformName::new("shape", UniformType::_1f),
                UniformName::new("direction", UniformType::_2f),
                UniformName::new("final_pass", UniformType::_1f),
                UniformName::new("mask_pass", UniformType::_1f),
            ],
        )?;
        self.program = Some(program.clone());
        Ok(program)
    }

    fn buffer_for_target(
        &mut self,
        renderer: &mut GlesRenderer,
        request: BlurTargetRequest<'_>,
    ) -> Result<(&SceneBlurCacheEntry, f32), GlesError> {
        let BlurTargetRequest {
            framebuffer,
            output_size,
            target_transform,
            target,
            rect,
            damage,
        } = request;
        let now = Instant::now();
        let cached = self.entries.iter().position(|entry| entry.matches(target));
        if let Some(index) = cached
            && self.entries[index].rect == rect
            && self.entries[index].sample_rect == padded_target_rect(output_size, target, rect)
            && !target_is_damaged(self.entries[index].sample_rect, damage)
        {
            self.entries[index].location = target.location;
            self.entries[index].target_opacity = target.opacity;
            let opacity = self.entries[index].opacity(now, target.opacity);
            return Ok((&self.entries[index], opacity));
        }

        let program = self.program(renderer)?;
        let sample_rect = padded_target_rect(output_size, target, rect);
        let capture_size = blur_texture_size(target, sample_rect.size);
        let texture_size = blur_texture_size(target, rect.size);
        let visible_source = source_rect_for_visible_target(sample_rect, rect, capture_size);
        let (capture, scratch, blurred, output) = match cached {
            Some(index)
                if self.entries[index].texture_size == texture_size
                    && self.entries[index].capture_size == capture_size =>
            {
                let entry = &mut self.entries[index];
                capture_target(
                    renderer,
                    framebuffer,
                    output_size,
                    target_transform,
                    sample_rect,
                    capture_size,
                    &mut entry.capture,
                )?;
                render_blur_texture(
                    renderer,
                    BlurRenderPass {
                        program: &program,
                        material: target.material,
                        visible_size: rect.size,
                        texture_size,
                        capture_size,
                        visible_source,
                        capture: &entry.capture,
                        scratch: &mut entry.scratch,
                        blurred: &mut entry.blurred,
                        output: &mut entry.output,
                    },
                )?;
                entry.rect = rect;
                entry.sample_rect = sample_rect;
                entry.location = target.location;
                entry.target_opacity = target.opacity;
                let opacity = entry.opacity(now, target.opacity);
                return Ok((&self.entries[index], opacity));
            }
            _ => {
                let mut capture = renderer.create_buffer(
                    Fourcc::Abgr8888,
                    Size::<i32, Buffer>::from((capture_size.w, capture_size.h)),
                )?;
                let mut scratch = renderer.create_buffer(
                    Fourcc::Abgr8888,
                    Size::<i32, Buffer>::from((capture_size.w, capture_size.h)),
                )?;
                let mut blurred = renderer.create_buffer(
                    Fourcc::Abgr8888,
                    Size::<i32, Buffer>::from((capture_size.w, capture_size.h)),
                )?;
                let mut output = renderer.create_buffer(
                    Fourcc::Abgr8888,
                    Size::<i32, Buffer>::from((texture_size.w, texture_size.h)),
                )?;
                capture_target(
                    renderer,
                    framebuffer,
                    output_size,
                    target_transform,
                    sample_rect,
                    capture_size,
                    &mut capture,
                )?;
                render_blur_texture(
                    renderer,
                    BlurRenderPass {
                        program: &program,
                        material: target.material,
                        visible_size: rect.size,
                        texture_size,
                        capture_size,
                        visible_source,
                        capture: &capture,
                        scratch: &mut scratch,
                        blurred: &mut blurred,
                        output: &mut output,
                    },
                )?;
                (capture, scratch, blurred, output)
            }
        };

        match cached {
            Some(index) => {
                self.entries[index] = SceneBlurCacheEntry {
                    id: self.entries[index].id.clone(),
                    surface: target.surface.clone(),
                    blur_layer: target.blur_layer,
                    rect,
                    sample_rect,
                    location: target.location,
                    size: target.size,
                    material: target.material,
                    capture_size,
                    texture_size,
                    capture,
                    scratch,
                    blurred,
                    output,
                    target_opacity: target.opacity,
                };
            }
            None => self.entries.push(SceneBlurCacheEntry {
                id: Id::new(),
                surface: target.surface.clone(),
                blur_layer: target.blur_layer,
                rect,
                sample_rect,
                location: target.location,
                size: target.size,
                material: target.material,
                capture_size,
                texture_size,
                capture,
                scratch,
                blurred,
                output,
                target_opacity: target.opacity,
            }),
        }

        let index = cached.unwrap_or(self.entries.len() - 1);
        let opacity = self.entries[index].opacity(now, target.opacity);
        Ok((&self.entries[index], opacity))
    }

    fn cached_entry(&self, target: &LayerRenderTarget) -> Option<&SceneBlurCacheEntry> {
        self.entries.iter().find(|entry| entry.matches(target))
    }

    fn target_capture_sample_rect(
        &self,
        output_size: Size<i32, Physical>,
        target: &LayerRenderTarget,
        damage: &[Rectangle<i32, Physical>],
    ) -> Option<Rectangle<i32, Physical>> {
        let rect = clipped_target_rect(output_size, target)?;
        let sample_rect = padded_target_rect(output_size, target, rect);
        self.cached_entry(target)
            .is_none_or(|entry| {
                entry.rect != rect
                    || entry.sample_rect != sample_rect
                    || target_is_damaged(sample_rect, damage)
            })
            .then_some(sample_rect)
    }
}

pub type BlurElement = RoundedWindowElement<TextureRenderElement<GlesTexture>>;

pub struct BlurCaptureRequest<'a> {
    pub framebuffer: &'a GlesTarget<'a>,
    pub output_size: Size<i32, Physical>,
    pub target_transform: Transform,
    pub targets: &'a [LayerRenderTarget],
    pub damage: &'a [Rectangle<i32, Physical>],
}

struct BlurTargetRequest<'a> {
    framebuffer: &'a GlesTarget<'a>,
    output_size: Size<i32, Physical>,
    target_transform: Transform,
    target: &'a LayerRenderTarget,
    rect: Rectangle<i32, Physical>,
    damage: &'a [Rectangle<i32, Physical>],
}

pub fn capture_blur_elements(
    cache: &mut SceneBlurCache,
    renderer: &mut GlesRenderer,
    request: BlurCaptureRequest<'_>,
) -> Result<Vec<BlurElement>, GlesError> {
    let mut elements = Vec::new();
    for target in request.targets {
        let Some(rect) = clipped_target_rect(request.output_size, target) else {
            continue;
        };
        let (entry, opacity) = cache.buffer_for_target(
            renderer,
            BlurTargetRequest {
                framebuffer: request.framebuffer,
                output_size: request.output_size,
                target_transform: request.target_transform,
                target,
                rect,
                damage: request.damage,
            },
        )?;
        elements.push(render_element(renderer, rect, entry, opacity));
    }

    Ok(elements)
}

#[derive(Debug)]
struct SceneBlurCacheEntry {
    id: Id,
    surface: WlSurface,
    blur_layer: BlurLayer,
    rect: Rectangle<i32, Physical>,
    sample_rect: Rectangle<i32, Physical>,
    location: Point<i32, Logical>,
    size: Size<i32, Logical>,
    material: LayerMaterial,
    capture_size: Size<i32, Physical>,
    texture_size: Size<i32, Physical>,
    capture: GlesTexture,
    scratch: GlesTexture,
    blurred: GlesTexture,
    output: GlesTexture,
    target_opacity: f32,
}

impl SceneBlurCacheEntry {
    fn matches(&self, target: &LayerRenderTarget) -> bool {
        self.surface == target.surface
            && self.blur_layer == target.blur_layer
            && (self.blur_layer != BlurLayer::Window || self.location == target.location)
            && self.size == target.size
            && self.material == target.material
    }

    fn opacity(&self, _now: Instant, current_opacity: f32) -> f32 {
        current_opacity.clamp(0.0, 1.0)
    }
}

fn capture_target(
    renderer: &mut GlesRenderer,
    framebuffer: &GlesTarget<'_>,
    output_size: Size<i32, Physical>,
    target_transform: Transform,
    rect: Rectangle<i32, Physical>,
    texture_size: Size<i32, Physical>,
    capture: &mut GlesTexture,
) -> Result<(), GlesError> {
    let source = framebuffer_source_rect(output_size, target_transform, rect);
    let target = Rectangle::<i32, Physical>::from_size(texture_size);
    let mut target_framebuffer = renderer.bind(capture)?;
    renderer.blit(
        framebuffer,
        &mut target_framebuffer,
        source,
        target,
        TextureFilter::Linear,
    )
}

fn framebuffer_source_rect(
    output_size: Size<i32, Physical>,
    transform: Transform,
    rect: Rectangle<i32, Physical>,
) -> Rectangle<i32, Physical> {
    transform.transform_rect_in(rect, &output_size)
}

fn render_element(
    renderer: &GlesRenderer,
    rect: Rectangle<i32, Physical>,
    entry: &SceneBlurCacheEntry,
    opacity: f32,
) -> BlurElement {
    let element = TextureRenderElement::from_static_texture(
        entry.id.clone(),
        renderer.context_id(),
        Point::<f64, Physical>::from((rect.loc.x as f64, rect.loc.y as f64)),
        entry.output.clone(),
        1,
        Transform::Normal,
        Some(opacity.clamp(0.0, 1.0)),
        Some(Rectangle::<f64, Logical>::from_size(
            Size::<f64, Logical>::from((entry.texture_size.w as f64, entry.texture_size.h as f64)),
        )),
        Some(Size::<i32, Logical>::from((rect.size.w, rect.size.h))),
        None,
        Kind::Unspecified,
    );
    RoundedWindowElement::new_with_shape(
        element,
        rect,
        material_clip_shape(entry.material, rect.size),
    )
}
