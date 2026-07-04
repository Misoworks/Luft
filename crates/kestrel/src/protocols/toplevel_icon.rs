use crate::state::KestrelState;
use image::{ColorType, ImageEncoder, codecs::png::PngEncoder};
use smithay::{
    reexports::{
        wayland_protocols::xdg::toplevel_icon::v1::server::{
            xdg_toplevel_icon_manager_v1::{self, XdgToplevelIconManagerV1},
            xdg_toplevel_icon_v1::{self, XdgToplevelIconV1},
        },
        wayland_server::{
            Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource,
            backend::GlobalId,
            protocol::{wl_buffer::WlBuffer, wl_shm, wl_surface::WlSurface},
        },
    },
    wayland::{
        compositor::{self, Cacheable},
        shm::{self, BufferAccessError},
    },
};
use std::{io::Cursor, sync::Mutex};

#[derive(Debug, Clone, Default)]
pub struct ToplevelIconSummary {
    pub uri: Option<String>,
    pub name: Option<String>,
}

pub fn toplevel_icon_for_surface(wl_surface: &WlSurface) -> ToplevelIconSummary {
    let icon = icon_snapshot_for_surface(wl_surface);
    ToplevelIconSummary {
        uri: icon_pixmap_uri(&icon),
        name: icon.icon_name.clone(),
    }
}

fn icon_snapshot_for_surface(wl_surface: &WlSurface) -> IconSnapshot {
    compositor::with_states(wl_surface, |states| {
        let mut cached = states.cached_state.get::<ToplevelIconSurfaceState>();
        let pending = cached.pending().icon.clone();
        if pending.buffers.is_empty() && pending.icon_name.is_none() {
            cached.current().icon.clone()
        } else {
            pending
        }
    })
}

fn icon_pixmap_uri(icon: &IconSnapshot) -> Option<String> {
    let buffer = best_icon_buffer(&icon.buffers)?;
    let rgba = buffer_to_rgba(buffer)?;
    pixmap_data_uri(buffer.width, buffer.height, &rgba)
}

fn best_icon_buffer(buffers: &[CopiedIconBuffer]) -> Option<&CopiedIconBuffer> {
    buffers
        .iter()
        .filter(|buffer| {
            buffer.width > 0
                && buffer.height > 0
                && buffer.stride > 0
                && !buffer.pixels.is_empty()
        })
        .max_by_key(|buffer| buffer.width * buffer.height)
}

fn buffer_to_rgba(buffer: &CopiedIconBuffer) -> Option<Vec<u8>> {
    let width = usize::try_from(buffer.width).ok()?;
    let height = usize::try_from(buffer.height).ok()?;
    let stride = usize::try_from(buffer.stride).ok()?;
    if width == 0 || height == 0 || stride < width * 4 {
        return None;
    }

    let mut rgba = vec![0_u8; width * height * 4];
    for y in 0..height {
        let row_start = y * stride;
        if row_start + width * 4 > buffer.pixels.len() {
            return None;
        }
        for x in 0..width {
            let src = row_start + x * 4;
            let dst = (y * width + x) * 4;
            write_rgba_pixel(
                &mut rgba[dst..dst + 4],
                &buffer.pixels[src..src + 4],
                buffer.format,
            );
        }
    }
    Some(rgba)
}

fn write_rgba_pixel(out: &mut [u8], pixel: &[u8], format: wl_shm::Format) {
    match format {
        wl_shm::Format::Argb8888 => {
            out[0] = pixel[1];
            out[1] = pixel[2];
            out[2] = pixel[3];
            out[3] = pixel[0];
        }
        wl_shm::Format::Xrgb8888 => {
            out[0] = pixel[1];
            out[1] = pixel[2];
            out[2] = pixel[3];
            out[3] = 255;
        }
        wl_shm::Format::Abgr8888 => {
            out[0] = pixel[3];
            out[1] = pixel[2];
            out[2] = pixel[1];
            out[3] = pixel[0];
        }
        wl_shm::Format::Xbgr8888 => {
            out[0] = pixel[3];
            out[1] = pixel[2];
            out[2] = pixel[1];
            out[3] = 255;
        }
        _ => {
            out[0] = pixel[1];
            out[1] = pixel[2];
            out[2] = pixel[3];
            out[3] = pixel[0];
        }
    }
}

