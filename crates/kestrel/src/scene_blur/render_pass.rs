use crate::layers::LayerMaterial;
use smithay::{
    backend::renderer::{
        Bind, Frame, Renderer,
        gles::{GlesError, GlesRenderer, GlesTexProgram, GlesTexture, Uniform, UniformValue},
    },
    utils::{Buffer, Physical, Rectangle, Size, Transform},
};

use super::geometry::material_radius;

pub(super) struct BlurRenderPass<'a> {
    pub program: &'a GlesTexProgram,
    pub material: LayerMaterial,
    pub visible_size: Size<i32, Physical>,
    pub texture_size: Size<i32, Physical>,
    pub capture_size: Size<i32, Physical>,
    pub visible_source: Rectangle<f64, Buffer>,
    pub capture_source: Rectangle<f64, Buffer>,
    pub capture: &'a GlesTexture,
    pub scratch: &'a mut GlesTexture,
    pub blurred: &'a mut GlesTexture,
    pub output: &'a mut GlesTexture,
}

pub(super) fn render_blur_texture(
    renderer: &mut GlesRenderer,
    pass: BlurRenderPass<'_>,
) -> Result<(), GlesError> {
    let full_damage = [Rectangle::<i32, Physical>::from_size(pass.texture_size)];
    let capture_damage = [Rectangle::<i32, Physical>::from_size(pass.capture_size)];
    {
        let mut scratch_target = renderer.bind(pass.scratch)?;
        let mut frame =
            renderer.render(&mut scratch_target, pass.capture_size, Transform::Normal)?;
        frame.clear(
            smithay::backend::renderer::Color32F::new(0.0, 0.0, 0.0, 0.0),
            &capture_damage,
        )?;
        frame.render_texture_from_to(
            pass.capture,
            pass.capture_source,
            Rectangle::<i32, Physical>::from_size(pass.capture_size),
            &capture_damage,
            &[],
            Transform::Normal,
            1.0,
            Some(pass.program),
            &blur_uniforms(BlurUniforms {
                texel_size: pass.capture_size,
                target_size: pass.capture_size,
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
            renderer.render(&mut blurred_target, pass.capture_size, Transform::Normal)?;
        frame.clear(
            smithay::backend::renderer::Color32F::new(0.0, 0.0, 0.0, 0.0),
            &capture_damage,
        )?;
        let horizontal_source = Rectangle::<f64, Buffer>::from_size(Size::<f64, Buffer>::from((
            pass.capture_size.w as f64,
            pass.capture_size.h as f64,
        )));
        frame.render_texture_from_to(
            pass.scratch,
            horizontal_source,
            Rectangle::<i32, Physical>::from_size(pass.capture_size),
            &capture_damage,
            &[],
            Transform::Normal,
            1.0,
            Some(pass.program),
            &blur_uniforms(BlurUniforms {
                texel_size: pass.capture_size,
                target_size: pass.capture_size,
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
        let mut output_target = renderer.bind(pass.output)?;
        let mut frame =
            renderer.render(&mut output_target, pass.texture_size, Transform::Normal)?;
        frame.clear(
            smithay::backend::renderer::Color32F::new(0.0, 0.0, 0.0, 0.0),
            &full_damage,
        )?;
        frame.render_texture_from_to(
            pass.blurred,
            pass.visible_source,
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

struct BlurUniforms {
    texel_size: Size<i32, Physical>,
    target_size: Size<i32, Physical>,
    visible_size: Size<i32, Physical>,
    material: LayerMaterial,
    direction: (f32, f32),
    final_pass: bool,
    mask_pass: bool,
}

fn blur_uniforms(uniforms: BlurUniforms) -> [Uniform<'static>; 7] {
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
            "shape",
            UniformValue::_1f(match uniforms.material {
                LayerMaterial::Rect => 0.0,
                LayerMaterial::RoundRect { .. } => 1.0,
                LayerMaterial::RoundTop { .. } => 2.0,
                LayerMaterial::RoundLeft { .. } => 3.0,
                LayerMaterial::RoundRight { .. } => 4.0,
            }),
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
