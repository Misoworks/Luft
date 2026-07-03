use super::DrmError;
use crate::output::OutputDescriptor;
use luft_config::{DisplayConfig, OutputConfig};
use smithay::{
    backend::drm::DrmDevice,
    reexports::drm::control::{Device as ControlDevice, Mode, ResourceHandles, connector, crtc},
    utils::{Physical, Raw, Size, Transform},
};
use std::cmp::Ordering;
use tracing::debug;

pub(super) fn descriptors(outputs: &[ConnectedOutput]) -> Vec<OutputDescriptor> {
    outputs
        .iter()
        .map(|output| output.descriptor.clone())
        .collect()
}

#[derive(Clone)]
pub(super) struct ConnectedOutput {
    pub(super) descriptor: OutputDescriptor,
    pub(super) connector: connector::Handle,
    pub(super) mode: Mode,
    pub(super) crtc: crtc::Handle,
}

impl ConnectedOutput {
    pub(super) fn matches(&self, other: &Self) -> bool {
        self.connector == other.connector && self.crtc == other.crtc && self.mode == other.mode
    }

    pub(super) fn discover_all(
        device: &DrmDevice,
        display_config: &DisplayConfig,
    ) -> Result<Vec<Self>, DrmError> {
        let resources = device.resource_handles().map_err(|error| {
            DrmError::Unsupported(format!("failed to read DRM resources: {error}"))
        })?;
        let mut outputs = Vec::new();
        let mut used_crtcs = Vec::new();
        for connector in resources.connectors() {
            match Self::for_connector(device, &resources, *connector, &used_crtcs, display_config) {
                Ok(Some(output)) => {
                    used_crtcs.push(output.crtc);
                    outputs.push(output);
                }
                Ok(None) => {}
                Err(error) => return Err(error),
            }
        }

        if outputs.is_empty() {
            return Err(DrmError::Unsupported(
                "no connected DRM outputs with usable modes were found".to_string(),
            ));
        }
        Ok(outputs)
    }

    fn for_connector(
        device: &DrmDevice,
        resources: &ResourceHandles,
        connector: connector::Handle,
        used_crtcs: &[crtc::Handle],
        display_config: &DisplayConfig,
    ) -> Result<Option<Self>, DrmError> {
        let info = device.get_connector(connector, true).map_err(|error| {
            DrmError::Unsupported(format!(
                "failed to inspect connector {connector:?}: {error}"
            ))
        })?;
        if info.state() != connector::State::Connected {
            debug!(connector = %info, state = ?info.state(), "skipping disconnected DRM connector");
            return Ok(None);
        }

        let connector_name = info.to_string();
        let Some(mode) = select_mode(&info, display_config.outputs.get(&connector_name)) else {
            debug!(connector = %info, "skipping connected DRM connector without modes");
            return Ok(None);
        };
        let Some(crtc) = select_crtc(device, resources, &info, used_crtcs)? else {
            return Err(DrmError::Unsupported(format!(
                "connected DRM connector {info} has no available compatible CRTC"
            )));
        };
        let (width, height) = mode.size();
        Ok(Some(Self {
            descriptor: OutputDescriptor {
                name: connector_name.clone(),
                make: "DRM".to_string(),
                model: connector_name,
                physical_size: connector_physical_size(&info),
                subpixel: info.subpixel().into(),
                size: Size::<i32, Physical>::from((i32::from(width), i32::from(height))),
                refresh_millihertz: i32::try_from(mode.vrefresh().saturating_mul(1000))
                    .unwrap_or(crate::output::DEFAULT_REFRESH_MILLIHERTZ),
                scale: 1.0,
                transform: Transform::Normal,
            },
            connector,
            mode,
            crtc,
        }))
    }
}

fn connector_physical_size(info: &connector::Info) -> Size<i32, Raw> {
    info.size()
        .and_then(|(width, height)| Some((i32::try_from(width).ok()?, i32::try_from(height).ok()?)))
        .unwrap_or_default()
        .into()
}

fn select_mode(info: &connector::Info, output_config: Option<&OutputConfig>) -> Option<Mode> {
    if let Some(mode) = output_config.and_then(|config| configured_mode(info.modes(), config)) {
        return Some(mode);
    }

    info.modes().iter().copied().max_by_key(|mode| {
        let (width, height) = mode.size();
        (
            u64::from(width) * u64::from(height),
            mode_refresh_millihertz(*mode),
        )
    })
}

fn configured_mode(modes: &[Mode], config: &OutputConfig) -> Option<Mode> {
    let mut best = None;
    for mode in modes
        .iter()
        .copied()
        .filter(|mode| mode_matches_config_dimensions(*mode, config))
    {
        if best.is_none_or(|current| compare_configured_modes(mode, current, config).is_gt()) {
            best = Some(mode);
        }
    }
    best
}

fn compare_configured_modes(left: Mode, right: Mode, config: &OutputConfig) -> Ordering {
    if let Some(refresh) = config.refresh_millihertz {
        let left_delta = mode_refresh_millihertz(left).abs_diff(refresh);
        let right_delta = mode_refresh_millihertz(right).abs_diff(refresh);
        let refresh_ordering = right_delta.cmp(&left_delta);
        if refresh_ordering != Ordering::Equal {
            return refresh_ordering;
        }
    }

    default_mode_ordering(left, right)
}

fn default_mode_ordering(left: Mode, right: Mode) -> Ordering {
    let (left_width, left_height) = left.size();
    let (right_width, right_height) = right.size();
    let left_area = u64::from(left_width) * u64::from(left_height);
    let right_area = u64::from(right_width) * u64::from(right_height);
    left_area
        .cmp(&right_area)
        .then_with(|| mode_refresh_millihertz(left).cmp(&mode_refresh_millihertz(right)))
}

fn mode_matches_config_dimensions(mode: Mode, config: &OutputConfig) -> bool {
    let (width, height) = mode.size();
    config
        .width
        .is_none_or(|requested| i32::from(width) == requested)
        && config
            .height
            .is_none_or(|requested| i32::from(height) == requested)
}

fn mode_refresh_millihertz(mode: Mode) -> i32 {
    i32::try_from(mode.vrefresh().saturating_mul(1000))
        .unwrap_or(crate::output::DEFAULT_REFRESH_MILLIHERTZ)
}

fn select_crtc(
    device: &DrmDevice,
    resources: &ResourceHandles,
    connector: &connector::Info,
    used_crtcs: &[crtc::Handle],
) -> Result<Option<crtc::Handle>, DrmError> {
    for encoder in connector.encoders() {
        let info = device.get_encoder(*encoder).map_err(|error| {
            DrmError::Unsupported(format!("failed to inspect encoder {encoder:?}: {error}"))
        })?;
        if let Some(crtc) = info.crtc()
            && !used_crtcs.contains(&crtc)
        {
            return Ok(Some(crtc));
        }
        let filter = info.possible_crtcs();
        if let Some(crtc) = resources.crtcs().iter().find(|crtc| {
            !used_crtcs.contains(crtc)
                && resources
                    .filter_crtcs(filter)
                    .iter()
                    .any(|candidate| candidate == *crtc)
        }) {
            return Ok(Some(*crtc));
        }
    }

    Ok(None)
}
