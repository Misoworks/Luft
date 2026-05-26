use crate::{
    blur_material,
    layers::{LayerMaterial, LayerRenderTarget},
};
use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            ExportMem, TextureMapping,
            element::{
                Kind,
                memory::{MemoryRenderBuffer, MemoryRenderBufferRenderElement},
            },
            gles::{GlesError, GlesRenderer, GlesTarget},
        },
    },
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Buffer, Logical, Physical, Point, Rectangle, Size},
};

#[derive(Default)]
pub struct SceneBlurCache {
    entries: Vec<SceneBlurCacheEntry>,
}

impl SceneBlurCache {
    pub fn retain_targets(&mut self, targets: &[LayerRenderTarget]) {
        self.entries.retain(|entry| {
            targets
                .iter()
                .any(|target| target.size.w > 1 && target.size.h > 1 && entry.matches(target))
        });
    }

    pub fn targets_need_capture(
        &self,
        output_size: Size<i32, Physical>,
        targets: &[LayerRenderTarget],
        damage: &[Rectangle<i32, Physical>],
    ) -> bool {
        targets.iter().any(|target| {
            let Some(rect) = clipped_target_rect(output_size, target) else {
                return false;
            };
            !self.has_target(target) || target_is_damaged(rect, damage)
        })
    }

    pub fn cached_elements(
        &self,
        renderer: &mut GlesRenderer,
        output_size: Size<i32, Physical>,
        targets: &[LayerRenderTarget],
    ) -> Result<Vec<MemoryRenderBufferRenderElement<GlesRenderer>>, GlesError> {
        let mut elements = Vec::new();
        for target in targets {
            let Some(rect) = clipped_target_rect(output_size, target) else {
                continue;
            };
            let Some(buffer) = self.cached_buffer(target) else {
                continue;
            };
            elements.push(MemoryRenderBufferRenderElement::from_buffer(
                renderer,
                (rect.loc.x as f64, rect.loc.y as f64),
                buffer,
                None,
                None,
                Some(Size::<i32, Logical>::from((rect.size.w, rect.size.h))),
                Kind::Unspecified,
            )?);
        }

        Ok(elements)
    }

    fn buffer_for_target(
        &mut self,
        renderer: &mut GlesRenderer,
        framebuffer: &GlesTarget<'_>,
        output_size: Size<i32, Physical>,
        target: &LayerRenderTarget,
        rect: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
    ) -> Result<&MemoryRenderBuffer, GlesError> {
        let cached = self.entries.iter().position(|entry| entry.matches(target));
        if let Some(index) = cached
            && !target_is_damaged(rect, damage)
        {
            return Ok(&self.entries[index].buffer);
        }

        let pixels = capture_pixels(renderer, framebuffer, output_size, rect)?;
        let buffer = blur_patch_from_capture(&pixels, rect.size, target.material);
        match cached {
            Some(index) => self.entries[index].buffer = buffer,
            None => self.entries.push(SceneBlurCacheEntry {
                surface: target.surface.clone(),
                location: target.location,
                size: target.size,
                material: target.material,
                buffer,
            }),
        }

        let index = cached.unwrap_or(self.entries.len() - 1);
        Ok(&self.entries[index].buffer)
    }

    fn has_target(&self, target: &LayerRenderTarget) -> bool {
        self.entries.iter().any(|entry| entry.matches(target))
    }

    fn cached_buffer(&self, target: &LayerRenderTarget) -> Option<&MemoryRenderBuffer> {
        self.entries
            .iter()
            .find(|entry| entry.matches(target))
            .map(|entry| &entry.buffer)
    }
}

pub fn capture_blur_elements(
    cache: &mut SceneBlurCache,
    renderer: &mut GlesRenderer,
    framebuffer: &GlesTarget<'_>,
    output_size: Size<i32, Physical>,
    targets: &[LayerRenderTarget],
    damage: &[Rectangle<i32, Physical>],
    enabled: bool,
) -> Result<Vec<MemoryRenderBufferRenderElement<GlesRenderer>>, GlesError> {
    if !enabled {
        return Ok(Vec::new());
    }

    let mut elements = Vec::new();
    for target in targets {
        let Some(rect) = clipped_target_rect(output_size, target) else {
            continue;
        };
        let buffer =
            cache.buffer_for_target(renderer, framebuffer, output_size, target, rect, damage)?;
        elements.push(MemoryRenderBufferRenderElement::from_buffer(
            renderer,
            (rect.loc.x as f64, rect.loc.y as f64),
            buffer,
            None,
            None,
            Some(Size::<i32, Logical>::from((rect.size.w, rect.size.h))),
            Kind::Unspecified,
        )?);
    }

    Ok(elements)
}

#[derive(Debug)]
struct SceneBlurCacheEntry {
    surface: WlSurface,
    location: Point<i32, Logical>,
    size: Size<i32, Logical>,
    material: LayerMaterial,
    buffer: MemoryRenderBuffer,
}

impl SceneBlurCacheEntry {
    fn matches(&self, target: &LayerRenderTarget) -> bool {
        self.surface == target.surface
            && self.location == target.location
            && self.size == target.size
            && self.material == target.material
    }
}

fn clipped_target_rect(
    output_size: Size<i32, Physical>,
    target: &LayerRenderTarget,
) -> Option<Rectangle<i32, Physical>> {
    if target.size.w <= 1 || target.size.h <= 1 {
        return None;
    }

    let output = Rectangle::<i32, Physical>::from_size(output_size);
    Rectangle::<i32, Physical>::new(
        (target.location.x, target.location.y).into(),
        (target.size.w, target.size.h).into(),
    )
    .intersection(output)
}

fn capture_pixels(
    renderer: &mut GlesRenderer,
    framebuffer: &GlesTarget<'_>,
    output_size: Size<i32, Physical>,
    rect: Rectangle<i32, Physical>,
) -> Result<Vec<u8>, GlesError> {
    let region = Rectangle::<i32, Buffer>::new(
        (rect.loc.x, output_size.h - rect.loc.y - rect.size.h).into(),
        (rect.size.w, rect.size.h).into(),
    );
    let mapping = renderer.copy_framebuffer(framebuffer, region, Fourcc::Abgr8888)?;
    let flipped = mapping.flipped();
    let bytes = renderer.map_texture(&mapping)?;

    Ok(top_left_pixels(bytes, rect.size, flipped))
}

fn target_is_damaged(rect: Rectangle<i32, Physical>, damage: &[Rectangle<i32, Physical>]) -> bool {
    damage
        .iter()
        .any(|damage| damage.intersection(rect).is_some())
}

fn top_left_pixels(source: &[u8], size: Size<i32, Physical>, flipped: bool) -> Vec<u8> {
    if !flipped {
        return source.to_vec();
    }

    let stride = (size.w * 4) as usize;
    let mut pixels = vec![0; source.len()];
    for y in 0..size.h as usize {
        let source_start = (size.h as usize - 1 - y) * stride;
        let target_start = y * stride;
        pixels[target_start..target_start + stride]
            .copy_from_slice(&source[source_start..source_start + stride]);
    }
    pixels
}

fn blur_patch_from_capture(
    pixels: &[u8],
    size: Size<i32, Physical>,
    material: LayerMaterial,
) -> MemoryRenderBuffer {
    blur_material::build_blur_patch_for_material(
        pixels,
        size,
        Point::from((0, 0)),
        Size::<i32, Logical>::from((size.w, size.h)),
        material,
    )
}
