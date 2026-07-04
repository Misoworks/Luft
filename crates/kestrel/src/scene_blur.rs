use crate::layers::{BlurLayer, LayerMaterial, LayerRenderTarget};
use crate::scene_backdrop::SceneBackdrop;
use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            Bind, BlitFrame, Frame, FrameContext, Offscreen, TextureFilter,
            element::{Element, Id, Kind, RenderElement},
            gles::{
                GlesError, GlesFrame, GlesRenderer, GlesTexProgram, GlesTexture,
                UniformName, UniformType,
            },
        },
    },
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{
        user_data::UserDataMap, Buffer, Logical, Physical, Point, Rectangle, Scale, Size,
        Transform,
    },
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

mod geometry;
mod render_pass;

use geometry::{
    blur_texture_size, clipped_target_rect, padded_target_rect, source_rect_for_visible_target,
};
use render_pass::{BlurRenderPass, render_blur_texture};

const BLUR_SHADER: &str = include_str!("shaders/scene_blur.frag");

pub struct BlurEffectManager {
    entries: Vec<BlurTargetEntry>,
    inner_cache: Rc<RefCell<HashMap<Id, BlurInner>>>,
}

struct BlurTargetEntry {
    id: Id,
    commit: smithay::backend::renderer::utils::CommitCounter,
    surface: WlSurface,
    blur_layer: BlurLayer,
    location: Point<i32, Logical>,
    size: Size<i32, Logical>,
    material: LayerMaterial,
    target_opacity: f32,
}

#[derive(Clone, Debug)]
struct BackdropCapture {
    texture: GlesTexture,
    generation: u64,
}

pub struct FramebufferBlurElement {
    id: Id,
    commit: smithay::backend::renderer::utils::CommitCounter,
    blur_layer: BlurLayer,
    rect: Rectangle<i32, Physical>,
    sample_rect: Rectangle<i32, Physical>,
    material: LayerMaterial,
    opacity: f32,
    texture_size: Size<i32, Physical>,
    capture_size: Size<i32, Physical>,
    visible_source: Rectangle<f64, Buffer>,
    output_size: Size<i32, Physical>,
    target_transform: Transform,
    backdrop: Option<BackdropCapture>,
    inner_cache: Rc<RefCell<HashMap<Id, BlurInner>>>,
}

struct BlurInner {
    capture: GlesTexture,
    scratch: GlesTexture,
    blurred: GlesTexture,
    output: GlesTexture,
    intermediate: Option<GlesTexture>,
    program: Option<GlesTexProgram>,
    texture_size: Size<i32, Physical>,
    capture_size: Size<i32, Physical>,
    backdrop_generation: Option<u64>,
}

impl Default for BlurEffectManager {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            inner_cache: Rc::new(RefCell::new(HashMap::new())),
        }
    }
}

impl BlurEffectManager {
    pub fn retain_targets(&mut self, targets: &[LayerRenderTarget]) {
        for entry in &mut self.entries {
            if let Some(target) = targets
                .iter()
                .find(|target| target.size.w > 1 && target.size.h > 1 && entry.matches(target))
            {
                if entry.location != target.location {
                    entry.commit.increment();
                }
                if (entry.target_opacity - target.opacity).abs() > 0.01 {
                    entry.commit.increment();
                }
                entry.location = target.location;
                entry.target_opacity = target.opacity;
            }
        }
        self.entries
            .retain(|entry| targets.iter().any(|target| entry.matches(target)));
        self.inner_cache
            .borrow_mut()
            .retain(|id, _| self.entries.iter().any(|entry| entry.id == *id));
    }

    pub fn elements_for(
        &mut self,
        output_size: Size<i32, Physical>,
        target_transform: Transform,
        targets: &[LayerRenderTarget],
        backdrop: Option<&SceneBackdrop>,
    ) -> Vec<FramebufferBlurElement> {
        let layer_backdrop = backdrop.and_then(|backdrop| {
            let texture = backdrop.texture()?.clone();
            Some(BackdropCapture {
                texture,
                generation: backdrop.generation(),
            })
        });
        let mut elements = Vec::with_capacity(targets.len());
        for target in targets {
            if target.blur_layer != BlurLayer::Window && layer_backdrop.is_none() {
                continue;
            }
            let Some(rect) = clipped_target_rect(output_size, target) else {
                continue;
            };
            let sample_rect = padded_target_rect(output_size, target, rect);
            let capture_size = blur_texture_size(target, sample_rect.size);
            let texture_size = blur_texture_size(target, rect.size);
            let visible_source =
                source_rect_for_visible_target(sample_rect, rect, capture_size);
            let entry = self.entry_for(target);
            elements.push(FramebufferBlurElement {
                id: entry.id.clone(),
                commit: entry.commit,
                blur_layer: target.blur_layer,
                rect,
                sample_rect,
                material: target.material,
                opacity: target.opacity.clamp(0.0, 1.0),
                texture_size,
                capture_size,
                visible_source,
                output_size,
                target_transform,
                backdrop: match target.blur_layer {
                    BlurLayer::Window => None,
                    BlurLayer::Top | BlurLayer::Overlay => layer_backdrop.clone(),
                },
                inner_cache: Rc::clone(&self.inner_cache),
            });
        }
        elements
    }

