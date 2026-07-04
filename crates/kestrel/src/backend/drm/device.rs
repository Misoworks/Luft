use self::outputs::{ConnectedOutput, descriptors};
use super::{DrmError, cursor::HardwareCursor};
use crate::{
    output::OutputDescriptor,
    state::KestrelState,
};
use luft_config::DisplayConfig;
use smithay::{
    backend::{
        allocator::{
            Format, Fourcc,
            dmabuf::Dmabuf,
            gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
        },
        drm::{
            DrmDevice, DrmDeviceFd, DrmDeviceNotifier, compositor::DrmCompositor,
            exporter::gbm::GbmFramebufferExporter,
        },
        egl::{EGLContext, EGLDisplay},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::{Bind, gles::GlesRenderer},
        session::{Session, libseat::LibSeatSession, libseat::LibSeatSessionNotifier},
        udev::{UdevBackend, UdevEvent, primary_gpu},
    },
    output::OutputModeSource,
    reexports::{
        drm::{control::crtc, node::DrmNode},
        input::Libinput,
        rustix::fs::OFlags,
        rustix::fs::stat,
        wayland_server::DisplayHandle,
    },
    utils::{DeviceFd, Scale},
};
use tracing::info;

mod outputs;

const SUPPORTED_COLOR_FORMATS: [Fourcc; 4] = [
    Fourcc::Xrgb8888,
    Fourcc::Xbgr8888,
    Fourcc::Argb8888,
    Fourcc::Abgr8888,
];

pub type SessionCompositor = DrmCompositor<
    GbmAllocator<DrmDeviceFd>,
    GbmFramebufferExporter<DrmDeviceFd>,
    (),
    DrmDeviceFd,
>;

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
    pub fn link_compositor_outputs(&mut self, state: &KestrelState) {
        for session_output in &mut self.outputs {
            let Some(output) = state.outputs.output(&session_output.descriptor.name) else {
                continue;
            };
            session_output
                .compositor
                .set_output_mode_source(OutputModeSource::Auto(output.downgrade()));
        }
    }

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
        self.primary_output().compositor.crtc() == crtc
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

    pub fn sync_cursor(&mut self, state: &mut KestrelState) {
        let crtcs = self
            .outputs
            .iter()
            .map(|output| output.compositor.crtc());
        self.cursor.sync(&self.drm, crtcs, state);
    }

    pub fn frame_submitted(&mut self, crtc: crtc::Handle) -> Result<(), DrmError> {
        let Some(output) = self
            .outputs
            .iter_mut()
            .find(|output| output.compositor.crtc() == crtc)
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
            output
                .compositor
                .reset_state()
                .map_err(compositor_error)?;
            output.compositor.reset_buffers();
        }
        Ok(())
    }
}

pub struct SessionOutput {
    pub descriptor: OutputDescriptor,
    output: ConnectedOutput,
    pub compositor: SessionCompositor,
    frame_queued: bool,
}

impl SessionOutput {
    pub fn mark_frame_queued(&mut self) {
        self.frame_queued = true;
    }

    pub fn has_pending_frame(&self) -> bool {
        self.frame_queued
    }

    fn discard_pending_frame(&mut self) {
        self.frame_queued = false;
    }

    fn frame_submitted(&mut self) -> Result<(), DrmError> {
        if !self.frame_queued {
            return Ok(());
        }
        self.compositor
            .frame_submitted()
            .map_err(compositor_error)?;
        self.frame_queued = false;
        Ok(())
    }
}

fn create_compositor(
    drm: &mut DrmDevice,
    gbm: GbmDevice<DrmDeviceFd>,
    renderer_formats: Vec<Format>,
    import_node: Option<DrmNode>,
    output: &ConnectedOutput,
) -> Result<SessionCompositor, DrmError> {
    let drm_surface = drm
        .create_surface(output.crtc, output.mode, &[output.connector])
        .map_err(|error| DrmError::Unsupported(format!("failed to create DRM surface: {error}")))?;
    let allocator = GbmAllocator::new(gbm.clone(), GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT);
    let exporter = GbmFramebufferExporter::new(gbm.clone(), import_node.into());
    let mode_source = OutputModeSource::Static {
        size: output.descriptor.size,
        scale: output_scale(output.descriptor.scale),
        transform: output.descriptor.transform,
    };

    DrmCompositor::new(
        mode_source,
        drm_surface,
        None,
        allocator,
        exporter,
        SUPPORTED_COLOR_FORMATS,
        renderer_formats,
        drm.cursor_size(),
        Some(gbm),
    )
    .map_err(compositor_error)
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
            let compositor = create_compositor(
                drm,
                gbm.clone(),
                renderer_formats.clone(),
                import_node,
                &output,
            )?;
            Ok(SessionOutput {
                descriptor: output.descriptor.clone(),
                output,
                compositor,
                frame_queued: false,
            })
        })
        .collect()
}

fn output_scale(scale: f64) -> Scale<f64> {
    Scale::from(scale.clamp(0.5, 4.0))
}

fn compositor_error<E: std::fmt::Display>(error: E) -> DrmError {
    DrmError::Unsupported(format!("DRM compositor error: {error}"))
}
