use super::{
    MIN_WINDOW_HEIGHT, MIN_WINDOW_WIDTH, ResizeEdge, WindowFrameControl, WindowRestoreState,
    WindowVisualExtents,
};
use crate::{
    state::BatonState,
    window_geometry::{move_geometry, resize_geometry},
};
use smithay::{
    input::keyboard::KeyboardHandle,
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel,
    utils::{Logical, Point},
    wayland::shell::xdg::ToplevelSurface,
};
use staccato_layout::{LayoutError, ModeId, Rect, WindowId, WindowState};

const MAXIMIZED_MARGIN: i32 = 0;
const TOP_PANEL_HEIGHT: i32 = 34;
const BOTTOM_PANEL_HEIGHT: i32 = 48;
const FLOATING_MARGIN: i32 = 8;

#[derive(Debug, Clone)]
pub enum WindowGrab {
    Move {
        surface: ToplevelSurface,
        pointer_start: Point<f64, Logical>,
        start_geometry: Rect,
    },
    Resize {
        id: WindowId,
        edge: ResizeEdge,
        pointer_start: Point<f64, Logical>,
        start_geometry: Rect,
    },
}

impl BatonState {
    pub fn begin_drag(&mut self, surface: ToplevelSurface) {
        let Some((_id, start_geometry)) = self.windows.geometry_for_surface(&surface) else {
            return;
        };

        self.drag = Some(WindowGrab::Move {
            surface,
            pointer_start: self.pointer_location,
            start_geometry,
        });
    }

    pub fn begin_resize(&mut self, surface: ToplevelSurface, edge: ResizeEdge) {
        let Some((id, start_geometry)) = self.windows.geometry_for_surface(&surface) else {
            return;
        };

        self.windows.set_restore_geometry(id, None);
        let _ = self.layout.set_window_state(id, WindowState::Floating);
        self.drag = Some(WindowGrab::Resize {
            id,
            edge,
            pointer_start: self.pointer_location,
            start_geometry,
        });
    }

    pub fn update_drag(&mut self, location: Point<f64, Logical>) {
        let Some(grab) = self.drag.clone() else {
            return;
        };

        match grab {
            WindowGrab::Move {
                surface,
                pointer_start,
                start_geometry,
            } => {
                let Some((id, _)) = self.windows.geometry_for_surface(&surface) else {
                    return;
                };
                self.windows.set_restore_geometry(id, None);
                let geometry = move_geometry(start_geometry, pointer_start, location);
                self.apply_window_geometry(id, geometry, false, false, false);
            }
            WindowGrab::Resize {
                id,
                edge,
                pointer_start,
                start_geometry,
            } => {
                let geometry = resize_geometry(start_geometry, edge, pointer_start, location);
                self.apply_window_geometry(id, geometry, false, false, false);
            }
        }
    }

    pub fn end_drag(&mut self) {
        self.drag = None;
    }

    pub fn handle_window_control(
        &mut self,
        keyboard: &KeyboardHandle<Self>,
        id: WindowId,
        control: WindowFrameControl,
    ) -> Result<(), LayoutError> {
        if control != WindowFrameControl::Close {
            self.activate_window(keyboard, id)?;
        }

        match control {
            WindowFrameControl::Minimize => self.minimize_window(keyboard, id),
            WindowFrameControl::Maximize => self.toggle_maximize_window(id),
            WindowFrameControl::Close => {
                let result = self.close_window(id);
                if result.is_ok() {
                    self.focus_active_workspace(keyboard);
                }
                result
            }
        }
    }

    pub fn show_window(&mut self, id: WindowId) -> Result<(), LayoutError> {
        self.windows
            .set_hidden(id, false, self.animations_enabled());
        let state = if self
            .windows
            .window(id)
            .is_some_and(|window| window.fullscreen)
        {
            WindowState::Fullscreen
        } else if self.windows.restore_geometry(id).is_some() {
            WindowState::Maximized
        } else {
            WindowState::Floating
        };
        self.layout.set_window_state(id, state)?;
        self.apply_active_arrangement();
        Ok(())
    }

    pub fn minimize_window(
        &mut self,
        keyboard: &KeyboardHandle<Self>,
        id: WindowId,
    ) -> Result<(), LayoutError> {
        if !self.windows.set_hidden(id, true, self.animations_enabled()) {
            return Err(LayoutError::UnknownWindow(id));
        }

        self.layout.set_window_state(id, WindowState::Hidden)?;
        self.apply_active_arrangement();
        self.focus_active_workspace(keyboard);
        Ok(())
    }

    pub fn toggle_maximize_window(&mut self, id: WindowId) -> Result<(), LayoutError> {
        if self.windows.restore_geometry(id).is_some() {
            return self.unmaximize_window(id);
        }

        self.maximize_window(id)
    }