    fn entry_for(&mut self, target: &LayerRenderTarget) -> &mut BlurTargetEntry {
        if let Some(index) = self.entries.iter().position(|entry| entry.matches(target)) {
            let entry = &mut self.entries[index];
            if entry.size != target.size || entry.material != target.material {
                entry.commit.increment();
                entry.size = target.size;
                entry.material = target.material;
            }
            return entry;
        }

        self.entries.push(BlurTargetEntry {
            id: Id::new(),
            commit: Default::default(),
            surface: target.surface.clone(),
            blur_layer: target.blur_layer,
            location: target.location,
            size: target.size,
            material: target.material,
            target_opacity: target.opacity,
        });
        self.entries.last_mut().expect("entry just pushed")
    }
}

impl BlurTargetEntry {
    fn matches(&self, target: &LayerRenderTarget) -> bool {
        self.surface == target.surface
            && self.blur_layer == target.blur_layer
            && (self.blur_layer != BlurLayer::Window || self.location == target.location)
            && self.size == target.size
            && self.material == target.material
    }
}

impl Element for FramebufferBlurElement {
    fn id(&self) -> &Id {
        &self.id
    }

    fn current_commit(&self) -> smithay::backend::renderer::utils::CommitCounter {
        self.commit
    }

    fn src(&self) -> Rectangle<f64, Buffer> {
        Rectangle::from_size(Size::<f64, Buffer>::from((
            self.rect.size.w as f64,
            self.rect.size.h as f64,
        )))
    }

    fn geometry(&self, _scale: Scale<f64>) -> Rectangle<i32, Physical> {
        self.rect
    }

    fn transform(&self) -> Transform {
        Transform::Normal
    }

    fn is_framebuffer_effect(&self) -> bool {
        self.backdrop.is_none()
    }

    fn alpha(&self) -> f32 {
        self.opacity
    }

    fn kind(&self) -> Kind {
        Kind::Unspecified
    }

    fn damage_since(
        &self,
        scale: Scale<f64>,
        commit: Option<smithay::backend::renderer::utils::CommitCounter>,
    ) -> smithay::backend::renderer::utils::DamageSet<i32, Physical> {
        use smithay::backend::renderer::utils::DamageSet;
        if commit != Some(self.commit) {
            let mut rects = vec![Rectangle::from_size(self.geometry(scale).size)];
            if self.blur_layer != BlurLayer::Window && self.sample_rect != self.rect {
                let offset = (
                    self.sample_rect.loc.x - self.rect.loc.x,
                    self.sample_rect.loc.y - self.rect.loc.y,
                );
                rects.push(Rectangle::new(offset.into(), self.sample_rect.size));
            }
            return DamageSet::from_slice(&rects);
        }
        DamageSet::default()
    }
}

impl RenderElement<GlesRenderer> for FramebufferBlurElement {
    fn capture_framebuffer(
        &self,
        frame: &mut GlesFrame<'_, '_>,
        _src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        _cache: &UserDataMap,
    ) -> Result<(), GlesError> {
        debug_assert!(self.backdrop.is_none());
        let output_rect = Rectangle::from_size(frame.output_size());
        let Some(_clamped_dst) = dst.intersection(output_rect) else {
            return Ok(());
        };

        let blit_source =
            framebuffer_source_rect(self.output_size, self.target_transform, self.sample_rect);
        self.prepare_blur_inner(
            frame,
            BlurCaptureSource::LiveFramebuffer { blit_source },
        )
    }

    fn draw(
        &self,
        frame: &mut GlesFrame<'_, '_>,
        _src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        _opaque_regions: &[Rectangle<i32, Physical>],
        _cache: Option<&UserDataMap>,
    ) -> Result<(), GlesError> {
        if let Some(backdrop) = &self.backdrop {
            self.prepare_blur_inner(
                frame,
                BlurCaptureSource::Backdrop {
                    texture: &backdrop.texture,
                    generation: backdrop.generation,
                    sample_rect: self.sample_rect,
                },
            )?;
        }

        let inner = self
            .inner_cache
            .borrow()
            .get(&self.id)
            .and_then(|inner| inner.intermediate.clone());
        let Some(texture) = inner else {
            return Ok(());
        };

        let output_rect = Rectangle::from_size(frame.output_size());
        let Some(clamped_dst) = dst.intersection(output_rect) else {
            return Ok(());
        };
        let clamp_offset = clamped_dst.loc - dst.loc;
        let mut filtered_damage = Vec::with_capacity(damage.len());
        for mut rect in damage.iter().copied() {
            if clamped_dst != dst {
                if let Some(mut crop) = rect.intersection(clamped_dst) {
                    crop.loc -= clamp_offset;
                    filtered_damage.push(crop);
                }
            } else {
                rect.loc -= dst.loc;
                filtered_damage.push(rect);
            }
        }
        if filtered_damage.is_empty() {
            return Ok(());
        }

        let texture_source = Rectangle::from_size(
            Size::<f64, Buffer>::from((
                self.texture_size.w as f64,
                self.texture_size.h as f64,
            )),
        );
        frame.render_texture_from_to(
            &texture,
            texture_source,
            clamped_dst,
            &filtered_damage,
            &[],
            Transform::Normal,
            self.opacity,
            None,
            &[],
        )
    }
}