fn pixmap_data_uri(width: i32, height: i32, rgba: &[u8]) -> Option<String> {
    let mut png = Cursor::new(Vec::new());
    PngEncoder::new(&mut png)
        .write_image(
            rgba,
            width.unsigned_abs(),
            height.unsigned_abs(),
            ColorType::Rgba8.into(),
        )
        .ok()?;
    Some(format!(
        "data:image/png;base64,{}",
        base64_encode(png.get_ref())
    ))
}

const BASE64_TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        let value = ((first as u32) << 16) | ((second as u32) << 8) | third as u32;

        encoded.push(BASE64_TABLE[((value >> 18) & 0x3f) as usize] as char);
        encoded.push(BASE64_TABLE[((value >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            encoded.push(BASE64_TABLE[((value >> 6) & 0x3f) as usize] as char);
        } else {
            encoded.push('=');
        }
        if chunk.len() > 2 {
            encoded.push(BASE64_TABLE[(value & 0x3f) as usize] as char);
        } else {
            encoded.push('=');
        }
    }
    encoded
}

#[derive(Debug)]
pub struct ToplevelIconGlobal {
    _global: GlobalId,
}

impl ToplevelIconGlobal {
    pub fn new(display: &DisplayHandle) -> Self {
        let sizes = vec![16, 24, 32, 48, 64, 128];
        Self {
            _global: display.create_global::<KestrelState, XdgToplevelIconManagerV1, _>(
                1,
                ToplevelIconManagerUserData { sizes },
            ),
        }
    }
}

#[derive(Debug, Clone)]
struct CopiedIconBuffer {
    scale: i32,
    width: i32,
    height: i32,
    stride: i32,
    format: wl_shm::Format,
    pixels: Vec<u8>,
}

#[derive(Debug, Clone, Default)]
struct IconSnapshot {
    icon_name: Option<String>,
    buffers: Vec<CopiedIconBuffer>,
}

#[derive(Debug, Clone, Default)]
pub struct ToplevelIconSurfaceState {
    icon: IconSnapshot,
}

impl Cacheable for ToplevelIconSurfaceState {
    fn commit(&mut self, _dh: &DisplayHandle) -> Self {
        self.clone()
    }

    fn merge_into(self, into: &mut Self, _dh: &DisplayHandle) {
        *into = self;
    }
}

#[derive(Debug)]
pub struct ToplevelIconManagerUserData {
    pub sizes: Vec<i32>,
}

#[derive(Debug, Default)]
pub struct IconUserData {
    builder: Mutex<IconSnapshot>,
    constructed: Mutex<Option<IconSnapshot>>,
}

impl IconUserData {
    fn is_immutable(&self) -> bool {
        self.constructed.lock().unwrap().is_some()
    }

    fn freeze(&self) {
        let mut constructed = self.constructed.lock().unwrap();
        if constructed.is_none() {
            *constructed = Some(self.builder.lock().unwrap().clone());
        }
    }

    fn snapshot(&self) -> IconSnapshot {
        self.constructed
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| self.builder.lock().unwrap().clone())
    }

    fn set_icon_name(&self, icon_name: String) {
        self.builder.lock().unwrap().icon_name = Some(icon_name);
    }

    fn add_buffer(&self, buffer: WlBuffer, scale: i32) -> Result<(), IconBufferError> {
        let copied = copy_shm_icon_buffer(&buffer, scale)?;
        let mut builder = self.builder.lock().unwrap();
        for existing in builder.buffers.iter_mut() {
            if existing.width == copied.width
                && existing.height == copied.height
                && existing.scale == copied.scale
            {
                *existing = copied;
                return Ok(());
            }
        }
        builder.buffers.push(copied);
        Ok(())
    }
}

#[derive(Debug)]
enum IconBufferError {
    NotShm,
    Invalid,
    BadMap,
}

impl GlobalDispatch<XdgToplevelIconManagerV1, ToplevelIconManagerUserData> for KestrelState {
    fn bind(
        _state: &mut Self,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<XdgToplevelIconManagerV1>,
        data: &ToplevelIconManagerUserData,
        data_init: &mut DataInit<'_, Self>,
    ) {
        let manager = data_init.init(resource, ());
        for size in &data.sizes {
            manager.icon_size(*size);
        }
        manager.done();
    }
}

