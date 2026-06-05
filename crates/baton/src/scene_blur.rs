use crate::{
    blur_material,
    layers::{BlurLayer, LayerMaterial, LayerRenderTarget},
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
use std::time::{Duration, Instant};

const BLUR_FADE_IN: Duration = Duration::from_millis(145);
const BLUR_FADE_OUT: Duration = Duration::from_millis(110);
const BLUR_FADE_SETTLE: Duration = Duration::from_millis(24);

#[derive(Default)]
pub struct SceneBlurCache {
    entries: Vec<SceneBlurCacheEntry>,
}

impl SceneBlurCache {
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn retain_targets(&mut self, targets: &[LayerRenderTarget]) {
        let now = Instant::now();
        for entry in &mut self.entries {
            if let Some(target) = targets
                .iter()
                .find(|target| target.size.w > 1 && target.size.h > 1 && entry.matches(target))
            {
                entry.removed_at = None;
                entry.target_opacity = target.opacity;
            } else if entry.removed_at.is_none() {
                entry.removed_at = Some(now);
            }
        }
        self.entries.retain(|entry| {
            if entry.blur_layer != BlurLayer::Window && entry.removed_at.is_some() {
                return false;
            }
            entry.removed_at.is_none_or(|removed_at| {
                now.saturating_duration_since(removed_at) <= BLUR_FADE_OUT + BLUR_FADE_SETTLE
            })
        });
    }

    pub fn is_animating(&self) -> bool {
        let now = Instant::now();
        self.entries.iter().any(|entry| {
            if let Some(removed_at) = entry.removed_at {
                return now.saturating_duration_since(removed_at)
                    <= BLUR_FADE_OUT + BLUR_FADE_SETTLE;
            }
            now.saturating_duration_since(entry.appeared_at) <= BLUR_FADE_IN + BLUR_FADE_SETTLE
        })
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
        blur_layer: BlurLayer,
        targets: &[LayerRenderTarget],
    ) -> Result<Vec<MemoryRenderBufferRenderElement<GlesRenderer>>, GlesError> {
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
                &entry.buffer,
                entry.opacity(now, target.opacity),
            )?);
        }

        for entry in self.fading_entries(blur_layer) {
            let Some(rect) = clipped_entry_rect(output_size, entry) else {
                continue;
            };
            elements.push(render_element(
                renderer,
                rect,
                &entry.buffer,
                entry.opacity(now, entry.target_opacity),
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
    ) -> Result<(&MemoryRenderBuffer, f32), GlesError> {
        let now = Instant::now();
        let cached = self.entries.iter().position(|entry| entry.matches(target));
        if let Some(index) = cached
            && !target_is_damaged(rect, damage)
        {
            self.entries[index].removed_at = None;
            self.entries[index].target_opacity = target.opacity;
            let opacity = self.entries[index].opacity(now, target.opacity);
            return Ok((&self.entries[index].buffer, opacity));
        }

        let pixels = capture_pixels(renderer, framebuffer, output_size, rect)?;
        let buffer = blur_patch_from_capture(&pixels, rect.size, target.material);
        match cached {
            Some(index) => {
                self.entries[index].buffer = buffer;
                self.entries[index].removed_at = None;
                self.entries[index].target_opacity = target.opacity;
            }
            None => self.entries.push(SceneBlurCacheEntry {
                surface: target.surface.clone(),
                blur_layer: target.blur_layer,
                location: target.location,
                size: target.size,
                material: target.material,
                buffer,
                target_opacity: target.opacity,
                appeared_at: now,
                removed_at: None,
            }),
        }

        let index = cached.unwrap_or(self.entries.len() - 1);
        let opacity = self.entries[index].opacity(now, target.opacity);
        Ok((&self.entries[index].buffer, opacity))
    }

    fn has_target(&self, target: &LayerRenderTarget) -> bool {
        self.entries.iter().any(|entry| entry.matches(target))
    }

    fn cached_entry(&self, target: &LayerRenderTarget) -> Option<&SceneBlurCacheEntry> {
        self.entries.iter().find(|entry| entry.matches(target))
    }

    fn fading_entries(&self, blur_layer: BlurLayer) -> impl Iterator<Item = &SceneBlurCacheEntry> {
        self.entries
            .iter()
            .filter(move |entry| entry.blur_layer == blur_layer && entry.removed_at.is_some())
    }
}

pub fn capture_blur_elements(
    cache: &mut SceneBlurCache,
    renderer: &mut GlesRenderer,
    framebuffer: &GlesTarget<'_>,
    output_size: Size<i32, Physical>,
    blur_layer: BlurLayer,
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
        let (buffer, opacity) =
            cache.buffer_for_target(renderer, framebuffer, output_size, target, rect, damage)?;
        elements.push(render_element(renderer, rect, buffer, opacity)?);
    }

    let now = Instant::now();
    for entry in cache.fading_entries(blur_layer) {
        let Some(rect) = clipped_entry_rect(output_size, entry) else {
            continue;
        };
        elements.push(render_element(
            renderer,
            rect,
            &entry.buffer,
            entry.opacity(now, entry.target_opacity),
        )?);
    }

    Ok(elements)
}

