use crate::{
    layers::{BlurLayer, LayerMaterial, LayerRenderTarget},
    window_clip::RoundedWindowElement,
};
use asher_config::BlurQuality;
use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            Bind, Blit, Frame, Offscreen, Renderer, TextureFilter,
            element::{Id, Kind, texture::TextureRenderElement},
            gles::{
                GlesError, GlesRenderer, GlesTarget, GlesTexProgram, GlesTexture, Uniform,
                UniformName, UniformType, UniformValue,
            },
        },
    },
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Buffer, Logical, Physical, Point, Rectangle, Size, Transform},
};
use std::time::Instant;

const WINDOW_BLUR_SAMPLE_PADDING: i32 = 36;
const LAYER_BLUR_SAMPLE_PADDING: i32 = 36;

const BLUR_SHADER: &str = r#"#version 100
#extension GL_OES_standard_derivatives : enable

//_DEFINES_

#if defined(EXTERNAL)
#extension GL_OES_EGL_image_external : require
#endif

precision highp float;
#if defined(EXTERNAL)
uniform samplerExternalOES tex;
#else
uniform sampler2D tex;
#endif

uniform float alpha;
uniform vec2 texel;
uniform vec2 target_size;
uniform float radius;
uniform vec2 direction;
uniform float final_pass;
uniform float mask_pass;
varying vec2 v_coords;

#if defined(DEBUG_FLAGS)
uniform float tint;
#endif

float hash(vec2 value) {
    return fract(sin(dot(value, vec2(127.1, 311.7))) * 43758.5453123) - 0.5;
}

float sdfRoundedBox(vec2 position, vec2 center, vec2 extents, float corner_radius) {
    vec2 p = position - center;
    vec2 q = abs(p) - extents + vec2(corner_radius);
    return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - corner_radius;
}

float roundedCoverage(vec2 pixel) {
    if (radius <= 0.0) {
        return 1.0;
    }

    vec2 center = target_size * 0.5;
    float distance = sdfRoundedBox(pixel + vec2(0.5), center, center, radius);
    return distance <= 0.0 ? 1.0 : 0.0;
}

vec2 blurSampleUv(vec2 uv) {
    return clamp(uv, vec2(0.0), vec2(1.0));
}

void main() {
    vec2 uv = v_coords;
    vec4 color;

    if (mask_pass > 0.5) {
        color = texture2D(tex, uv);
    } else {
        vec2 step = texel * direction;
        color = texture2D(tex, blurSampleUv(uv)) * 0.227027;
        color += texture2D(tex, blurSampleUv(uv + step * 1.384615)) * 0.316216;
        color += texture2D(tex, blurSampleUv(uv - step * 1.384615)) * 0.316216;
        color += texture2D(tex, blurSampleUv(uv + step * 3.230769)) * 0.070270;
        color += texture2D(tex, blurSampleUv(uv - step * 3.230769)) * 0.070270;

        if (final_pass < 0.5) {
            gl_FragColor = color;
            return;
        }
    }

    float luma = dot(color.rgb, vec3(0.2126, 0.7152, 0.0722));
    color.rgb = luma + (color.rgb - vec3(luma)) * 1.06;
    color.rgb += hash(uv * target_size) * 0.0028;

    float coverage = roundedCoverage(uv * target_size);
    if (coverage <= 0.0) {
        discard;
    }
    color.a = coverage * alpha;
    color.rgb *= color.a;

#if defined(DEBUG_FLAGS)
    if (tint == 1.0)
        color = vec4(0.0, 0.2, 0.0, 0.2) + color * 0.8;
#endif

    gl_FragColor = color;
}
"#;

#[derive(Default)]
pub struct SceneBlurCache {
    entries: Vec<SceneBlurCacheEntry>,
    program: Option<GlesTexProgram>,
    animating: bool,
}

