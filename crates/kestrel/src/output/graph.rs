use super::{OutputDescriptor, configure_output_at, create_output};
use luft_config::DisplayConfig;
use luft_ipc::OutputSummary;
use smithay::{
    output::Output,
    reexports::wayland_server::DisplayHandle,
    utils::{Logical, Physical, Point, Size},
};
use std::collections::BTreeMap;

pub struct OutputGraph {
    outputs: BTreeMap<String, ManagedOutput>,
    order: Vec<String>,
    primary: String,
}

#[derive(Clone)]
pub struct ManagedOutput {
    pub descriptor: OutputDescriptor,
    pub output: Output,
    pub location: Point<i32, Logical>,
    pub enabled: bool,
}

impl OutputGraph {
    pub fn new(
        display: &DisplayHandle,
        config: &DisplayConfig,
        descriptors: Vec<OutputDescriptor>,
    ) -> Self {
        let mut order = Vec::new();
        let mut outputs = descriptors
            .into_iter()
            .map(|descriptor| {
                let output = managed_output(display, config, descriptor);
                order.push(output.descriptor.name.clone());
                (output.descriptor.name.clone(), output)
            })
            .collect::<BTreeMap<_, _>>();
        if outputs.is_empty() {
            let output =
                managed_output(display, config, super::NestedOutput::default().descriptor());
            order.push(output.descriptor.name.clone());
            outputs.insert(output.descriptor.name.clone(), output);
        }

        let mut graph = Self {
            outputs,
            order,
            primary: String::new(),
        };
        graph.primary = graph.select_primary(config);
        graph
    }

    pub fn primary(&self) -> &ManagedOutput {
        self.outputs
            .get(&self.primary)
            .or_else(|| self.outputs.values().next())
            .expect("Kestrel output graph must contain at least one output")
    }

    pub fn primary_mut(&mut self) -> &mut ManagedOutput {
        let fallback = self
            .order
            .iter()
            .find(|name| self.outputs.contains_key(*name))
            .cloned()
            .or_else(|| self.outputs.keys().next().cloned())
            .expect("Kestrel output graph must contain at least one output");
        let primary = if self.outputs.contains_key(&self.primary) {
            self.primary.clone()
        } else {
            fallback
        };
        self.outputs
            .get_mut(&primary)
            .expect("Kestrel output graph primary lookup must be stable")
    }

    pub fn primary_output(&self) -> &Output {
        &self.primary().output
    }

    pub fn primary_size(&self) -> Size<i32, Physical> {
        self.primary().descriptor.size
    }

    #[cfg(feature = "session-backend")]
    pub fn primary_refresh_millihertz(&self) -> i32 {
        self.primary().descriptor.refresh_millihertz
    }

    pub fn primary_scale(&self) -> f64 {
        self.primary().output.current_scale().fractional_scale()
    }

    pub fn primary_transform(&self) -> smithay::utils::Transform {
        self.primary().descriptor.transform
    }

    pub fn contains(&self, name: &str) -> bool {
        self.outputs.contains_key(name)
    }

    pub fn scale(&self, name: Option<&str>) -> Option<f64> {
        let output = match name {
            Some(name) => self.outputs.get(name)?,
            None => self.primary(),
        };
        Some(output.output.current_scale().fractional_scale())
    }

    #[cfg(feature = "session-backend")]
    pub fn replace(
        &mut self,
        display: &DisplayHandle,
        config: &DisplayConfig,
        descriptors: Vec<OutputDescriptor>,
    ) {
        let mut existing = std::mem::take(&mut self.outputs);
        let mut outputs = BTreeMap::new();
        let mut order = Vec::new();
        for descriptor in descriptors {
            if let Some(mut output) = existing.remove(&descriptor.name) {
                output.descriptor = configured_descriptor(config, descriptor);
                output.location = configured_location(config, &output.descriptor.name);
                output.enabled = configured_enabled(config, &output.descriptor.name);
                configure_managed_output(&output);
                order.push(output.descriptor.name.clone());
                outputs.insert(output.descriptor.name.clone(), output);
            } else {
                let output = managed_output(display, config, descriptor);
                order.push(output.descriptor.name.clone());
                outputs.insert(output.descriptor.name.clone(), output);
            }
        }

        if outputs.is_empty() {
            let output =
                managed_output(display, config, super::NestedOutput::default().descriptor());
            order.push(output.descriptor.name.clone());
            outputs.insert(output.descriptor.name.clone(), output);
        }
        self.outputs = outputs;
        self.order = order;
        self.primary = self.select_primary(config);
    }

