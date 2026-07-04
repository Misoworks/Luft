use crate::{state::KestrelState, window::ManagedWindow};
use smithay::{
    backend::renderer::{
        Renderer,
        element::{
            Element, Id, Kind, RenderElement, UnderlyingStorage,
            surface::{WaylandSurfaceRenderElement, render_elements_from_surface_tree},
        },
        gles::GlesRenderer,
        utils::{CommitCounter, DamageSet, OpaqueRegions},
    },
    desktop::PopupManager,
    utils::{Buffer, Physical, Point, Rectangle, Scale, Size},
};

pub const WINDOW_RADIUS: i32 = 12;

pub struct RoundedWindowElement<E> {
    element: E,
    clip: Rectangle<i32, Physical>,
    shape: ClipShape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipShape {
    Rect,
    RoundRect { radius: i32 },
    RoundTop { radius: i32 },
    RoundLeft { radius: i32 },
    RoundRight { radius: i32 },
}

pub fn window_elements(
    renderer: &mut GlesRenderer,
    state: &KestrelState,
) -> Vec<RoundedWindowElement<WaylandSurfaceRenderElement<GlesRenderer>>> {
    let mut elements = Vec::new();
    if let Some(transition) = state.workspace_transition() {
        let width = state.output_size().w as f64;
        let direction = transition.direction as f64;
        let from_offset = (-direction * width * transition.progress).round() as i32;
        let to_offset = (direction * width * (1.0 - transition.progress)).round() as i32;
        append_workspace_elements(
            renderer,
            state,
            &transition.from,
            from_offset,
            &mut elements,
        );
        append_workspace_elements(renderer, state, &transition.to, to_offset, &mut elements);
    } else {
        append_workspace_elements(
            renderer,
            state,
            state.layout.active_workspace(),
            0,
            &mut elements,
        );
    }
    elements
}

pub fn window_elements_for_window(
    renderer: &mut GlesRenderer,
    window: &ManagedWindow,
    offset_x: i32,
    output_size: Size<i32, Physical>,
) -> Vec<RoundedWindowElement<WaylandSurfaceRenderElement<GlesRenderer>>> {
    let mut elements = Vec::new();
    append_window_elements(renderer, window, offset_x, output_size, &mut elements);
    elements
}

fn append_workspace_elements(
    renderer: &mut GlesRenderer,
    state: &KestrelState,
    workspace: &luft_ipc::WorkspaceId,
    offset_x: i32,
    elements: &mut Vec<RoundedWindowElement<WaylandSurfaceRenderElement<GlesRenderer>>>,
) {
    for window in state.windows.render_windows_on_workspace(workspace) {
        append_window_elements(renderer, window, offset_x, state.output_size(), elements);
    }
}

fn append_window_elements(
    renderer: &mut GlesRenderer,
    window: &ManagedWindow,
    offset_x: i32,
    output_size: Size<i32, Physical>,
    elements: &mut Vec<RoundedWindowElement<WaylandSurfaceRenderElement<GlesRenderer>>>,
) {
    let transform = window.render_transform(offset_x, output_size);
    let titlebar_height = window.titlebar_height();
    let surface_offset = window.surface_offset();
    let location = Point::<i32, Physical>::from((
        (transform.x + surface_offset.x as f64 * transform.scale).round() as i32,
        (transform.y + (titlebar_height + surface_offset.y) as f64 * transform.scale).round()
            as i32,
    ));
    let frame_clip = Rectangle::<i32, Physical>::new(
        Point::from((transform.x.round() as i32, transform.y.round() as i32)),
        Size::from((
            (window.size.w as f64 * transform.scale).round().max(1.0) as i32,
            ((window.size.h + titlebar_height) as f64 * transform.scale)
                .round()
                .max(1.0) as i32,
        )),
    );
    elements.extend(
        render_elements_from_surface_tree(
            renderer,
            window.surface.wl_surface(),
            (location.x, location.y),
            transform.scale,
            transform.alpha,
            Kind::Unspecified,
        )
        .into_iter()
        .map(|element: WaylandSurfaceRenderElement<GlesRenderer>| {
            let (clip, radius) = if window.server_decorated {
                (frame_clip, window_radius(window, transform.scale))
            } else {
                (element.geometry(Scale::from(1.0)), 0)
            };
            RoundedWindowElement::new(element, clip, radius)
        }),
    );
    append_popup_elements(renderer, window, transform, surface_offset, elements);
}

fn append_popup_elements(
    renderer: &mut GlesRenderer,
    window: &ManagedWindow,
    transform: crate::window_animation::WindowTransform,
    surface_offset: Point<i32, smithay::utils::Logical>,
    elements: &mut Vec<RoundedWindowElement<WaylandSurfaceRenderElement<GlesRenderer>>>,
) {
    for (popup, popup_offset) in PopupManager::popups_for_surface(window.surface.wl_surface()) {
        let popup_geometry = popup.geometry();
        let popup_surface_offset = surface_offset + popup_offset - popup_geometry.loc;
        let popup_location = Point::<i32, Physical>::from((
            (transform.x + popup_surface_offset.x as f64 * transform.scale).round() as i32,
            (transform.y
                + (window.titlebar_height() + popup_surface_offset.y) as f64 * transform.scale)
                .round() as i32,
        ));
        elements.extend(
            render_elements_from_surface_tree(
                renderer,
                popup.wl_surface(),
                (popup_location.x, popup_location.y),
                transform.scale,
                transform.alpha,
                Kind::Unspecified,
            )
            .into_iter()
            .map(|element: WaylandSurfaceRenderElement<GlesRenderer>| {
                let clip = element.geometry(Scale::from(1.0));
                RoundedWindowElement::new_with_shape(element, clip, ClipShape::Rect)
            }),
        );
    }
}

impl<E> RoundedWindowElement<E> {
    pub fn new(element: E, clip: Rectangle<i32, Physical>, radius: i32) -> Self {
        let shape = if radius > 0 {
            ClipShape::RoundRect { radius }
        } else {
            ClipShape::Rect
        };
        Self::new_with_shape(element, clip, shape)
    }

