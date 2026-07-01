use crate::{
    layers::{BlurLayer, LayerMaterial, LayerRenderTarget},
    window_clip::ClipShape,
};
use asher_config::BlurQuality;
use smithay::utils::{Buffer, Logical, Physical, Point, Rectangle, Size};

const WINDOW_BLUR_SAMPLE_PADDING: i32 = 0;
const LAYER_BLUR_SAMPLE_PADDING: i32 = 36;

pub(super) fn material_radius(
    material: LayerMaterial,
    texture_size: Size<i32, Physical>,
    visible_size: Size<i32, Physical>,
) -> f32 {
    match material {
        LayerMaterial::Rect => 0.0,
        LayerMaterial::RoundRect { radius }
        | LayerMaterial::RoundTop { radius }
        | LayerMaterial::RoundLeft { radius }
        | LayerMaterial::RoundRight { radius } => {
            let scale_x = visible_size.w.max(1) as f32 / texture_size.w.max(1) as f32;
            let scale_y = visible_size.h.max(1) as f32 / texture_size.h.max(1) as f32;
            let scale = scale_x.min(scale_y).max(1.0);
            radius as f32 / scale
        }
    }
}

pub(super) fn material_clip_shape(
    material: LayerMaterial,
    visible_size: Size<i32, Physical>,
) -> ClipShape {
    let clamp = |radius: i32| {
        radius
            .max(0)
            .min(visible_size.w / 2)
            .min(visible_size.h / 2)
    };
    match material {
        LayerMaterial::Rect => ClipShape::Rect,
        LayerMaterial::RoundRect { radius } => ClipShape::RoundRect {
            radius: clamp(radius),
        },
        LayerMaterial::RoundTop { radius } => ClipShape::RoundTop {
            radius: clamp(radius),
        },
        LayerMaterial::RoundLeft { radius } => ClipShape::RoundLeft {
            radius: clamp(radius),
        },
        LayerMaterial::RoundRight { radius } => ClipShape::RoundRight {
            radius: clamp(radius),
        },
    }
}

pub(super) fn clipped_target_rect(
    output_size: Size<i32, Physical>,
    target: &LayerRenderTarget,
) -> Option<Rectangle<i32, Physical>> {
    clipped_rect(output_size, target.location, target.size)
}

pub(super) fn padded_target_rect(
    output_size: Size<i32, Physical>,
    target: &LayerRenderTarget,
    rect: Rectangle<i32, Physical>,
) -> Rectangle<i32, Physical> {
    let padding = blur_sample_padding(target);
    if padding <= 0 {
        return rect;
    }

    let output = Rectangle::<i32, Physical>::from_size(output_size);
    let left = (rect.loc.x - padding).max(output.loc.x);
    let top = (rect.loc.y - padding).max(output.loc.y);
    let right = (rect.loc.x + rect.size.w + padding).min(output.loc.x + output.size.w);
    let bottom = (rect.loc.y + rect.size.h + padding).min(output.loc.y + output.size.h);
    Rectangle::<i32, Physical>::new((left, top).into(), (right - left, bottom - top).into())
}

fn blur_sample_padding(target: &LayerRenderTarget) -> i32 {
    if target.blur_layer != BlurLayer::Window && target.material == LayerMaterial::Rect {
        return 0;
    }

    match target.blur_layer {
        BlurLayer::Window => WINDOW_BLUR_SAMPLE_PADDING,
        BlurLayer::Top | BlurLayer::Overlay => LAYER_BLUR_SAMPLE_PADDING,
    }
}

pub(super) fn source_rect_for_visible_target(
    sample_rect: Rectangle<i32, Physical>,
    visible_rect: Rectangle<i32, Physical>,
    capture_size: Size<i32, Physical>,
) -> Rectangle<f64, Buffer> {
    let scale_x = capture_size.w as f64 / sample_rect.size.w.max(1) as f64;
    let scale_y = capture_size.h as f64 / sample_rect.size.h.max(1) as f64;
    let max_x = capture_size.w.max(1) as f64;
    let max_y = capture_size.h.max(1) as f64;
    let left = ((visible_rect.loc.x - sample_rect.loc.x) as f64 * scale_x).clamp(0.0, max_x);
    let top = ((visible_rect.loc.y - sample_rect.loc.y) as f64 * scale_y).clamp(0.0, max_y);
    let right = (left + visible_rect.size.w as f64 * scale_x).clamp(left, max_x);
    let bottom = (top + visible_rect.size.h as f64 * scale_y).clamp(top, max_y);
    let left = (left + 0.5).min(right);
    let top = (top + 0.5).min(bottom);
    let right = (right - 0.5).max(left);
    let bottom = (bottom - 0.5).max(top);
    Rectangle::<f64, Buffer>::new(
        (left, top).into(),
        ((right - left).max(1.0), (bottom - top).max(1.0)).into(),
    )
}

fn clipped_rect(
    output_size: Size<i32, Physical>,
    location: Point<i32, Logical>,
    size: Size<i32, Logical>,
) -> Option<Rectangle<i32, Physical>> {
    if size.w <= 1 || size.h <= 1 {
        return None;
    }

    let output = Rectangle::<i32, Physical>::from_size(output_size);
    Rectangle::<i32, Physical>::new((location.x, location.y).into(), (size.w, size.h).into())
        .intersection(output)
}

pub(super) fn blur_texture_size(
    target: &LayerRenderTarget,
    size: Size<i32, Physical>,
    quality: BlurQuality,
) -> Size<i32, Physical> {
    let scale = blur_downscale(target, size.w, size.h, quality);
    Size::<i32, Physical>::from((
        div_ceil(size.w, scale).max(1),
        div_ceil(size.h, scale).max(1),
    ))
}

fn blur_downscale(
    target: &LayerRenderTarget,
    width: i32,
    height: i32,
    quality: BlurQuality,
) -> i32 {
    let base: i32 = if target.blur_layer != BlurLayer::Window {
        2
    } else {
        let area = width.saturating_mul(height);
        if area >= 420_000 {
            12
        } else if area >= 120_000 {
            10
        } else {
            7
        }
    };

    match quality {
        BlurQuality::Quality => base.saturating_sub(2).max(2),
        BlurQuality::Balanced => base,
        BlurQuality::Performance => base.saturating_add(3),
    }
}

fn div_ceil(value: i32, divisor: i32) -> i32 {
    (value + divisor - 1) / divisor
}

pub(super) fn target_is_damaged(
    rect: Rectangle<i32, Physical>,
    damage: &[Rectangle<i32, Physical>],
) -> bool {
    damage
        .iter()
        .any(|damage| damage.intersection(rect).is_some())
}
