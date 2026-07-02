use super::{DrmError, device::SessionSurface};
use crate::state::KestrelState;
use smithay::{
    backend::{
        allocator::gbm::GbmBuffer,
        drm::{
            Framebuffer, PlaneConfig, PlaneState,
            exporter::{ExportBuffer, ExportFramebuffer, gbm::GbmFramebufferExporter},
            gbm::GbmFramebuffer,
        },
        renderer::{
            element::{
                Element, RenderElement, UnderlyingStorage,
                surface::{WaylandSurfaceRenderElement, render_elements_from_surface_tree},
            },
            gles::GlesRenderer,
            utils::Buffer as RendererBuffer,
        },
    },
    utils::{Physical, Rectangle, Scale, Transform},
};
use tracing::debug;

pub struct DirectScanout {
    exporter: GbmFramebufferExporter<smithay::backend::drm::DrmDeviceFd>,
    pending: Option<PendingDirectFrame>,
}

struct PendingDirectFrame {
    _buffer: RendererBuffer,
    _framebuffer: GbmFramebuffer,
}

impl DirectScanout {
    pub fn new(exporter: GbmFramebufferExporter<smithay::backend::drm::DrmDeviceFd>) -> Self {
        Self {
            exporter,
            pending: None,
        }
    }

    pub fn has_pending_frame(&self) -> bool {
        self.pending.is_some()
    }

    pub fn frame_submitted(&mut self) {
        self.pending = None;
    }

    pub fn try_queue(
        &mut self,
        state: &KestrelState,
        renderer: &mut GlesRenderer,
        surface: &SessionSurface,
    ) -> Result<bool, DrmError> {
        if self.pending.is_some() {
            return Ok(false);
        }

        let Some(element) = fullscreen_element(state, renderer) else {
            return Ok(false);
        };
        if !eligible_for_primary_scanout(&element, state.output_size()) {
            return Ok(false);
        }

        let Some(UnderlyingStorage::Wayland(buffer)) = element.underlying_storage(renderer) else {
            return Ok(false);
        };
        let buffer = buffer.clone();
        let export = ExportBuffer::<GbmBuffer>::Wayland(&buffer);
        if !self.exporter.can_add_framebuffer(&export) {
            return Ok(false);
        }

        let Some(framebuffer) = self
            .exporter
            .add_framebuffer(surface.surface().device_fd(), export, true)
            .map_err(scanout_error)?
        else {
            return Ok(false);
        };

        if !surface
            .surface()
            .plane_info()
            .formats
            .contains(&framebuffer.format())
        {
            return Ok(false);
        }

        let plane = plane_state(surface, &element, &framebuffer, state.output_size());
        if surface.surface().test_state([plane], false).is_err() {
            return Ok(false);
        }

        let plane = plane_state(surface, &element, &framebuffer, state.output_size());
        let result = if surface.surface().commit_pending() {
            surface.surface().commit([plane], true)
        } else {
            surface.surface().page_flip([plane], true)
        };
        if result.is_err() {
            return Ok(false);
        }

        self.pending = Some(PendingDirectFrame {
            _buffer: buffer,
            _framebuffer: framebuffer,
        });
        debug!("queued fullscreen client on the DRM primary plane");
        Ok(true)
    }
}

fn fullscreen_element(
    state: &KestrelState,
    renderer: &mut GlesRenderer,
) -> Option<WaylandSurfaceRenderElement<GlesRenderer>> {
    if state.animations_active() || state.workspace_transition().is_some() {
        return None;
    }

    let window = state
        .windows
        .fullscreen_window(state.layout.active_workspace())?;
    if window.server_decorated || window.hidden || window.closing {
        return None;
    }

    let transform = window.render_transform(0, state.output_size());
    if transform.scale != 1.0 || transform.alpha != 1.0 {
        return None;
    }

    let location = window.surface_location();
    let elements = render_elements_from_surface_tree(
        renderer,
        window.surface.wl_surface(),
        (location.x, location.y),
        1.0,
        1.0,
        smithay::backend::renderer::element::Kind::Unspecified,
    );
    if elements.len() == 1 {
        elements.into_iter().next()
    } else {
        None
    }
}

fn eligible_for_primary_scanout(
    element: &WaylandSurfaceRenderElement<GlesRenderer>,
    output_size: smithay::utils::Size<i32, Physical>,
) -> bool {
    if element.alpha() != 1.0 || element.transform() != Transform::Normal {
        return false;
    }

    let output = Rectangle::from_size(output_size);
    if element.geometry(Scale::from(1.0)) != output {
        return false;
    }

    output
        .subtract_rects(element.opaque_regions(Scale::from(1.0)))
        .is_empty()
}

fn plane_state<'a>(
    surface: &SessionSurface,
    element: &WaylandSurfaceRenderElement<GlesRenderer>,
    framebuffer: &'a GbmFramebuffer,
    output_size: smithay::utils::Size<i32, Physical>,
) -> PlaneState<'a> {
    PlaneState {
        handle: surface.plane(),
        config: Some(PlaneConfig {
            src: element.src(),
            dst: Rectangle::from_size(output_size),
            transform: Transform::Normal,
            alpha: 1.0,
            damage_clips: None,
            fb: *framebuffer.as_ref(),
            fence: None,
        }),
    }
}

fn scanout_error(error: impl std::fmt::Display) -> DrmError {
    DrmError::Unsupported(format!(
        "failed to export fullscreen scanout buffer: {error}"
    ))
}
