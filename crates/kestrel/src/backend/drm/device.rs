use self::outputs::{ConnectedOutput, descriptors};
use super::{DrmError, cursor::HardwareCursor, scanout::DirectScanout};
use crate::output::OutputDescriptor;
use luft_config::DisplayConfig;
use smithay::{
    backend::{
        allocator::{
            Format, Fourcc,
            dmabuf::Dmabuf,
            gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
        },
        drm::{
            DrmDevice, DrmDeviceFd, DrmDeviceNotifier, GbmBufferedSurface, GbmBufferedSurfaceError,
            exporter::gbm::GbmFramebufferExporter,
        },
        egl::{EGLContext, EGLDisplay},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::{Bind, gles::GlesRenderer},
        session::{Session, libseat::LibSeatSession, libseat::LibSeatSessionNotifier},
        udev::{UdevBackend, UdevEvent, primary_gpu},
    },
    reexports::{
        drm::{control::crtc, node::DrmNode},
        input::Libinput,
        rustix::fs::OFlags,
        rustix::fs::stat,
        wayland_server::DisplayHandle,
    },
    utils::DeviceFd,
};
use tracing::info;

mod outputs;

pub type SessionSurface = GbmBufferedSurface<GbmAllocator<DrmDeviceFd>, ()>;

pub struct OpenedSessionDevice {
    pub device: SessionDevice,
    pub sources: SessionSources,
}

pub struct SessionSources {
    pub session_notifier: LibSeatSessionNotifier,
    pub udev: UdevBackend,
    pub drm_notifier: DrmDeviceNotifier,
    pub input: LibinputInputBackend,
}

pub struct SessionDevice {
    _session: LibSeatSession,
    pub active_device_id: u64,
    pub drm: DrmDevice,
    cursor: HardwareCursor,
    outputs: Vec<SessionOutput>,
    primary: usize,
    gbm: GbmDevice<DrmDeviceFd>,
    renderer_formats: Vec<Format>,
    import_node: Option<DrmNode>,
    pub renderer: GlesRenderer,
}

pub fn open(
    _display: &DisplayHandle,
    display_config: &DisplayConfig,
) -> Result<OpenedSessionDevice, DrmError> {
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
    let active_device_id = stat(&path)
        .map_err(|error| {
            DrmError::Unsupported(format!(
                "failed to read DRM device id for {}: {error}",
                path.display()
            ))
        })?
        .st_rdev as u64;
    let drm_fd = DrmDeviceFd::new(DeviceFd::from(fd));
    let (mut drm, drm_notifier) = DrmDevice::new(drm_fd.clone(), true).map_err(|error| {
        DrmError::Unsupported(format!(
            "failed to initialize DRM device {}: {error}",
            path.display()
        ))
    })?;
    let outputs = ConnectedOutput::discover_all(&drm, display_config)?;
    let output = outputs.first().cloned().ok_or_else(|| {
        DrmError::Unsupported("no connected DRM outputs with usable modes were found".to_string())
    })?;

    let gbm = GbmDevice::new(drm_fd.clone())
        .map_err(|error| DrmError::Unsupported(format!("failed to create GBM device: {error}")))?;
    let egl = unsafe { EGLDisplay::new(gbm.clone()) }
        .map_err(|error| DrmError::Unsupported(format!("failed to create EGL display: {error}")))?;
    let context = EGLContext::new(&egl)
        .map_err(|error| DrmError::Unsupported(format!("failed to create EGL context: {error}")))?;
    let renderer = unsafe { GlesRenderer::new(context) }.map_err(|error| {
        DrmError::Unsupported(format!("failed to create GLES renderer: {error}"))
    })?;
    let renderer_formats = <GlesRenderer as Bind<Dmabuf>>::supported_formats(&renderer)
        .ok_or_else(|| {
            DrmError::Unsupported("GLES renderer exposes no GBM render formats".to_string())
        })?
        .into_iter()
        .collect::<Vec<_>>();
    let import_node = DrmNode::from_file(&drm_fd).ok();
    let cursor = HardwareCursor::new(&drm)?;
    let outputs = create_session_outputs(
        &mut drm,
        gbm.clone(),
        renderer_formats.clone(),
        import_node,
        outputs,
    )?;

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

    Ok(OpenedSessionDevice {
        device: SessionDevice {
            _session: session,
            active_device_id,
            drm,
            cursor,
            outputs,
            primary: 0,
            gbm,
            renderer_formats,
            import_node,
            renderer,
        },
        sources: SessionSources {
            session_notifier,
            udev,
            drm_notifier,
            input,
        },
    })
}

impl SessionDevice {
    pub fn rescan_outputs(&mut self, display_config: &DisplayConfig) -> Result<bool, DrmError> {
        let connected = ConnectedOutput::discover_all(&self.drm, display_config)?;
        let output = connected.first().cloned().ok_or_else(|| {
            DrmError::Unsupported(
                "no connected DRM outputs with usable modes were found".to_string(),
            )
        })?;
        let descriptors = descriptors(&connected);
        let descriptors_changed = self.descriptors() != descriptors;
        let primary_changed = !self.primary_output().output.matches(&output);
        if !primary_changed && !descriptors_changed {
            return Ok(false);
        }

        self.reset_surfaces()?;
        self.outputs = create_session_outputs(
            &mut self.drm,
            self.gbm.clone(),
            self.renderer_formats.clone(),
            self.import_node,
            connected,
        )?;
        self.discard_pending_frame();
        self.primary = 0;
        Ok(true)
    }

    pub fn handles_udev_event(&self, event: &UdevEvent) -> bool {
        match event {
            UdevEvent::Changed { device_id } | UdevEvent::Removed { device_id } => {
                *device_id == self.active_device_id
            }
            UdevEvent::Added { .. } => false,
        }
    }