    pub fn new_with_shape(element: E, clip: Rectangle<i32, Physical>, shape: ClipShape) -> Self {
        Self {
            element,
            clip,
            shape: shape.clamped(clip.size),
        }
    }
}

impl ClipShape {
    fn clamped(self, size: Size<i32, Physical>) -> Self {
        match self {
            Self::Rect => Self::Rect,
            Self::RoundRect { radius } => Self::RoundRect {
                radius: clamp_radius(radius, size),
            },
            Self::RoundTop { radius } => Self::RoundTop {
                radius: clamp_radius(radius, size),
            },
            Self::RoundLeft { radius } => Self::RoundLeft {
                radius: clamp_radius(radius, size),
            },
            Self::RoundRight { radius } => Self::RoundRight {
                radius: clamp_radius(radius, size),
            },
        }
    }

    fn is_rounded(self) -> bool {
        !matches!(self, Self::Rect)
    }
}

fn window_radius(window: &ManagedWindow, scale: f64) -> i32 {
    if window.flat_frame_corners() {
        0
    } else {
        (WINDOW_RADIUS as f64 * scale).round().max(1.0) as i32
    }
}

impl<E: Element> Element for RoundedWindowElement<E> {
    fn id(&self) -> &Id {
        self.element.id()
    }

    fn current_commit(&self) -> CommitCounter {
        self.element.current_commit()
    }

    fn src(&self) -> Rectangle<f64, Buffer> {
        self.element.src()
    }

    fn geometry(&self, scale: Scale<f64>) -> Rectangle<i32, Physical> {
        self.element.geometry(scale)
    }

    fn transform(&self) -> smithay::utils::Transform {
        self.element.transform()
    }

    fn damage_since(
        &self,
        scale: Scale<f64>,
        commit: Option<CommitCounter>,
    ) -> DamageSet<i32, Physical> {
        self.element.damage_since(scale, commit)
    }

    fn opaque_regions(&self, scale: Scale<f64>) -> OpaqueRegions<i32, Physical> {
        let element_geometry = self.element.geometry(scale);
        self.element
            .opaque_regions(scale)
            .into_iter()
            .flat_map(|mut region| {
                region.loc += element_geometry.loc;
                clip_strips(self.clip, self.shape)
                    .into_iter()
                    .filter_map(move |strip| {
                        let mut clipped = region.intersection(strip)?;
                        clipped.loc -= element_geometry.loc;
                        Some(clipped)
                    })
            })
            .collect()
    }

    fn alpha(&self) -> f32 {
        self.element.alpha()
    }