impl Dispatch<XdgToplevelIconManagerV1, ()> for KestrelState {
    fn request(
        state: &mut Self,
        _client: &Client,
        _resource: &XdgToplevelIconManagerV1,
        request: xdg_toplevel_icon_manager_v1::Request,
        _data: &(),
        _handle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            xdg_toplevel_icon_manager_v1::Request::CreateIcon { id } => {
                data_init.init(id, IconUserData::default());
            }
            xdg_toplevel_icon_manager_v1::Request::SetIcon { toplevel, icon } => {
                let Some(toplevel_surface) = state.xdg_shell_state.get_toplevel(&toplevel) else {
                    return;
                };
                let wl_surface = toplevel_surface.wl_surface().clone();
                compositor::with_states(&wl_surface, |states| {
                    let mut cached = states.cached_state.get::<ToplevelIconSurfaceState>();
                    let pending = cached.pending();
                    pending.icon = match icon {
                        Some(icon) => {
                            let data = icon.data::<IconUserData>().unwrap();
                            data.freeze();
                            data.snapshot()
                        }
                        None => IconSnapshot::default(),
                    };
                });
            }
            xdg_toplevel_icon_manager_v1::Request::Destroy => {}
            _ => {}
        }
    }
}

impl Dispatch<XdgToplevelIconV1, IconUserData> for KestrelState {
    fn request(
        _state: &mut Self,
        _client: &Client,
        icon: &XdgToplevelIconV1,
        request: xdg_toplevel_icon_v1::Request,
        data: &IconUserData,
        handle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            xdg_toplevel_icon_v1::Request::SetName { icon_name } => {
                if data.is_immutable() {
                    post_immutable_error(handle, icon);
                    return;
                }
                data.set_icon_name(icon_name);
            }
            xdg_toplevel_icon_v1::Request::AddBuffer { buffer, scale } => {
                if data.is_immutable() {
                    post_immutable_error(handle, icon);
                    return;
                }
                match data.add_buffer(buffer, scale) {
                    Ok(()) => {}
                    Err(IconBufferError::NotShm) => {
                        handle.post_error(
                            icon,
                            xdg_toplevel_icon_v1::Error::InvalidBuffer as u32,
                            "The wl_buffer must be backed by wl_shm".to_string(),
                        );
                    }
                    Err(IconBufferError::Invalid) => {
                        handle.post_error(
                            icon,
                            xdg_toplevel_icon_v1::Error::InvalidBuffer as u32,
                            "The wl_buffer must be a square".to_string(),
                        );
                    }
                    Err(IconBufferError::BadMap) => {}
                }
            }
            xdg_toplevel_icon_v1::Request::Destroy => {}
            _ => {}
        }
    }
}

fn post_immutable_error(handle: &DisplayHandle, icon: &XdgToplevelIconV1) {
    handle.post_error(
        icon,
        xdg_toplevel_icon_v1::Error::Immutable as u32,
        "Request made after the icon has been assigned to a toplevel via 'set_icon'".to_string(),
    );
}

fn copy_shm_icon_buffer(buffer: &WlBuffer, scale: i32) -> Result<CopiedIconBuffer, IconBufferError> {
    shm::with_buffer_contents(buffer, |ptr, len, data| {
        if data.width != data.height || data.width <= 0 || data.height <= 0 {
            return Err(IconBufferError::Invalid);
        }

        let row_bytes = data.stride.max(0) as usize;
        let height = data.height.max(0) as usize;
        let offset = data.offset.max(0) as usize;
        let size = row_bytes
            .checked_mul(height)
            .and_then(|rows| offset.checked_add(rows))
            .filter(|end| *end <= len)
            .ok_or(IconBufferError::Invalid)?;

        let mut pixels = vec![0_u8; size - offset];
        if !pixels.is_empty() {
            for (row, chunk) in pixels.chunks_mut(row_bytes).enumerate() {
                let start = offset + row * row_bytes;
                let end = start + chunk.len();
                if end > len {
                    return Err(IconBufferError::Invalid);
                }
                chunk.copy_from_slice(unsafe {
                    std::slice::from_raw_parts(ptr.add(start), chunk.len())
                });
            }
        }

        Ok(CopiedIconBuffer {
            scale,
            width: data.width,
            height: data.height,
            stride: data.stride,
            format: data.format,
            pixels,
        })
    })
    .map_err(|error| match error {
        BufferAccessError::NotManaged => IconBufferError::NotShm,
        BufferAccessError::BadMap
        | BufferAccessError::NotReadable
        | BufferAccessError::NotWritable => IconBufferError::BadMap,
    })?
}