enum BlurCaptureSource<'a> {
    LiveFramebuffer {
        blit_source: Rectangle<i32, Physical>,
    },
    Backdrop {
        texture: &'a GlesTexture,
        generation: u64,
        sample_rect: Rectangle<i32, Physical>,
    },
}

impl FramebufferBlurElement {
    fn prepare_blur_inner(
        &self,
        frame: &mut GlesFrame<'_, '_>,
        source: BlurCaptureSource<'_>,
    ) -> Result<(), GlesError> {
        let mut cache = self.inner_cache.borrow_mut();
        let inner = cache.entry(self.id.clone()).or_insert_with(|| {
            BlurInner::new(
                frame.renderer().as_mut(),
                self.texture_size,
                self.capture_size,
            )
            .expect("blur inner allocation")
        });

        if inner.texture_size != self.texture_size || inner.capture_size != self.capture_size {
            *inner = BlurInner::new(
                frame.renderer().as_mut(),
                self.texture_size,
                self.capture_size,
            )?;
        }

        let needs_refresh = match &source {
            BlurCaptureSource::Backdrop { generation, .. } => {
                inner.backdrop_generation != Some(*generation)
            }
            BlurCaptureSource::LiveFramebuffer { .. } => true,
        };
        if !needs_refresh {
            return Ok(());
        }

        match source {
            BlurCaptureSource::LiveFramebuffer { blit_source } => {
                let capture_target = Rectangle::<i32, Physical>::from_size(self.capture_size);
                {
                    let mut capture_framebuffer = {
                        let mut guard = frame.renderer();
                        guard.as_mut().bind(&mut inner.capture)?
                    };
                    frame.blit_to(
                        &mut capture_framebuffer,
                        blit_source,
                        capture_target,
                        TextureFilter::Linear,
                    )?;
                }
                let capture_source = Rectangle::<f64, Buffer>::from_size(Size::<f64, Buffer>::from((
                    self.capture_size.w as f64,
                    self.capture_size.h as f64,
                )));
                let mut guard = frame.renderer();
                let renderer = guard.as_mut();
                let program = inner.program(renderer)?;
                render_blur_texture(
                    renderer,
                    BlurRenderPass {
                        program: &program,
                        material: self.material,
                        visible_size: self.rect.size,
                        texture_size: self.texture_size,
                        capture_size: self.capture_size,
                        visible_source: self.visible_source,
                        capture_source,
                        capture: &inner.capture,
                        scratch: &mut inner.scratch,
                        blurred: &mut inner.blurred,
                        output: &mut inner.output,
                    },
                )?;
            }
            BlurCaptureSource::Backdrop {
                texture,
                generation,
                sample_rect,
            } => {
                inner.backdrop_generation = Some(generation);
                let capture_source = Rectangle::<f64, Buffer>::new(
                    (sample_rect.loc.x as f64, sample_rect.loc.y as f64).into(),
                    (sample_rect.size.w as f64, sample_rect.size.h as f64).into(),
                );
                let mut guard = frame.renderer();
                let renderer = guard.as_mut();
                let program = inner.program(renderer)?;
                render_blur_texture(
                    renderer,
                    BlurRenderPass {
                        program: &program,
                        material: self.material,
                        visible_size: self.rect.size,
                        texture_size: self.texture_size,
                        capture_size: self.capture_size,
                        visible_source: self.visible_source,
                        capture_source,
                        capture: texture,
                        scratch: &mut inner.scratch,
                        blurred: &mut inner.blurred,
                        output: &mut inner.output,
                    },
                )?;
            }
        }
        inner.intermediate = Some(inner.output.clone());
        Ok(())
    }
}

impl BlurInner {
    fn new(
        renderer: &mut GlesRenderer,
        texture_size: Size<i32, Physical>,
        capture_size: Size<i32, Physical>,
    ) -> Result<Self, GlesError> {
        Ok(Self {
            capture: texture(renderer, capture_size)?,
            scratch: texture(renderer, capture_size)?,
            blurred: texture(renderer, capture_size)?,
            output: texture(renderer, texture_size)?,
            intermediate: None,
            program: None,
            texture_size,
            capture_size,
            backdrop_generation: None,
        })
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
}

fn texture(
    renderer: &mut GlesRenderer,
    size: Size<i32, Physical>,
) -> Result<GlesTexture, GlesError> {
    renderer.create_buffer(
        Fourcc::Abgr8888,
        Size::<i32, Buffer>::from((size.w, size.h)),
    )
}

fn framebuffer_source_rect(
    output_size: Size<i32, Physical>,
    transform: Transform,
    rect: Rectangle<i32, Physical>,
) -> Rectangle<i32, Physical> {
    transform.transform_rect_in(rect, &output_size)
}