#[derive(Debug)]
struct SceneBlurCacheEntry {
    surface: WlSurface,
    blur_layer: BlurLayer,
    location: Point<i32, Logical>,
    size: Size<i32, Logical>,
    material: LayerMaterial,
    buffer: MemoryRenderBuffer,
    target_opacity: f32,
    appeared_at: Instant,
    removed_at: Option<Instant>,
}

impl SceneBlurCacheEntry {
    fn matches(&self, target: &LayerRenderTarget) -> bool {
        self.surface == target.surface
            && self.blur_layer == target.blur_layer
            && self.location == target.location
            && self.size == target.size
            && self.material == target.material
    }

    fn opacity(&self, now: Instant, current_opacity: f32) -> f32 {
        if let Some(removed_at) = self.removed_at {
            let progress =
                duration_progress(now.saturating_duration_since(removed_at), BLUR_FADE_OUT);
            return (self.target_opacity * (1.0 - progress)).clamp(0.0, 1.0);
        }
        (current_opacity
            * duration_progress(
                now.saturating_duration_since(self.appeared_at),
                BLUR_FADE_IN,
            ))
        .clamp(0.0, 1.0)
    }
}

fn clipped_target_rect(
    output_size: Size<i32, Physical>,
    target: &LayerRenderTarget,
) -> Option<Rectangle<i32, Physical>> {
    clipped_rect(output_size, target.location, target.size)
}

fn clipped_entry_rect(
    output_size: Size<i32, Physical>,
    entry: &SceneBlurCacheEntry,
) -> Option<Rectangle<i32, Physical>> {
    clipped_rect(output_size, entry.location, entry.size)
}

fn clipped_rect(
    output_size: Size<i32, Physical>,
    location: Point<i32, Logical>,
    size: Size<i32, Logical>,
) -> Option<Rectangle<i32, Physical>> {
    if size.w <= 1 || size.h <= 1 {
        return None;
    }

    let output = Rectangle::<i32, Physical>::from_size(output_size);
    Rectangle::<i32, Physical>::new((location.x, location.y).into(), (size.w, size.h).into())
        .intersection(output)
}

fn render_element(
    renderer: &mut GlesRenderer,
    rect: Rectangle<i32, Physical>,
    buffer: &MemoryRenderBuffer,
    opacity: f32,
) -> Result<MemoryRenderBufferRenderElement<GlesRenderer>, GlesError> {
    MemoryRenderBufferRenderElement::from_buffer(
        renderer,
        (rect.loc.x as f64, rect.loc.y as f64),
        buffer,
        Some(opacity.clamp(0.0, 1.0)),
        None,
        Some(Size::<i32, Logical>::from((rect.size.w, rect.size.h))),
        Kind::Unspecified,
    )
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

fn duration_progress(elapsed: Duration, duration: Duration) -> f32 {
    if duration.is_zero() {
        return 1.0;
    }
    (elapsed.as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0)
}