impl SceneBlurCache {
    pub fn clear(&mut self) {
        self.entries.clear();
    }

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
            let Some(rect) = clipped_target_rect(output_size, target) else {
                return false;
            };
            let sample_rect = padded_target_rect(output_size, target, rect);
            self.cached_entry(target).is_none_or(|entry| {
                entry.rect != rect
                    || entry.sample_rect != sample_rect
                    || target_is_damaged(sample_rect, damage)
            })
        })
    }

    pub fn cached_elements(
        &self,
        renderer: &mut GlesRenderer,
        output_size: Size<i32, Physical>,
        display_transform: Transform,
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
                display_transform,
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
            quality,
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
        let capture_size = blur_texture_size(target, sample_rect.size, quality);
        let texture_size = blur_texture_size(target, rect.size, quality);
        let source = source_rect_for_visible_target(
            output_size,
            target_transform,
            sample_rect,
            rect,
            capture_size,
        );
        let (capture, scratch, blurred) = match cached {
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
                        source,
                        capture: &entry.capture,
                        scratch: &mut entry.scratch,
                        blurred: &mut entry.blurred,
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
                    Size::<i32, Buffer>::from((texture_size.w, texture_size.h)),
                )?;
                let mut blurred = renderer.create_buffer(
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
                        source,
                        capture: &capture,
                        scratch: &mut scratch,
                        blurred: &mut blurred,
                    },
                )?;
                (capture, scratch, blurred)
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
}

pub type BlurElement = RoundedWindowElement<TextureRenderElement<GlesTexture>>;

pub struct BlurCaptureRequest<'a> {
    pub framebuffer: &'a GlesTarget<'a>,
    pub output_size: Size<i32, Physical>,
    pub target_transform: Transform,
    pub targets: &'a [LayerRenderTarget],
    pub damage: &'a [Rectangle<i32, Physical>],
    pub enabled: bool,
    pub quality: BlurQuality,
}

struct BlurTargetRequest<'a> {
    framebuffer: &'a GlesTarget<'a>,
    output_size: Size<i32, Physical>,
    target_transform: Transform,
    target: &'a LayerRenderTarget,
    rect: Rectangle<i32, Physical>,
    damage: &'a [Rectangle<i32, Physical>],
    quality: BlurQuality,
}

pub fn capture_blur_elements(
    cache: &mut SceneBlurCache,
    renderer: &mut GlesRenderer,
    request: BlurCaptureRequest<'_>,
) -> Result<Vec<BlurElement>, GlesError> {
    if !request.enabled {
        return Ok(Vec::new());
    }

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
                quality: request.quality,
            },
        )?;
        elements.push(render_element(
            renderer,
            rect,
            entry,
            opacity,
            request.target_transform,
        ));
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

struct BlurRenderPass<'a> {
    program: &'a GlesTexProgram,
    material: LayerMaterial,
    visible_size: Size<i32, Physical>,
    texture_size: Size<i32, Physical>,
    capture_size: Size<i32, Physical>,
    source: Rectangle<f64, Buffer>,
    capture: &'a GlesTexture,
    scratch: &'a mut GlesTexture,
    blurred: &'a mut GlesTexture,
}