    pub fn set_primary_size(&mut self, size: Size<i32, Physical>) {
        let output = self.primary_mut();
        output.descriptor.size = size;
        configure_managed_output(output);
    }

    pub fn set_primary_refresh_millihertz(&mut self, refresh_millihertz: i32) -> bool {
        let output = self.primary_mut();
        if output.descriptor.refresh_millihertz == refresh_millihertz {
            return false;
        }

        output.descriptor.refresh_millihertz = refresh_millihertz;
        configure_managed_output(output);
        true
    }

    pub fn set_scale(&mut self, name: Option<&str>, scale: f64) -> Option<bool> {
        let scale = scale.clamp(0.5, 4.0);
        let output = match name {
            Some(name) => self.outputs.get_mut(name)?,
            None => self.primary_mut(),
        };
        if (output.output.current_scale().fractional_scale() - scale).abs() < f64::EPSILON {
            return Some(false);
        }

        output.descriptor.scale = scale;
        configure_managed_output(output);
        Some(true)
    }

    pub fn summaries(&self) -> Vec<OutputSummary> {
        self.order
            .iter()
            .filter_map(|name| self.outputs.get(name))
            .map(|output| output.summary(output.descriptor.name == self.primary))
            .collect()
    }

    fn select_primary(&self, config: &DisplayConfig) -> String {
        config
            .primary
            .as_ref()
            .filter(|primary| self.outputs.contains_key(*primary))
            .cloned()
            .or_else(|| {
                self.order.iter().find_map(|name| {
                    self.outputs
                        .get(name)
                        .filter(|output| output.enabled)
                        .map(|output| output.descriptor.name.clone())
                })
            })
            .or_else(|| {
                self.order
                    .iter()
                    .find(|name| self.outputs.contains_key(*name))
                    .cloned()
            })
            .or_else(|| self.outputs.keys().next().cloned())
            .expect("Kestrel output graph must contain at least one output")
    }
}

impl ManagedOutput {
    fn summary(&self, primary: bool) -> OutputSummary {
        OutputSummary {
            name: self.descriptor.name.clone(),
            make: self.descriptor.make.clone(),
            model: self.descriptor.model.clone(),
            width: self.descriptor.size.w,
            height: self.descriptor.size.h,
            refresh_millihertz: self.descriptor.refresh_millihertz,
            scale: self.output.current_scale().fractional_scale(),
            primary,
            enabled: self.enabled,
        }
    }
}

fn managed_output(
    display: &DisplayHandle,
    config: &DisplayConfig,
    descriptor: OutputDescriptor,
) -> ManagedOutput {
    let descriptor = configured_descriptor(config, descriptor);
    let output = create_output(display, &descriptor);
    ManagedOutput {
        location: configured_location(config, &descriptor.name),
        enabled: configured_enabled(config, &descriptor.name),
        descriptor,
        output,
    }
}

fn configured_descriptor(
    config: &DisplayConfig,
    mut descriptor: OutputDescriptor,
) -> OutputDescriptor {
    descriptor.scale = config.output_scale(&descriptor.name);
    descriptor
}

fn configured_location(config: &DisplayConfig, name: &str) -> Point<i32, Logical> {
    config
        .outputs
        .get(name)
        .map(|output| (output.x, output.y).into())
        .unwrap_or_else(|| (0, 0).into())
}

fn configured_enabled(config: &DisplayConfig, name: &str) -> bool {
    config.outputs.get(name).is_none_or(|output| output.enabled)
}

fn configure_managed_output(output: &ManagedOutput) {
    configure_output_at(
        &output.output,
        output.descriptor.size,
        output.descriptor.refresh_millihertz,
        output.descriptor.scale,
        output.location,
        output.descriptor.transform,
    );
}
