use super::DrmError;
use crate::output::OutputDescriptor;
use smithay::{
    backend::{
        allocator::{
            Fourcc,
            dmabuf::Dmabuf,
            gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
        },
        drm::{
            DrmDevice, DrmDeviceFd, DrmDeviceNotifier, GbmBufferedSurface, GbmBufferedSurfaceError,
        },
        egl::{EGLContext, EGLDisplay},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::{Bind, gles::GlesRenderer},
        session::{Session, libseat::LibSeatSession, libseat::LibSeatSessionNotifier},
        udev::{UdevBackend, primary_gpu},
    },
    reexports::{
        drm::control::{Device as ControlDevice, Mode, ResourceHandles, connector, crtc},
        input::Libinput,
        rustix::fs::OFlags,
        wayland_server::DisplayHandle,
    },
    utils::{DeviceFd, Physical, Raw, Size},
};
use tracing::{debug, info};

pub type SessionSurface = GbmBufferedSurface<GbmAllocator<DrmDeviceFd>, ()>;

pub struct SessionDevice {
    pub session: LibSeatSession,
    pub session_notifier: LibSeatSessionNotifier,
    pub drm: DrmDevice,
    pub drm_notifier: DrmDeviceNotifier,
    pub surface: SessionSurface,
    pub renderer: GlesRenderer,
    pub input: LibinputInputBackend,
    pub descriptor: OutputDescriptor,
}

pub fn open(_display: &DisplayHandle) -> Result<SessionDevice, DrmError> {
    let (mut session, session_notifier) = LibSeatSession::new().map_err(|error| {
        DrmError::Unsupported(format!("failed to open libseat session: {error}"))
    })?;
    let seat = session.seat();
    let udev = UdevBackend::new(&seat).map_err(|error| {
        DrmError::Unsupported(format!("failed to scan DRM devices on {seat}: {error}"))
    })?;
    let path = primary_gpu(&seat)
        .map_err(|error| {
            DrmError::Unsupported(format!(
                "failed to select a primary DRM device on {seat}: {error}"
            ))
        })?
        .or_else(|| {
            udev.device_list()
                .next()
                .map(|(_, path)| path.to_path_buf())
        })
        .ok_or_else(|| DrmError::Unsupported(format!("no DRM devices found on {seat}")))?;

    let fd = session
        .open(&path, OFlags::RDWR | OFlags::CLOEXEC)
        .map_err(|error| {
            DrmError::Unsupported(format!(
                "failed to open {} through libseat: {error}",
                path.display()
            ))
        })?;
    let drm_fd = DrmDeviceFd::new(DeviceFd::from(fd));
    let (mut drm, drm_notifier) = DrmDevice::new(drm_fd.clone(), true).map_err(|error| {
        DrmError::Unsupported(format!(
            "failed to initialize DRM device {}: {error}",
            path.display()
        ))
    })?;
    let output = ConnectedOutput::discover(&drm)?;

    let gbm = GbmDevice::new(drm_fd.clone())
        .map_err(|error| DrmError::Unsupported(format!("failed to create GBM device: {error}")))?;
    let egl = unsafe { EGLDisplay::new(gbm.clone()) }
        .map_err(|error| DrmError::Unsupported(format!("failed to create EGL display: {error}")))?;
    let context = EGLContext::new(&egl)
        .map_err(|error| DrmError::Unsupported(format!("failed to create EGL context: {error}")))?;
    let renderer = unsafe { GlesRenderer::new(context) }.map_err(|error| {
        DrmError::Unsupported(format!("failed to create GLES renderer: {error}"))
    })?;
    let drm_surface = drm
        .create_surface(output.crtc, output.mode, &[output.connector])
        .map_err(|error| DrmError::Unsupported(format!("failed to create DRM surface: {error}")))?;
    let renderer_formats = <GlesRenderer as Bind<Dmabuf>>::supported_formats(&renderer)
        .ok_or_else(|| {
            DrmError::Unsupported("GLES renderer exposes no GBM render formats".to_string())
        })?;
    let allocator = GbmAllocator::new(gbm, GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT);
    let surface = SessionSurface::new(
        drm_surface,
        allocator,
        &[Fourcc::Argb8888, Fourcc::Xrgb8888],
        renderer_formats,
    )
    .map_err(surface_error)?;

    let mut libinput = Libinput::new_with_udev(LibinputSessionInterface::from(session.clone()));
    libinput
        .udev_assign_seat(&seat)
        .map_err(|()| DrmError::Unsupported(format!("failed to assign libinput to {seat}")))?;
    let input = LibinputInputBackend::new(libinput);

    info!(
        device = %path.display(),
        connector = %output.descriptor.name,
        width = output.descriptor.size.w,
        height = output.descriptor.size.h,
        refresh_millihertz = output.descriptor.refresh_millihertz,
        "opened DRM session device"
    );

    Ok(SessionDevice {
        session,
        session_notifier,
        drm,
        drm_notifier,
        surface,
        renderer,
        input,
        descriptor: output.descriptor,
    })
}

fn surface_error(error: GbmBufferedSurfaceError<std::io::Error>) -> DrmError {
    DrmError::Unsupported(format!("failed to create GBM scanout surface: {error}"))
}

struct ConnectedOutput {
    descriptor: OutputDescriptor,
    connector: connector::Handle,
    mode: Mode,
    crtc: crtc::Handle,
}

impl ConnectedOutput {
    fn discover(device: &DrmDevice) -> Result<Self, DrmError> {
        let resources = device.resource_handles().map_err(|error| {
            DrmError::Unsupported(format!("failed to read DRM resources: {error}"))
        })?;
        for connector in resources.connectors() {
            match Self::for_connector(device, &resources, *connector) {
                Ok(Some(output)) => return Ok(output),
                Ok(None) => {}
                Err(error) => return Err(error),
            }
        }

        Err(DrmError::Unsupported(
            "no connected DRM outputs with usable modes were found".to_string(),
        ))
    }

    fn for_connector(
        device: &DrmDevice,
        resources: &ResourceHandles,
        connector: connector::Handle,
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

        let Some(mode) = info.modes().first().copied() else {
            debug!(connector = %info, "skipping connected DRM connector without modes");
            return Ok(None);
        };
        let Some(crtc) = select_crtc(device, resources, &info)? else {
            return Err(DrmError::Unsupported(format!(
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
                subpixel: info.subpixel().into(),
                size: Size::<i32, Physical>::from((i32::from(width), i32::from(height))),
                refresh_millihertz: i32::try_from(mode.vrefresh().saturating_mul(1000))
                    .unwrap_or(crate::output::DEFAULT_REFRESH_MILLIHERTZ),
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

fn select_crtc(
    device: &DrmDevice,
    resources: &ResourceHandles,
    connector: &connector::Info,
) -> Result<Option<crtc::Handle>, DrmError> {
    for encoder in connector.encoders() {
        let info = device.get_encoder(*encoder).map_err(|error| {
            DrmError::Unsupported(format!("failed to inspect encoder {encoder:?}: {error}"))
        })?;
        if let Some(crtc) = info.crtc() {
            return Ok(Some(crtc));
        }
        let filter = info.possible_crtcs();
        if let Some(crtc) = resources.crtcs().iter().find(|crtc| {
            resources
                .filter_crtcs(filter)
                .iter()
                .any(|candidate| candidate == *crtc)
        }) {
            return Ok(Some(*crtc));
        }
    }

    Ok(None)
}