    pub fn maximize_window(&mut self, id: WindowId) -> Result<(), LayoutError> {
        if self
            .windows
            .window(id)
            .is_some_and(|window| window.fullscreen)
        {
            self.unfullscreen_window(id)?;
        }
        if self.windows.restore_geometry(id).is_some() {
            return Ok(());
        }

        let geometry = self
            .layout
            .window(id)
            .map(|window| window.geometry)
            .or_else(|| self.windows.window(id).map(|window| window.geometry()))
            .ok_or(LayoutError::UnknownWindow(id))?;
        self.windows.set_restore_geometry(id, Some(geometry));
        self.apply_window_geometry(
            id,
            self.maximized_geometry(id)?,
            true,
            false,
            self.animations_enabled(),
        );
        Ok(())
    }

    pub fn unmaximize_window(&mut self, id: WindowId) -> Result<(), LayoutError> {
        if let Some(restore) = self.windows.restore_geometry(id) {
            self.windows.set_restore_geometry(id, None);
            self.apply_window_geometry(id, restore, false, false, self.animations_enabled());
        }

        Ok(())
    }

    pub fn fullscreen_window(&mut self, id: WindowId) -> Result<(), LayoutError> {
        let managed = self
            .windows
            .window(id)
            .cloned()
            .ok_or(LayoutError::UnknownWindow(id))?;
        if managed.fullscreen {
            return Ok(());
        }

        let restore = self
            .layout
            .window(id)
            .map(|window| WindowRestoreState {
                geometry: window.geometry,
                state: window.state.clone(),
            })
            .unwrap_or(WindowRestoreState {
                geometry: managed.geometry(),
                state: WindowState::Floating,
            });
        self.windows.set_fullscreen_restore(id, Some(restore));
        self.windows.set_fullscreen(id, true);
        self.apply_window_geometry(
            id,
            self.fullscreen_geometry(),
            false,
            true,
            self.animations_enabled(),
        );
        Ok(())
    }

    pub fn unfullscreen_window(&mut self, id: WindowId) -> Result<(), LayoutError> {
        if !self
            .windows
            .window(id)
            .is_some_and(|window| window.fullscreen)
        {
            return Ok(());
        }

        let restore = self.windows.fullscreen_restore(id);
        self.windows.set_fullscreen(id, false);
        self.windows.set_fullscreen_restore(id, None);
        match restore {
            Some(restore) if restore.state == WindowState::Maximized => {
                self.apply_window_geometry(
                    id,
                    self.maximized_geometry(id)?,
                    true,
                    false,
                    self.animations_enabled(),
                );
            }
            Some(restore) => {
                self.apply_window_geometry(
                    id,
                    restore.geometry,
                    false,
                    false,
                    self.animations_enabled(),
                );
            }
            None => {
                self.apply_window_geometry(
                    id,
                    self.fit_initial_window_geometry(self.windows.next_geometry()),
                    false,
                    false,
                    self.animations_enabled(),
                );
            }
        }
        Ok(())
    }

    fn apply_window_geometry(
        &mut self,
        id: WindowId,
        geometry: Rect,
        maximized: bool,
        fullscreen: bool,
        animate: bool,
    ) {
        let titlebar_height = self
            .windows
            .window(id)
            .map(|window| window.titlebar_height())
            .unwrap_or_default();
        let extents = self
            .windows
            .window(id)
            .map(|window| window.visual_extents())
            .unwrap_or_default();
        let from = self.windows.window(id).map(|window| window.geometry());
        let geometry = if fullscreen {
            self.fit_fullscreen_window_geometry(geometry)
        } else if maximized {
            self.fit_maximized_window_geometry(geometry, titlebar_height)
        } else {
            self.fit_window_geometry(geometry, titlebar_height, extents)
        };
        let Some((surface, geometry)) = self.windows.set_geometry(id, geometry) else {
            return;
        };
        self.windows.set_maximized(id, maximized);
        if let Some(from) = from {
            self.windows.animate_geometry(id, from, animate);
        }

        let _ = self.layout.set_window_geometry(id, geometry);
        let state = if fullscreen {
            WindowState::Fullscreen
        } else if maximized {
            WindowState::Maximized
        } else {
            WindowState::Floating
        };
        let _ = self.layout.set_window_state(id, state);
        configure_surface(&surface, geometry, maximized, fullscreen);
        self.mark_scene_dirty();
    }

    pub fn apply_active_arrangement(&mut self) {
        let Ok(arrangement) = self.layout.arrange_active() else {
            return;
        };

        for (id, geometry) in arrangement.windows {
            self.apply_arranged_window_geometry(id, geometry);
        }
    }