fn render_blur_texture(
    renderer: &mut GlesRenderer,
    pass: BlurRenderPass<'_>,
) -> Result<(), GlesError> {
    let full_damage = [Rectangle::<i32, Physical>::from_size(pass.texture_size)];
    let full_source = Rectangle::<f64, Buffer>::from_size(Size::<f64, Buffer>::from((
        pass.texture_size.w as f64,
        pass.texture_size.h as f64,
    )));
    {
        let mut scratch_target = renderer.bind(pass.scratch)?;
        let mut frame =
            renderer.render(&mut scratch_target, pass.texture_size, Transform::Normal)?;
        frame.clear(
            smithay::backend::renderer::Color32F::new(0.0, 0.0, 0.0, 0.0),
            &full_damage,
        )?;
        frame.render_texture_from_to(
            pass.capture,
            pass.source,
            Rectangle::<i32, Physical>::from_size(pass.texture_size),
            &full_damage,
            &[],
            Transform::Normal,
            1.0,
            Some(pass.program),
            &blur_uniforms(BlurUniforms {
                texel_size: pass.capture_size,
                target_size: pass.texture_size,
                visible_size: pass.visible_size,
                material: pass.material,
                direction: (1.0, 0.0),
                final_pass: false,
                mask_pass: false,
            }),
        )?;
        let _ = frame.finish()?;
    }

    {
        let mut blurred_target = renderer.bind(pass.blurred)?;
        let mut frame =
            renderer.render(&mut blurred_target, pass.texture_size, Transform::Normal)?;
        frame.clear(
            smithay::backend::renderer::Color32F::new(0.0, 0.0, 0.0, 0.0),
            &full_damage,
        )?;
        frame.render_texture_from_to(
            pass.scratch,
            full_source,
            Rectangle::<i32, Physical>::from_size(pass.texture_size),
            &full_damage,
            &[],
            Transform::Normal,
            1.0,
            Some(pass.program),
            &blur_uniforms(BlurUniforms {
                texel_size: pass.texture_size,
                target_size: pass.texture_size,
                visible_size: pass.visible_size,
                material: pass.material,
                direction: (0.0, 1.0),
                final_pass: false,
                mask_pass: false,
            }),
        )?;
        let _ = frame.finish()?;
    }

    {
        let mut scratch_target = renderer.bind(pass.scratch)?;
        let mut frame =
            renderer.render(&mut scratch_target, pass.texture_size, Transform::Normal)?;
        frame.clear(
            smithay::backend::renderer::Color32F::new(0.0, 0.0, 0.0, 0.0),
            &full_damage,
        )?;
        frame.render_texture_from_to(
            pass.blurred,
            full_source,
            Rectangle::<i32, Physical>::from_size(pass.texture_size),
            &full_damage,
            &[],
            Transform::Normal,
            1.0,
            Some(pass.program),
            &blur_uniforms(BlurUniforms {
                texel_size: pass.texture_size,
                target_size: pass.texture_size,
                visible_size: pass.visible_size,
                material: pass.material,
                direction: (0.0, 0.0),
                final_pass: true,
                mask_pass: true,
            }),
        )?;
        let _ = frame.finish()?;
    }

    Ok(())
}

fn render_element(
    renderer: &GlesRenderer,
    rect: Rectangle<i32, Physical>,
    entry: &SceneBlurCacheEntry,
    opacity: f32,
    display_transform: Transform,
) -> BlurElement {
    let element = TextureRenderElement::from_static_texture(
        entry.id.clone(),
        renderer.context_id(),
        Point::<f64, Physical>::from((rect.loc.x as f64, rect.loc.y as f64)),
        entry.scratch.clone(),
        1,
        display_transform,
        Some(opacity.clamp(0.0, 1.0)),
        Some(Rectangle::<f64, Logical>::from_size(
            Size::<f64, Logical>::from((entry.texture_size.w as f64, entry.texture_size.h as f64)),
        )),
        Some(Size::<i32, Logical>::from((rect.size.w, rect.size.h))),
        None,
        Kind::Unspecified,
    );
    RoundedWindowElement::new(element, rect, 0)
}

struct BlurUniforms {
    texel_size: Size<i32, Physical>,
    target_size: Size<i32, Physical>,
    visible_size: Size<i32, Physical>,
    material: LayerMaterial,
    direction: (f32, f32),
    final_pass: bool,
    mask_pass: bool,
}

fn blur_uniforms(uniforms: BlurUniforms) -> [Uniform<'static>; 6] {
    [
        Uniform::new(
            "texel",
            UniformValue::_2f(
                1.0 / uniforms.texel_size.w.max(1) as f32,
                1.0 / uniforms.texel_size.h.max(1) as f32,
            ),
        ),
        Uniform::new(
            "target_size",
            UniformValue::_2f(uniforms.target_size.w as f32, uniforms.target_size.h as f32),
        ),
        Uniform::new(
            "radius",
            UniformValue::_1f(material_radius(
                uniforms.material,
                uniforms.target_size,
                uniforms.visible_size,
            )),
        ),
        Uniform::new(
            "direction",
            UniformValue::_2f(uniforms.direction.0, uniforms.direction.1),
        ),
        Uniform::new(
            "final_pass",
            UniformValue::_1f(if uniforms.final_pass { 1.0 } else { 0.0 }),
        ),
        Uniform::new(
            "mask_pass",
            UniformValue::_1f(if uniforms.mask_pass { 1.0 } else { 0.0 }),
        ),
    ]
}

