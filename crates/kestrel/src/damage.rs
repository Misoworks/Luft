use smithay::{
    backend::renderer::{
        Color32F,
        damage::OutputDamageTracker,
        gles::{GlesError, GlesRenderer, GlesTarget},
    },
    utils::{Physical, Rectangle, Size, Transform},
};

#[derive(Debug)]
pub struct DamageTracker {
    output_size: Size<i32, Physical>,
    transform: Transform,
    tracker: OutputDamageTracker,
}

pub const SCENE_CLEAR_COLOR: Color32F = Color32F::new(0.08, 0.085, 0.09, 1.0);

impl DamageTracker {
    pub fn from_output(output: &smithay::output::Output) -> Self {
        let output_size = output
            .current_mode()
            .map(|mode| mode.size)
            .unwrap_or_default();
        Self::from_output_size(output_size).with_transform(output.current_transform())
    }

    pub fn from_output_size(output_size: Size<i32, Physical>) -> Self {
        Self {
            output_size,
            transform: Transform::Normal,
            tracker: output_tracker(output_size, Transform::Normal),
        }
    }

    fn with_transform(mut self, transform: Transform) -> Self {
        self.transform = transform;
        self.tracker = output_tracker(self.output_size, transform);
        self
    }

    pub fn render_output<E>(
        &mut self,
        renderer: &mut GlesRenderer,
        framebuffer: &mut GlesTarget<'_>,
        output_size: Size<i32, Physical>,
        buffer_age: usize,
        elements: &[E],
    ) -> Result<Option<Vec<Rectangle<i32, Physical>>>, GlesError>
    where
        E: smithay::backend::renderer::element::RenderElement<GlesRenderer>,
    {
        if self.output_size != output_size {
            self.output_size = output_size;
            self.tracker = output_tracker(output_size, self.transform);
        }

        let result = self.tracker.render_output(
            renderer,
            framebuffer,
            buffer_age,
            elements,
            SCENE_CLEAR_COLOR,
        );

        match result {
            Ok(output) => Ok(output.damage.cloned()),
            Err(smithay::backend::renderer::damage::Error::Rendering(error)) => Err(error),
            Err(smithay::backend::renderer::damage::Error::OutputNoMode(_)) => Ok(None),
        }
    }

    pub fn reset(&mut self, output: &smithay::output::Output) {
        *self = Self::from_output(output);
    }
}

fn output_tracker(output_size: Size<i32, Physical>, transform: Transform) -> OutputDamageTracker {
    OutputDamageTracker::new(output_size, 1.0, transform)
}
