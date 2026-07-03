use crate::window_animation::{WindowAnimation, WindowTransform};
mod geometry;
mod hit;
mod ops;
mod stack;
mod stack_hit;
pub use hit::{ResizeEdge, WindowFrameControl, WindowFrameHit};
use luft_ipc::{Rect, WindowId, WindowState, WorkspaceId};
pub use ops::WindowGrab;
use smithay::{
    desktop::utils::bbox_from_surface_tree,
    utils::{Logical, Physical, Point, Rectangle, Size},
    wayland::{
        compositor,
        shell::xdg::{SurfaceCachedState, ToplevelSurface},
    },
};
pub use stack::WindowStack;

pub const TITLEBAR_HEIGHT: i32 = 32;
pub const TITLEBAR_CONTROL_SIZE: i32 = 12;
pub const TITLEBAR_CONTROL_GAP: i32 = 8;
pub const TITLEBAR_CONTROL_RIGHT: i32 = 14;
pub const TITLEBAR_CONTROL_HIT_PADDING: i32 = 10;
pub const RESIZE_BORDER: i32 = 5;
pub const MIN_WINDOW_WIDTH: i32 = 320;
pub const MIN_WINDOW_HEIGHT: i32 = 200;

#[derive(Debug, Clone)]
pub struct ManagedWindow {
    pub id: WindowId,
    pub workspace: WorkspaceId,
    pub surface: ToplevelSurface,
    pub location: Point<i32, Logical>,
    pub size: Size<i32, Logical>,
    pub requested_server_decoration: bool,
    pub server_decorated: bool,
    pub initial_size_pending: bool,
    pub hidden: bool,
    pub closing: bool,
    close_sent: bool,
    pub maximized: bool,
    pub fullscreen: bool,
    pub restore_geometry: Option<Rect>,
    fullscreen_restore: Option<WindowRestoreState>,
    pub animation: WindowAnimation,
}

#[derive(Debug, Clone)]
pub struct WindowRestoreState {
    pub geometry: Rect,
    pub state: WindowState,
}

impl ManagedWindow {
    pub fn geometry(&self) -> Rect {
        Rect::new(self.location.x, self.location.y, self.size.w, self.size.h)
    }

    pub fn full_geometry(&self, offset_x: i32) -> Rect {
        Rect::new(
            self.location.x + offset_x,
            self.location.y,
            self.size.w,
            self.size.h + self.titlebar_height(),
        )
    }

    pub fn render_transform(&self, offset_x: i32, output: Size<i32, Physical>) -> WindowTransform {
        self.animation
            .transform(self.full_geometry(offset_x), output)
    }

    pub fn content_location(&self) -> Point<i32, Logical> {
        (self.location.x, self.location.y + self.titlebar_height()).into()
    }

    pub fn surface_location(&self) -> Point<i32, Logical> {
        let content = self.content_location();
        let geometry = self.surface_geometry();
        (content.x - geometry.loc.x, content.y - geometry.loc.y).into()
    }

    pub fn surface_offset(&self) -> Point<i32, Logical> {
        let geometry = self.surface_geometry();
        (-geometry.loc.x, -geometry.loc.y).into()
    }

    pub fn surface_geometry(&self) -> Rectangle<i32, Logical> {
        let fallback = Rectangle::from_size(self.size);
        let geometry = self.committed_surface_geometry().unwrap_or(fallback);

        if geometry.size.w > 0 && geometry.size.h > 0 {
            geometry
        } else {
            fallback
        }
    }

    pub fn committed_surface_geometry(&self) -> Option<Rectangle<i32, Logical>> {
        let geometry = compositor::with_states(self.surface.wl_surface(), |states| {
            states
                .cached_state
                .get::<SurfaceCachedState>()
                .current()
                .geometry
        })?;

        if geometry.size.w > 0 && geometry.size.h > 0 {
            return Some(geometry);
        }

        let bounds = bbox_from_surface_tree(self.surface.wl_surface(), (0, 0));
        (!bounds.is_empty()).then_some(bounds)
    }

    pub fn titlebar_height(&self) -> i32 {
        if self.server_decorated && !self.fullscreen {
            TITLEBAR_HEIGHT
        } else {
            0
        }
    }

    pub fn flat_frame_corners(&self) -> bool {
        self.maximized || self.fullscreen
    }

    pub fn client_frame_extents(&self) -> bool {
        surface_has_client_frame_extents(&self.surface)
    }
}

pub fn surface_has_client_frame_extents(surface: &ToplevelSurface) -> bool {
    let geometry = compositor::with_states(surface.wl_surface(), |states| {
        states
            .cached_state
            .get::<SurfaceCachedState>()
            .current()
            .geometry
    });
    let Some(geometry) = geometry else {
        return false;
    };

    if geometry.loc.x != 0 || geometry.loc.y != 0 {
        return true;
    }

    let surface_bounds = bbox_from_surface_tree(surface.wl_surface(), (0, 0));
    !surface_bounds.is_empty() && geometry != surface_bounds
}
