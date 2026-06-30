use image::ImageReader;
use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            element::{
                Kind,
                memory::{MemoryRenderBuffer, MemoryRenderBufferRenderElement},
            },
            gles::{GlesError, GlesRenderer},
        },
    },
    utils::{Buffer, Physical, Rectangle, Size, Transform},
};
use std::path::{Path, PathBuf};
use tracing::warn;

#[derive(Debug, Default)]
pub struct Background {
    path: Option<PathBuf>,
    image: Option<BackgroundImage>,
    buffer: Option<BackgroundBuffer>,
}

impl Background {
    pub fn new(path: Option<PathBuf>) -> Self {
        let mut background = Self::default();
        background.set_path(path);
        background
    }

    pub fn set_path(&mut self, path: Option<PathBuf>) -> bool {
        if self.path == path {
            return false;
        }

        self.path = path;
        self.buffer = None;
        self.image = self
            .path
            .as_deref()
            .and_then(|path| load_background(path).map_err(log_background_error).ok());
        true
    }

    pub fn render_element(
        &mut self,
        renderer: &mut GlesRenderer,
        size: Size<i32, Physical>,
    ) -> Result<Option<MemoryRenderBufferRenderElement<GlesRenderer>>, GlesError> {
        let Some(image) = &self.image else {
            return Ok(None);
        };
        let size = normalized_size(size);

        if self
            .buffer
            .as_ref()
            .is_none_or(|buffer| buffer.size != size)
        {
            self.buffer = Some(BackgroundBuffer::new(image, size));
        }

        let Some(buffer) = &self.buffer else {
            return Ok(None);
        };
        MemoryRenderBufferRenderElement::from_buffer(
            renderer,
            (0.0, 0.0),
            &buffer.buffer,
            None,
            None,
            Some(size.to_logical(1)),
            Kind::Unspecified,
        )
        .map(Some)
    }
}

#[derive(Debug)]
struct BackgroundBuffer {
    size: Size<i32, Physical>,
    buffer: MemoryRenderBuffer,
}

impl BackgroundBuffer {
    fn new(image: &BackgroundImage, size: Size<i32, Physical>) -> Self {
        let pixels = image.cover(size);
        let buffer = MemoryRenderBuffer::from_slice(
            &pixels,
            Fourcc::Abgr8888,
            Size::<i32, Buffer>::from((size.w, size.h)),
            1,
            Transform::Normal,
            Some(vec![Rectangle::from_size(Size::from((size.w, size.h)))]),
        );
        Self { size, buffer }
    }
}

#[derive(Debug)]
struct BackgroundImage {
    width: i32,
    height: i32,
    pixels: Vec<u8>,
}

impl BackgroundImage {
    fn cover(&self, size: Size<i32, Physical>) -> Vec<u8> {
        let width = size.w.max(1);
        let height = size.h.max(1);
        let scale = (width as f32 / self.width as f32).max(height as f32 / self.height as f32);
        let visible_width = width as f32 / scale;
        let visible_height = height as f32 / scale;
        let offset_x = (self.width as f32 - visible_width) * 0.5;
        let offset_y = (self.height as f32 - visible_height) * 0.5;
        let mut output = vec![0; (width * height * 4) as usize];

        for y in 0..height {
            for x in 0..width {
                let source_x = offset_x + (x as f32 + 0.5) / scale;
                let source_y = offset_y + (y as f32 + 0.5) / scale;
                let pixel = self.sample(source_x, source_y);
                let index = ((y * width + x) * 4) as usize;
                output[index..index + 4].copy_from_slice(&pixel);
            }
        }

        output
    }

    fn sample(&self, x: f32, y: f32) -> [u8; 4] {
        let x0 = x.floor().clamp(0.0, (self.width - 1) as f32) as i32;
        let y0 = y.floor().clamp(0.0, (self.height - 1) as f32) as i32;
        let x1 = (x0 + 1).min(self.width - 1);
        let y1 = (y0 + 1).min(self.height - 1);
        let tx = x - x.floor();
        let ty = y - y.floor();

        let top = mix_pixel(self.pixel(x0, y0), self.pixel(x1, y0), tx);
        let bottom = mix_pixel(self.pixel(x0, y1), self.pixel(x1, y1), tx);
        mix_pixel(top, bottom, ty)
    }

    fn pixel(&self, x: i32, y: i32) -> [u8; 4] {
        let index = ((y * self.width + x) * 4) as usize;
        [
            self.pixels[index],
            self.pixels[index + 1],
            self.pixels[index + 2],
            self.pixels[index + 3],
        ]
    }
}

fn load_background(path: &Path) -> Result<BackgroundImage, image::ImageError> {
    let image = ImageReader::open(path)?.decode()?.to_rgba8();
    let (width, height) = image.dimensions();
    Ok(BackgroundImage {
        width: width as i32,
        height: height as i32,
        pixels: image.into_raw(),
    })
}

fn log_background_error(error: image::ImageError) {
    warn!(%error, "failed to load background image");
}

fn mix_pixel(left: [u8; 4], right: [u8; 4], amount: f32) -> [u8; 4] {
    [
        mix_channel(left[0], right[0], amount),
        mix_channel(left[1], right[1], amount),
        mix_channel(left[2], right[2], amount),
        mix_channel(left[3], right[3], amount),
    ]
}

fn mix_channel(left: u8, right: u8, amount: f32) -> u8 {
    (left as f32 + (right as f32 - left as f32) * amount).round() as u8
}

fn normalized_size(size: Size<i32, Physical>) -> Size<i32, Physical> {
    (size.w.max(1), size.h.max(1)).into()
}
