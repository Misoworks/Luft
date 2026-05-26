use super::BackendError;
#[cfg(feature = "session-backend")]
use crate::output::OutputDescriptor;
#[cfg(feature = "session-backend")]
use smithay::{
    backend::{
        drm::{DrmDevice, DrmDeviceFd, DrmDeviceNotifier},
        session::{Session, libseat::LibSeatSession},
        udev::{UdevBackend, primary_gpu},
    },
    output::Subpixel,
    reexports::{
        drm::control::{Device as ControlDevice, Mode, ResourceHandles, connector, crtc, encoder},
        rustix::fs::OFlags,
    },
    utils::{DevPath, DeviceFd, Physical, Raw, Size},
};
#[cfg(feature = "session-backend")]
use tracing::{debug, info};

#[cfg(feature = "session-backend")]
pub fn run() -> Result<(), BackendError> {
    let mut probe = SessionProbe::new()?;
    let (device, _notifier) = probe.open_primary_drm_device()?;
    let outputs = ConnectedOutputs::discover(&device)?;
    for output in &outputs.outputs {
        let descriptor = &output.descriptor;
        info!(
            connector = %descriptor.name,
            mode = %output.mode_name,
            width = descriptor.size.w,
            height = descriptor.size.h,
            refresh_millihertz = descriptor.refresh_millihertz,
            crtc = ?output.crtc,
            "selected session output"
        );
    }
    Err(BackendError::Unsupported(
        "DRM/KMS session backend selected connected outputs, but modeset rendering is not implemented yet".to_string(),
    ))
}

#[cfg(not(feature = "session-backend"))]
pub fn run() -> Result<(), BackendError> {
    Err(BackendError::Unsupported(
        "DRM/KMS session backend requires building Baton with --features session-backend"
            .to_string(),
    ))
}

#[cfg(feature = "session-backend")]
struct SessionProbe {
    session: LibSeatSession,
    seat: String,
    gpus: Vec<std::path::PathBuf>,
}

#[cfg(feature = "session-backend")]
impl SessionProbe {
    fn new() -> Result<Self, BackendError> {
        let (session, _notifier) = LibSeatSession::new().map_err(|error| {
            BackendError::Unsupported(format!("failed to open a libseat session: {error}"))
        })?;
        let seat = session.seat();
        let udev = UdevBackend::new(&seat).map_err(|error| {
            BackendError::Unsupported(format!("failed to scan DRM devices on {seat}: {error}"))
        })?;
        let gpus = udev
            .device_list()
            .map(|(_, path)| path.to_path_buf())
            .collect::<Vec<_>>();
        info!(seat, gpus = gpus.len(), "opened DRM session probe");
        Ok(Self {
            session,
            seat,
            gpus,
        })
    }

    fn open_primary_drm_device(&mut self) -> Result<(DrmDevice, DrmDeviceNotifier), BackendError> {
        let path = primary_gpu(&self.seat)
            .map_err(|error| {
                BackendError::Unsupported(format!(
                    "failed to select a primary DRM device on {}: {error}",
                    self.seat
                ))
            })?
            .or_else(|| self.gpus.first().cloned())
            .ok_or_else(|| {
                BackendError::Unsupported(format!("no DRM devices found on {}", self.seat))
            })?;
        let fd = self
            .session
            .open(&path, OFlags::RDWR | OFlags::CLOEXEC)
            .map_err(|error| {
                BackendError::Unsupported(format!(
                    "failed to open {} through libseat: {error}",
                    path.display()
                ))
            })?;
        let drm = DrmDeviceFd::new(DeviceFd::from(fd));
        info!(
            path = %path.display(),
            fd_path = ?drm.dev_path(),
            "opened primary DRM device"
        );
        DrmDevice::new(drm, true).map_err(|error| {
            BackendError::Unsupported(format!(
                "failed to initialize DRM device {}: {error}",
                path.display()
            ))
        })
    }
}

#[cfg(feature = "session-backend")]
#[derive(Debug)]
struct ConnectedOutputs {
    outputs: Vec<SessionOutput>,
}