fn material_radius(
    material: LayerMaterial,
    texture_size: Size<i32, Physical>,
    visible_size: Size<i32, Physical>,
) -> f32 {
    match material {
        LayerMaterial::Rect => 0.0,
        LayerMaterial::RoundRect { radius } => {
            let scale_x = visible_size.w.max(1) as f32 / texture_size.w.max(1) as f32;
            let scale_y = visible_size.h.max(1) as f32 / texture_size.h.max(1) as f32;
            let scale = scale_x.min(scale_y).max(1.0);
            radius as f32 / scale
        }
    }
}

fn clipped_target_rect(
    output_size: Size<i32, Physical>,
    target: &LayerRenderTarget,
) -> Option<Rectangle<i32, Physical>> {
    clipped_rect(output_size, target.location, target.size)
}

fn padded_target_rect(
    output_size: Size<i32, Physical>,
    target: &LayerRenderTarget,
    rect: Rectangle<i32, Physical>,
) -> Rectangle<i32, Physical> {
    let padding = blur_sample_padding(target);
    if padding <= 0 {
        return rect;
    }

    let output = Rectangle::<i32, Physical>::from_size(output_size);
    let left = (rect.loc.x - padding).max(output.loc.x);
    let top = (rect.loc.y - padding).max(output.loc.y);
    let right = (rect.loc.x + rect.size.w + padding).min(output.loc.x + output.size.w);
    let bottom = (rect.loc.y + rect.size.h + padding).min(output.loc.y + output.size.h);
    Rectangle::<i32, Physical>::new((left, top).into(), (right - left, bottom - top).into())
}

fn blur_sample_padding(target: &LayerRenderTarget) -> i32 {
    match target.blur_layer {
        BlurLayer::Window => WINDOW_BLUR_SAMPLE_PADDING,
        BlurLayer::Top | BlurLayer::Overlay => LAYER_BLUR_SAMPLE_PADDING,
    }
}

fn source_rect_for_visible_target(
    output_size: Size<i32, Physical>,
    transform: Transform,
    sample_rect: Rectangle<i32, Physical>,
    visible_rect: Rectangle<i32, Physical>,
    capture_size: Size<i32, Physical>,
) -> Rectangle<f64, Buffer> {
    let sample_buffer = transform.transform_rect_in(sample_rect, &output_size);
    let visible_buffer = transform.transform_rect_in(visible_rect, &output_size);
    let scale_x = capture_size.w as f64 / sample_rect.size.w.max(1) as f64;
    let scale_y = capture_size.h as f64 / sample_rect.size.h.max(1) as f64;
    Rectangle::<f64, Buffer>::new(
        (
            (visible_buffer.loc.x - sample_buffer.loc.x) as f64 * scale_x,
            (visible_buffer.loc.y - sample_buffer.loc.y) as f64 * scale_y,
        )
            .into(),
        (
            visible_buffer.size.w as f64 * scale_x,
            visible_buffer.size.h as f64 * scale_y,
        )
            .into(),
    )
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

fn blur_texture_size(
    target: &LayerRenderTarget,
    size: Size<i32, Physical>,
    quality: BlurQuality,
) -> Size<i32, Physical> {
    let scale = blur_downscale(target, size.w, size.h, quality);
    Size::<i32, Physical>::from((
        div_ceil(size.w, scale).max(1),
        div_ceil(size.h, scale).max(1),
    ))
}

fn blur_downscale(
    target: &LayerRenderTarget,
    width: i32,
    height: i32,
    quality: BlurQuality,
) -> i32 {
    let base: i32 = if target.blur_layer != BlurLayer::Window {
        2
    } else {
        let area = width.saturating_mul(height);
        if area >= 420_000 {
            12
        } else if area >= 120_000 {
            10
        } else {
            7
        }
    };

    match quality {
        BlurQuality::Quality => base.saturating_sub(2).max(2),
        BlurQuality::Balanced => base,
        BlurQuality::Performance => base.saturating_add(3),
    }
}

fn div_ceil(value: i32, divisor: i32) -> i32 {
    (value + divisor - 1) / divisor
}

fn target_is_damaged(rect: Rectangle<i32, Physical>, damage: &[Rectangle<i32, Physical>]) -> bool {
    damage
        .iter()
        .any(|damage| damage.intersection(rect).is_some())
}
