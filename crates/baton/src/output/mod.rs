use crate::state::BatonState;
use smithay::{
    output::{Mode, Output, PhysicalProperties, Scale, Subpixel},
    reexports::wayland_server::DisplayHandle,
    utils::{Physical, Raw, Size, Transform},
};

#[derive(Debug, Clone)]
pub struct OutputDescriptor {
    pub name: String,
    pub make: String,
    pub model: String,
    pub physical_size: Size<i32, Raw>,
    pub subpixel: Subpixel,
    pub size: Size<i32, Physical>,
    pub refresh_millihertz: i32,
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
            name: "staccato-nested-1".to_string(),
            make: "Staccato".to_string(),
            model: "Nested".to_string(),
            physical_size: (340, 210).into(),
            subpixel: Subpixel::Unknown,
            size: self.size,
            refresh_millihertz: self.refresh_millihertz,
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

    output.create_global::<BatonState>(display);
    configure_output(&output, descriptor.size, descriptor.refresh_millihertz);
    output
}

pub fn configure_output(output: &Output, size: Size<i32, Physical>, refresh_millihertz: i32) {
    let mode = Mode {
        size: normalized_size(size),
        refresh: normalize_refresh(refresh_millihertz),
    };

    output.change_current_state(
        Some(mode),
        Some(Transform::Normal),
        Some(Scale::Integer(1)),
        Some((0, 0).into()),
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