    fn apply_arranged_window_geometry(&mut self, id: WindowId, geometry: Rect) {
        let maximized = self
            .layout
            .window(id)
            .is_some_and(|window| window.state == WindowState::Maximized);
        let fullscreen = self
            .windows
            .window(id)
            .is_some_and(|window| window.fullscreen);
        let titlebar_height = self
            .windows
            .window(id)
            .map(|window| window.titlebar_height())
            .unwrap_or_default();
        let extents = self
            .windows
            .window(id)
            .map(|window| window.visual_extents())
            .unwrap_or_default();
        let geometry = if fullscreen {
            self.fit_fullscreen_window_geometry(geometry)
        } else if maximized {
            self.fit_maximized_window_geometry(geometry, titlebar_height)
        } else {
            self.fit_window_geometry(geometry, titlebar_height, extents)
        };
        let Some((surface, geometry)) = self.windows.set_geometry(id, geometry) else {
            return;
        };
        self.windows.set_maximized(id, maximized);

        let _ = self.layout.set_window_geometry(id, geometry);
        configure_surface(&surface, geometry, maximized, fullscreen);
        self.mark_scene_dirty();
    }

    fn fullscreen_geometry(&self) -> Rect {
        Rect::new(0, 0, self.output_size.w, self.output_size.h)
    }

    fn maximized_geometry(&self, id: WindowId) -> Result<Rect, LayoutError> {
        let titlebar_height = self
            .windows
            .window(id)
            .map(|window| window.titlebar_height())
            .ok_or(LayoutError::UnknownWindow(id))?;
        let top = self.reserved_top();
        let bottom = self.reserved_bottom();
        let width = (self.output_size.w - MAXIMIZED_MARGIN * 2).max(MIN_WINDOW_WIDTH);
        let height = (self.output_size.h - top - bottom - titlebar_height).max(MIN_WINDOW_HEIGHT);
        Ok(Rect::new(MAXIMIZED_MARGIN, top, width, height))
    }

    pub fn fit_initial_window_geometry(&self, geometry: Rect) -> Rect {
        self.fit_window_geometry(geometry, 0, WindowVisualExtents::default())
    }

    fn fit_window_geometry(
        &self,
        geometry: Rect,
        titlebar_height: i32,
        extents: WindowVisualExtents,
    ) -> Rect {
        let min_x = FLOATING_MARGIN;
        let top = self.reserved_top();
        let bottom = self.reserved_bottom();
        let min_y = if top == 0 { FLOATING_MARGIN } else { top };
        let max_right = (self.output_size.w - FLOATING_MARGIN).max(min_x + MIN_WINDOW_WIDTH);
        let max_bottom =
            (self.output_size.h - bottom).max(min_y + MIN_WINDOW_HEIGHT + titlebar_height);
        let max_width = (max_right - min_x - extents.left - extents.right).max(MIN_WINDOW_WIDTH);
        let max_height = (max_bottom - min_y - titlebar_height - extents.top - extents.bottom)
            .max(MIN_WINDOW_HEIGHT);
        let width = geometry.width.clamp(MIN_WINDOW_WIDTH, max_width);
        let height = geometry.height.clamp(MIN_WINDOW_HEIGHT, max_height);
        let min_x = min_x + extents.left;
        let min_y = min_y + extents.top;
        let max_x = (max_right - width - extents.right).max(min_x);
        let max_y = (max_bottom - titlebar_height - height - extents.bottom).max(min_y);

        Rect::new(
            geometry.x.clamp(min_x, max_x),
            geometry.y.clamp(min_y, max_y),
            width,
            height,
        )
    }

    fn fit_maximized_window_geometry(&self, geometry: Rect, titlebar_height: i32) -> Rect {
        let min_y = self.reserved_top();
        let bottom = self.reserved_bottom();
        let max_width = self.output_size.w.max(MIN_WINDOW_WIDTH);
        let max_height =
            (self.output_size.h - min_y - bottom - titlebar_height).max(MIN_WINDOW_HEIGHT);
        Rect::new(
            0,
            min_y,
            geometry.width.clamp(MIN_WINDOW_WIDTH, max_width),
            geometry.height.clamp(MIN_WINDOW_HEIGHT, max_height),
        )
    }

    fn fit_fullscreen_window_geometry(&self, _geometry: Rect) -> Rect {
        Rect::new(
            0,
            0,
            self.output_size.w.max(MIN_WINDOW_WIDTH),
            self.output_size.h.max(MIN_WINDOW_HEIGHT),
        )
    }

    fn reserved_top(&self) -> i32 {
        match self.layout.active_mode() {
            ModeId::Panel => 0,
            _ => TOP_PANEL_HEIGHT,
        }
    }

    fn reserved_bottom(&self) -> i32 {
        match self.layout.active_mode() {
            ModeId::Panel => BOTTOM_PANEL_HEIGHT,
            _ => 0,
        }
    }
}

fn configure_surface(surface: &ToplevelSurface, geometry: Rect, maximized: bool, fullscreen: bool) {
    surface.with_pending_state(|state| {
        state.size = Some((geometry.width, geometry.height).into());
        if maximized {
            state.states.set(xdg_toplevel::State::Maximized);
        } else {
            state.states.unset(xdg_toplevel::State::Maximized);
        }
        if fullscreen {
            state.states.set(xdg_toplevel::State::Fullscreen);
        } else {
            state.states.unset(xdg_toplevel::State::Fullscreen);
        }
    });
    surface.send_pending_configure();
}
