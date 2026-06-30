use super::{DrmError, cursor::HardwareCursor, scanout::DirectScanout};
use crate::output::OutputDescriptor;
use asher_config::{DisplayConfig, OutputConfig};
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
        drm::{
            control::{Device as ControlDevice, Mode, ResourceHandles, connector, crtc},
            node::DrmNode,
        },
        input::Libinput,
        rustix::fs::OFlags,
        rustix::fs::stat,
        wayland_server::DisplayHandle,
    },
    utils::{DeviceFd, Physical, Raw, Size, Transform},
};
use std::cmp::Ordering;
use tracing::{debug, info};

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
                *device_id as u64 == self.active_device_id
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

    pub fn direct_scanout_pending(&self) -> bool {
        self.outputs
            .iter()
            .any(SessionOutput::direct_scanout_pending)
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

    fn direct_scanout_pending(&self) -> bool {
        self.direct_scanout.has_pending_frame()
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

fn descriptors(outputs: &[ConnectedOutput]) -> Vec<OutputDescriptor> {
    outputs
        .iter()
        .map(|output| output.descriptor.clone())
        .collect()
}

#[derive(Clone)]
pub struct ConnectedOutput {
    descriptor: OutputDescriptor,
    connector: connector::Handle,
    mode: Mode,
    crtc: crtc::Handle,
}

impl ConnectedOutput {
    fn matches(&self, other: &Self) -> bool {
        self.connector == other.connector && self.crtc == other.crtc && self.mode == other.mode
    }

    fn discover_all(
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
