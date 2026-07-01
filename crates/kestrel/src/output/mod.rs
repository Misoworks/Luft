use crate::state::KestrelState;
mod graph;

pub use graph::OutputGraph;

use smithay::{
    output::{Mode, Output, PhysicalProperties, Scale, Subpixel},
    reexports::wayland_server::DisplayHandle,
    utils::{Logical, Physical, Point, Raw, Size, Transform},
};

#[derive(Debug, Clone, PartialEq)]
pub struct OutputDescriptor {
    pub name: String,
    pub make: String,
    pub model: String,
    pub physical_size: Size<i32, Raw>,
    pub subpixel: Subpixel,
    pub size: Size<i32, Physical>,
    pub refresh_millihertz: i32,
    pub scale: f64,
    pub transform: Transform,
}

#[derive(Debug, Clone, Copy)]
pub struct NestedOutput {
    pub size: Size<i32, Physical>,
    pub refresh_millihertz: i32,
}

pub const DEFAULT_REFRESH_MILLIHERTZ: i32 = 60_000;

impl Default for NestedOutput {
    fn default() -> Self {
        Self {
            size: (1280, 800).into(),
            refresh_millihertz: DEFAULT_REFRESH_MILLIHERTZ,
        }
    }
}

impl NestedOutput {
    pub fn descriptor(self) -> OutputDescriptor {
        OutputDescriptor {
            name: "asher-nested-1".to_string(),
            make: "Asher".to_string(),
            model: "Nested".to_string(),
            physical_size: (340, 210).into(),
            subpixel: Subpixel::Unknown,
            size: self.size,
            refresh_millihertz: self.refresh_millihertz,
            scale: 1.0,
            transform: Transform::Flipped180,
        }
    }

    pub fn resize(&mut self, size: Size<i32, Physical>) -> bool {
        if self.size == size {
            return false;
        }

        self.size = size;
        true
    }

    pub fn set_refresh(&mut self, refresh_millihertz: i32) -> bool {
        let refresh_millihertz = normalize_refresh(refresh_millihertz);
        if self.refresh_millihertz == refresh_millihertz {
            return false;
        }

        self.refresh_millihertz = refresh_millihertz;
        true
    }
}

pub fn create_output(display: &DisplayHandle, descriptor: &OutputDescriptor) -> Output {
    let output = Output::new(
        descriptor.name.clone(),
        PhysicalProperties {
            size: descriptor.physical_size,
            subpixel: descriptor.subpixel,
            make: descriptor.make.clone(),
            model: descriptor.model.clone(),
        },
    );

    output.create_global::<KestrelState>(display);
    configure_output_at(
        &output,
        descriptor.size,
        descriptor.refresh_millihertz,
        descriptor.scale,
        (0, 0).into(),
        descriptor.transform,
    );
    output
}

#[cfg(feature = "session-backend")]
#[allow(dead_code)]
pub fn configure_output(
    output: &Output,
    size: Size<i32, Physical>,
    refresh_millihertz: i32,
    scale: f64,
) {
    configure_output_at(
        output,
        size,
        refresh_millihertz,
        scale,
        (0, 0).into(),
        Transform::Normal,
    );
}

pub fn configure_output_at(
    output: &Output,
    size: Size<i32, Physical>,
    refresh_millihertz: i32,
    scale: f64,
    location: Point<i32, Logical>,
    transform: Transform,
) {
    let mode = Mode {
        size: normalized_size(size),
        refresh: normalize_refresh(refresh_millihertz),
    };

    output.change_current_state(
        Some(mode),
        Some(transform),
        Some(output_scale(scale)),
        Some(location),
    );
    output.set_preferred(mode);
}

fn normalized_size(size: Size<i32, Physical>) -> Size<i32, Physical> {
    (size.w.max(1), size.h.max(1)).into()
}

fn normalize_refresh(refresh_millihertz: i32) -> i32 {
    if refresh_millihertz > 0 {
        refresh_millihertz
    } else {
        DEFAULT_REFRESH_MILLIHERTZ
    }
}

fn output_scale(scale: f64) -> Scale {
    let scale = scale.clamp(0.5, 4.0);
    if (scale.round() - scale).abs() < f64::EPSILON {
        Scale::Integer(scale.round() as i32)
    } else {
        Scale::Fractional(scale)
    }
}