    pub fn descriptors(&self) -> Vec<OutputDescriptor> {
        self.outputs
            .iter()
            .map(|output| output.descriptor.clone())
            .collect()
    }

    pub fn primary_descriptor(&self) -> &OutputDescriptor {
        &self.primary_output().descriptor
    }

    pub fn renderer_and_primary_output(&mut self) -> (&mut GlesRenderer, &mut SessionOutput) {
        (&mut self.renderer, &mut self.outputs[self.primary])
    }

    pub fn renderer_and_outputs(&mut self) -> (&mut GlesRenderer, usize, &mut [SessionOutput]) {
        (&mut self.renderer, self.primary, &mut self.outputs)
    }

    fn primary_output(&self) -> &SessionOutput {
        &self.outputs[self.primary]
    }

    pub fn is_primary_crtc(&self, crtc: crtc::Handle) -> bool {
        self.primary_output().surface.crtc() == crtc
    }

    pub fn drm_device_fd(&self) -> DrmDeviceFd {
        self.drm.device_fd().clone()
    }

    pub fn dmabuf_main_device(&self) -> u64 {
        self.active_device_id
    }

    pub fn frame_pending(&self) -> bool {
        self.outputs.iter().any(SessionOutput::has_pending_frame)
    }

    pub fn discard_pending_frame(&mut self) {
        for output in &mut self.outputs {
            output.discard_pending_frame();
        }
    }

    pub fn sync_cursor(&mut self, state: &mut crate::state::KestrelState) {
        let crtcs = self.outputs.iter().map(|output| output.output.crtc);
        self.cursor.sync(&self.drm, crtcs, state);
    }

    pub fn frame_submitted(&mut self, crtc: crtc::Handle) -> Result<(), DrmError> {
        let Some(output) = self
            .outputs
            .iter_mut()
            .find(|output| output.surface.crtc() == crtc)
        else {
            return Ok(());
        };
        output.frame_submitted()
    }

    pub fn pause(&mut self) {
        self.discard_pending_frame();
        self.drm.pause();
    }

    pub fn activate(&mut self) -> Result<(), DrmError> {
        self.drm.activate(true).map_err(|error| {
            DrmError::Unsupported(format!("failed to reactivate DRM device: {error}"))
        })?;
        self.reset_surfaces()?;
        self.discard_pending_frame();
        Ok(())
    }

    fn reset_surfaces(&mut self) -> Result<(), DrmError> {
        self.cursor.reset();
        for output in &mut self.outputs {
            output.surface.surface().reset_state().map_err(|error| {
                DrmError::Unsupported(format!("failed to reset DRM surface: {error}"))
            })?;
            output.surface.reset_buffer_ages();
        }
        Ok(())
    }
}

pub struct SessionOutput {
    pub descriptor: OutputDescriptor,
    output: ConnectedOutput,
    pub surface: SessionSurface,
    pub direct_scanout: DirectScanout,
    submitted_frame: Option<SubmittedFrame>,
}

impl SessionOutput {
    pub fn mark_frame_submitted(&mut self, frame: SubmittedFrame) {
        self.submitted_frame = Some(frame);
    }

    pub fn has_pending_frame(&self) -> bool {
        self.submitted_frame.is_some() || self.direct_scanout.has_pending_frame()
    }

    fn discard_pending_frame(&mut self) {
        self.direct_scanout.frame_submitted();
        self.submitted_frame = None;
    }

    fn frame_submitted(&mut self) -> Result<(), DrmError> {
        match self.submitted_frame.take() {
            Some(SubmittedFrame::Direct) => self.direct_scanout.frame_submitted(),
            Some(SubmittedFrame::Composited) | None => {
                let _ = self.surface.frame_submitted().map_err(|error| {
                    DrmError::Unsupported(format!("failed to retire submitted DRM frame: {error}"))
                })?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmittedFrame {
    Composited,
    Direct,
}

fn create_surface(
    drm: &mut DrmDevice,
    gbm: GbmDevice<DrmDeviceFd>,
    renderer_formats: Vec<Format>,
    output: &ConnectedOutput,
) -> Result<SessionSurface, DrmError> {
    let drm_surface = drm
        .create_surface(output.crtc, output.mode, &[output.connector])
        .map_err(|error| DrmError::Unsupported(format!("failed to create DRM surface: {error}")))?;
    let allocator = GbmAllocator::new(gbm, GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT);
    SessionSurface::new(
        drm_surface,
        allocator,
        &[Fourcc::Argb8888, Fourcc::Xrgb8888],
        renderer_formats,
    )
    .map_err(surface_error)
}

fn create_session_outputs(
    drm: &mut DrmDevice,
    gbm: GbmDevice<DrmDeviceFd>,
    renderer_formats: Vec<Format>,
    import_node: Option<DrmNode>,
    outputs: Vec<ConnectedOutput>,
) -> Result<Vec<SessionOutput>, DrmError> {
    outputs
        .into_iter()
        .map(|output| {
            let surface = create_surface(drm, gbm.clone(), renderer_formats.clone(), &output)?;
            let direct_scanout =
                DirectScanout::new(GbmFramebufferExporter::new(gbm.clone(), import_node));
            Ok(SessionOutput {
                descriptor: output.descriptor.clone(),
                output,
                surface,
                direct_scanout,
                submitted_frame: None,
            })
        })
        .collect()
}

fn surface_error(error: GbmBufferedSurfaceError<std::io::Error>) -> DrmError {
    DrmError::Unsupported(format!("failed to create GBM scanout surface: {error}"))
}