#[cfg(feature = "session-backend")]
impl ConnectedOutputs {
    fn discover(device: &DrmDevice) -> Result<Self, BackendError> {
        let resources = device.resource_handles().map_err(|error| {
            BackendError::Unsupported(format!("failed to read DRM resources: {error}"))
        })?;
        let mut claimed_crtcs = Vec::new();
        let outputs = resources
            .connectors()
            .iter()
            .filter_map(|connector| {
                match SessionOutput::for_connector(device, &resources, *connector, &claimed_crtcs) {
                    Ok(Some(output)) => {
                        claimed_crtcs.push(output.crtc);
                        Some(Ok(output))
                    }
                    Ok(None) => None,
                    Err(error) => Some(Err(error)),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;

        if outputs.is_empty() {
            return Err(BackendError::Unsupported(
                "no connected DRM outputs with usable modes were found".to_string(),
            ));
        }

        Ok(Self { outputs })
    }
}

#[cfg(feature = "session-backend")]
#[derive(Debug)]
struct SessionOutput {
    descriptor: OutputDescriptor,
    mode_name: String,
    crtc: crtc::Handle,
}

#[cfg(feature = "session-backend")]
impl SessionOutput {
    fn for_connector(
        device: &DrmDevice,
        resources: &ResourceHandles,
        connector: connector::Handle,
        claimed_crtcs: &[crtc::Handle],
    ) -> Result<Option<Self>, BackendError> {
        let info = device.get_connector(connector, true).map_err(|error| {
            BackendError::Unsupported(format!(
                "failed to inspect connector {connector:?}: {error}"
            ))
        })?;
        if info.state() != connector::State::Connected {
            debug!(connector = %info, state = ?info.state(), "skipping disconnected DRM connector");
            return Ok(None);
        }

        let Some(mode) = info.modes().first().copied() else {
            debug!(connector = %info, "skipping connected DRM connector without modes");
            return Ok(None);
        };
        let Some(crtc) = select_crtc(device, resources, &info, claimed_crtcs)? else {
            return Err(BackendError::Unsupported(format!(
                "connected DRM connector {info} has no available compatible CRTC"
            )));
        };
        let (width, height) = mode.size();
        let connector_name = info.to_string();
        Ok(Some(Self {
            descriptor: OutputDescriptor {
                name: connector_name.clone(),
                make: "DRM".to_string(),
                model: connector_name,
                physical_size: connector_physical_size(&info),
                subpixel: Subpixel::from(info.subpixel()),
                size: Size::<i32, Physical>::from((i32::from(width), i32::from(height))),
                refresh_millihertz: (mode.vrefresh() as i32).saturating_mul(1000),
            },
            mode_name: mode_name(mode),
            crtc,
        }))
    }
}

#[cfg(feature = "session-backend")]
fn connector_physical_size(info: &connector::Info) -> Size<i32, Raw> {
    info.size()
        .and_then(|(width, height)| Some((i32::try_from(width).ok()?, i32::try_from(height).ok()?)))
        .unwrap_or_default()
        .into()
}

#[cfg(feature = "session-backend")]
fn select_crtc(
    device: &DrmDevice,
    resources: &ResourceHandles,
    connector: &connector::Info,
    claimed: &[crtc::Handle],
) -> Result<Option<crtc::Handle>, BackendError> {
    if let Some(encoder) = connector.current_encoder() {
        if let Some(current) = encoder_crtc(device, encoder)?
            && !claimed.contains(&current)
        {
            return Ok(Some(current));
        }
    }

    for encoder in connector.encoders() {
        let info = device.get_encoder(*encoder).map_err(|error| {
            BackendError::Unsupported(format!("failed to inspect encoder {encoder:?}: {error}"))
        })?;
        if let Some(crtc) = first_available_crtc(resources, &info, claimed) {
            return Ok(Some(crtc));
        }
    }

    Ok(None)
}

#[cfg(feature = "session-backend")]
fn encoder_crtc(
    device: &DrmDevice,
    encoder: encoder::Handle,
) -> Result<Option<crtc::Handle>, BackendError> {
    device
        .get_encoder(encoder)
        .map(|info| info.crtc())
        .map_err(|error| {
            BackendError::Unsupported(format!("failed to inspect encoder {encoder:?}: {error}"))
        })
}

#[cfg(feature = "session-backend")]
fn first_available_crtc(
    resources: &ResourceHandles,
    encoder: &encoder::Info,
    claimed: &[crtc::Handle],
) -> Option<crtc::Handle> {
    resources
        .filter_crtcs(encoder.possible_crtcs())
        .iter()
        .copied()
        .find(|crtc| !claimed.contains(crtc))
}

#[cfg(feature = "session-backend")]
fn mode_name(mode: Mode) -> String {
    mode.name()
        .to_str()
        .map(ToString::to_string)
        .unwrap_or_else(|_| {
            let (width, height) = mode.size();
            format!("{width}x{height}@{}", mode.vrefresh())
        })
}