    fn kind(&self) -> Kind {
        self.element.kind()
    }
}

impl<R, E> RenderElement<R> for RoundedWindowElement<E>
where
    R: Renderer,
    E: RenderElement<R>,
{
    fn draw(
        &self,
        frame: &mut R::Frame<'_, '_>,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        opaque_regions: &[Rectangle<i32, Physical>],
    ) -> Result<(), R::Error> {
        let element_geometry = self.element.geometry(Scale::from(1.0));
        for strip in clip_strips(self.clip, self.shape) {
            let Some(piece) = strip.intersection(dst) else {
                continue;
            };
            let piece_damage = damage_for_piece(damage, dst, piece);
            if piece_damage.is_empty() {
                continue;
            }

            let piece_src =
                source_for_piece(src, element_geometry, piece, self.element.transform());
            self.element
                .draw(frame, piece_src, piece, &piece_damage, opaque_regions)?;
        }
        Ok(())
    }

    fn underlying_storage(&self, renderer: &mut R) -> Option<UnderlyingStorage<'_>> {
        if self.shape.is_rounded() {
            return None;
        }

        self.element.underlying_storage(renderer)
    }
}

fn clip_strips(rect: Rectangle<i32, Physical>, shape: ClipShape) -> Vec<Rectangle<i32, Physical>> {
    if shape == ClipShape::Rect {
        return vec![rect];
    }

    let mut strips = Vec::new();
    let mut y = 0;
    while y < rect.size.h {
        let inset = clip_inset(y, rect.size.h, shape);
        let mut next_y = y + 1;
        while next_y < rect.size.h && clip_inset(next_y, rect.size.h, shape) == inset {
            next_y += 1;
        }
        let (x, width) = match shape {
            ClipShape::Rect => (rect.loc.x, rect.size.w),
            ClipShape::RoundRect { .. } | ClipShape::RoundTop { .. } => {
                (rect.loc.x + inset, rect.size.w - inset * 2)
            }
            ClipShape::RoundLeft { .. } => (rect.loc.x + inset, rect.size.w - inset),
            ClipShape::RoundRight { .. } => (rect.loc.x, rect.size.w - inset),
        };
        if width > 0 {
            strips.push(Rectangle::new(
                (x, rect.loc.y + y).into(),
                (width, next_y - y).into(),
            ));
        }
        y = next_y;
    }
    strips
}

fn clip_inset(y: i32, height: i32, shape: ClipShape) -> i32 {
    let radius = match shape {
        ClipShape::Rect => 0,
        ClipShape::RoundRect { radius }
        | ClipShape::RoundTop { radius }
        | ClipShape::RoundLeft { radius }
        | ClipShape::RoundRight { radius } => radius,
    };
    if y >= radius && (matches!(shape, ClipShape::RoundTop { .. }) || y < height - radius) {
        return 0;
    }

    let center_y = if y < radius {
        radius as f64
    } else {
        (height - radius) as f64
    };
    let dy = (y as f64 + 0.5 - center_y).abs();
    let dx = ((radius * radius) as f64 - dy * dy).max(0.0).sqrt();
    (radius as f64 - dx).ceil() as i32
}

fn clamp_radius(radius: i32, size: Size<i32, Physical>) -> i32 {
    radius.max(0).min(size.w / 2).min(size.h / 2)
}

fn damage_for_piece(
    damage: &[Rectangle<i32, Physical>],
    dst: Rectangle<i32, Physical>,
    piece: Rectangle<i32, Physical>,
) -> Vec<Rectangle<i32, Physical>> {
    let piece_relative = Rectangle::new(
        (piece.loc.x - dst.loc.x, piece.loc.y - dst.loc.y).into(),
        piece.size,
    );

    damage
        .iter()
        .filter_map(|damage| {
            damage.intersection(piece_relative).map(|mut damage| {
                damage.loc -= piece_relative.loc;
                damage
            })
        })
        .collect()
}

fn source_for_piece(
    src: Rectangle<f64, Buffer>,
    element_geometry: Rectangle<i32, Physical>,
    piece: Rectangle<i32, Physical>,
    transform: smithay::utils::Transform,
) -> Rectangle<f64, Buffer> {
    let mut relative = piece;
    relative.loc -= element_geometry.loc;
    let physical_to_buffer_scale = src.size
        / transform
            .invert()
            .transform_size(element_geometry.size)
            .to_f64();
    let mut piece_src = relative.to_f64().to_logical(1.0).to_buffer(
        physical_to_buffer_scale,
        transform,
        &element_geometry.size.to_f64().to_logical(1.0),
    );
    piece_src.loc += src.loc;
    piece_src
}
