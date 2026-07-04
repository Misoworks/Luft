use crate::{
    damage::DamageTracker,
    scene_composite::SceneCompositeElement,
};
use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            Bind, Offscreen, gles::{GlesError, GlesRenderer, GlesTexture},
        },
    },
    utils::{Buffer, Physical, Size},
};

/// Offscreen copy of the scene below layer-shell blur targets (niri `EffectBuffer` pattern).
///
/// Layer blurs sample this texture instead of live framebuffer capture so the main
/// output can use partial KMS buffer age without reading stale back-buffer pixels.
pub struct SceneBackdrop {
    damage: DamageTracker,
    texture: Option<GlesTexture>,
    size: Size<i32, Physical>,
    generation: u64,
}

impl Default for SceneBackdrop {
    fn default() -> Self {
        Self {
            damage: DamageTracker::from_output_size(Size::from((1, 1))),
            texture: None,
            size: Size::from((0, 0)),
            generation: 0,
        }
    }
}

impl SceneBackdrop {
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn texture(&self) -> Option<&GlesTexture> {
        self.texture.as_ref()
    }

    pub fn render(
        &mut self,
        renderer: &mut GlesRenderer,
        output_size: Size<i32, Physical>,
        elements: &[SceneCompositeElement<'_>],
    ) -> Result<(), GlesError> {
        if elements.is_empty() {
            return Ok(());
        }

        self.ensure_texture(renderer, output_size)?;
        let texture = self
            .texture
            .as_mut()
            .expect("texture ensured before backdrop render");
        let mut target = renderer.bind(texture)?;
        if self.damage.render_output(renderer, &mut target, output_size, 1, elements)?
            .is_some()
        {
            self.generation = self.generation.wrapping_add(1);
        }
        Ok(())
    }

    fn ensure_texture(
        &mut self,
        renderer: &mut GlesRenderer,
        output_size: Size<i32, Physical>,
    ) -> Result<(), GlesError> {
        if self.size == output_size && self.texture.is_some() {
            return Ok(());
        }

        self.size = output_size;
        self.texture = Some(create_texture(renderer, output_size)?);
        self.damage = DamageTracker::from_output_size(output_size);
        self.generation = self.generation.wrapping_add(1);
        Ok(())
    }

    pub fn reset(&mut self, output: &smithay::output::Output) {
        self.damage = DamageTracker::from_output(output);
        self.size = output
            .current_mode()
            .map(|mode| mode.size)
            .unwrap_or_default();
        self.texture = None;
        self.generation = self.generation.wrapping_add(1);
    }
}

fn create_texture(
    renderer: &mut GlesRenderer,
    size: Size<i32, Physical>,
) -> Result<GlesTexture, GlesError> {
    renderer.create_buffer(
        Fourcc::Abgr8888,
        Size::<i32, Buffer>::from((size.w, size.h)),
    )
}
