use crate::{
    state::KestrelState,
    window::{ResizeEdge, surface_has_client_frame_extents},
};
use smithay::{
    desktop::{PopupKeyboardGrab, PopupKind, PopupManager, PopupPointerGrab, layer_map_for_output},
    input::pointer::Focus,
    reexports::{
        wayland_protocols::xdg::{
            decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode, shell::server::xdg_toplevel,
        },
        wayland_server::protocol::{wl_output::WlOutput, wl_seat, wl_surface::WlSurface},
    },
    utils::{Logical, Point, Rectangle, Serial, Size, Transform},
    wayland::{
        compositor,
        fractional_scale::{self, FractionalScaleHandler},
        shell::xdg::{
            PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
            decoration::XdgDecorationHandler,
        },
    },
};
use tracing::debug;

impl XdgShellHandler for KestrelState {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        surface.with_pending_state(|state| {
            state.states.set(xdg_toplevel::State::Activated);
        });
        self.map_toplevel(surface.clone());
        if let Some(keyboard) = self.keyboard.clone() {
            self.activate_surface(&keyboard, &surface);
        }
        surface.send_configure();
        debug!("mapped xdg toplevel");
    }

    fn new_popup(&mut self, surface: PopupSurface, positioner: PositionerState) {
        self.enter_output(surface.wl_surface());
        let target = self
            .popup_constraint_for(&surface)
            .or_else(|| Some(Rectangle::from_size(self.output_logical_size())));
        configure_popup(&surface, positioner, target);
        let _ = self
            .popup_manager
            .track_popup(PopupKind::from(surface.clone()));
        let _ = surface.send_configure();
        self.mark_scene_dirty();
    }

    fn move_request(&mut self, surface: ToplevelSurface, _seat: wl_seat::WlSeat, serial: Serial) {
        if !self.client_grab_allowed(&surface, serial) {
            debug!("ignored stale xdg toplevel move request");
            return;
        }
        if let Some(keyboard) = self.keyboard.clone() {
            self.activate_surface(&keyboard, &surface);
        }
        self.begin_client_drag(surface);
    }

    fn resize_request(
        &mut self,
        surface: ToplevelSurface,
        _seat: wl_seat::WlSeat,
        serial: Serial,
        edges: xdg_toplevel::ResizeEdge,
    ) {
        let Some(edge) = resize_edge_from_xdg(edges) else {
            return;
        };
        if !self.client_grab_allowed(&surface, serial) {
            debug!("ignored stale xdg toplevel resize request");
            return;
        }

        if let Some(keyboard) = self.keyboard.clone() {
            self.activate_surface(&keyboard, &surface);
        }
        self.begin_client_resize(surface, edge);
    }

    fn grab(&mut self, surface: PopupSurface, _seat: wl_seat::WlSeat, serial: Serial) {
        let popup = PopupKind::from(surface);
        let Ok(root) = smithay::desktop::find_popup_root_surface(&popup) else {
            return;
        };
        let seat = self.seat.clone();
        let Ok(grab) = self
            .popup_manager
            .grab_popup::<Self>(root, popup, &seat, serial)
        else {
            return;
        };

        if let Some(keyboard) = seat.get_keyboard() {
            keyboard.set_grab(self, PopupKeyboardGrab::new(&grab), serial);
        }
        if let Some(pointer) = seat.get_pointer() {
            pointer.set_grab(self, PopupPointerGrab::new(&grab), serial, Focus::Keep);
        }
    }

    fn reposition_request(
        &mut self,
        surface: PopupSurface,
        positioner: PositionerState,
        token: u32,
    ) {
        let target = self
            .popup_constraint_for(&surface)
            .or_else(|| Some(Rectangle::from_size(self.output_logical_size())));
        configure_popup(&surface, positioner, target);
        let _ = surface.send_repositioned(token);
        self.mark_scene_dirty();
    }

    fn popup_destroyed(&mut self, surface: PopupSurface) {
        self.leave_output(surface.wl_surface());
        self.mark_scene_dirty();
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        self.unmap_toplevel(&surface);
        debug!("unmapped xdg toplevel");
    }

    fn maximize_request(&mut self, surface: ToplevelSurface) {
        let Some(id) = self.windows.id_for_surface(&surface) else {
            surface.send_configure();
            return;
        };

        let _ = self.maximize_window(id);
    }

    fn unmaximize_request(&mut self, surface: ToplevelSurface) {
        let Some(id) = self.windows.id_for_surface(&surface) else {
            surface.send_configure();
            return;
        };

        let _ = self.unmaximize_window(id);
    }

    fn fullscreen_request(&mut self, surface: ToplevelSurface, _output: Option<WlOutput>) {
        let Some(id) = self.windows.id_for_surface(&surface) else {
            surface.send_configure();
            return;
        };

        let _ = self.fullscreen_window(id);
    }

    fn unfullscreen_request(&mut self, surface: ToplevelSurface) {
        let Some(id) = self.windows.id_for_surface(&surface) else {
            surface.send_configure();
            return;
        };

        let _ = self.unfullscreen_window(id);
    }

    fn minimize_request(&mut self, surface: ToplevelSurface) {
        let Some(keyboard) = self.keyboard.clone() else {
            return;
        };
        let Some(id) = self.windows.id_for_surface(&surface) else {
            return;
        };

        let _ = self.minimize_window(&keyboard, id);
    }

    fn parent_changed(&mut self, surface: ToplevelSurface) {
        self.sync_toplevel_parent(&surface);
    }
}

impl XdgDecorationHandler for KestrelState {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        self.set_decoration_mode(&toplevel, Mode::ClientSide);
    }

    fn request_mode(&mut self, toplevel: ToplevelSurface, mode: Mode) {
        self.set_decoration_mode(&toplevel, mode);
    }

    fn unset_mode(&mut self, toplevel: ToplevelSurface) {
        self.set_decoration_mode(&toplevel, Mode::ClientSide);
    }
}

impl FractionalScaleHandler for KestrelState {
    fn new_fractional_scale(&mut self, surface: WlSurface) {
        self.update_surface_scale(&surface);
    }
}

impl KestrelState {
    pub(crate) fn update_surface_scale(&self, surface: &WlSurface) {
        let scale = self.output().current_scale();
        let integer_scale = scale.integer_scale().max(1);
        let fractional_scale = scale.fractional_scale();

        compositor::with_states(surface, |states| {
            compositor::send_surface_state(surface, states, integer_scale, Transform::Normal);
            fractional_scale::with_fractional_scale(states, |state| {
                state.set_preferred_scale(fractional_scale);
            });
        });
    }

    pub(super) fn reconcile_decoration_after_commit(&mut self, surface: &WlSurface) -> bool {
        let Some((toplevel, change)) = self.windows.refresh_decoration_for_root_surface(surface)
        else {
            return false;
        };

        let mode = if change.server_decorated {
            Mode::ServerSide
        } else {
            Mode::ClientSide
        };
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(mode);
        });
        let _ = self.layout.set_window_geometry(change.id, change.geometry);
        self.apply_active_arrangement();
        true
    }

    pub(super) fn popup_constraint_for(
        &self,
        popup: &PopupSurface,
    ) -> Option<Rectangle<i32, Logical>> {
        let parent = popup.get_parent_surface()?;
        let origin = self.surface_origin(&parent)?;
        Some(output_constraint(self.output_logical_size(), origin))
    }

    fn set_decoration_mode(&mut self, toplevel: &ToplevelSurface, mode: Mode) {
        let requested_server_decoration = mode == Mode::ServerSide;
        let advertised_mode =
            if requested_server_decoration && surface_has_client_frame_extents(toplevel) {
                Mode::ClientSide
            } else {
                mode
            };
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(advertised_mode);
        });

        if let Some(change) = self
            .windows
            .set_requested_server_decoration(toplevel, requested_server_decoration)
        {
            let _ = self.layout.set_window_geometry(change.id, change.geometry);
            self.apply_active_arrangement();
        } else {
            toplevel.send_configure();
            self.mark_scene_dirty();
        }
    }

    fn surface_origin(&self, surface: &WlSurface) -> Option<Point<i32, Logical>> {
        if let Some(window) = self
            .windows
            .iter()
            .find(|window| window.surface.wl_surface() == surface)
        {
            return Some(window.content_location());
        }

        for window in self.windows.iter() {
            if let Some(origin) = popup_surface_origin(
                window.surface.wl_surface(),
                window.content_location(),
                surface,
            ) {
                return Some(origin);
            }
        }

        let layer_map = layer_map_for_output(self.output());
        for layer in layer_map.layers() {
            let Some(geometry) = layer_map.layer_geometry(layer) else {
                continue;
            };
            if layer.wl_surface() == surface {
                return Some(geometry.loc);
            }
            if let Some(origin) = popup_surface_origin(layer.wl_surface(), geometry.loc, surface) {
                return Some(origin);
            }
        }

        None
    }
}

pub(super) fn configure_existing_popup(
    surface: &PopupSurface,
    target: Option<Rectangle<i32, Logical>>,
) {
    surface.with_pending_state(|state| {
        state.geometry = target
            .map(|target| state.positioner.get_unconstrained_geometry(target))
            .unwrap_or_else(|| state.positioner.get_geometry());
    });
}

fn resize_edge_from_xdg(edge: xdg_toplevel::ResizeEdge) -> Option<ResizeEdge> {
    use xdg_toplevel::ResizeEdge as XdgResizeEdge;

    match edge {
        XdgResizeEdge::None => None,
        XdgResizeEdge::Top => Some(ResizeEdge {
            left: false,
            right: false,
            top: true,
            bottom: false,
        }),
        XdgResizeEdge::Bottom => Some(ResizeEdge {
            left: false,
            right: false,
            top: false,
            bottom: true,
        }),
        XdgResizeEdge::Left => Some(ResizeEdge {
            left: true,
            right: false,
            top: false,
            bottom: false,
        }),
        XdgResizeEdge::TopLeft => Some(ResizeEdge {
            left: true,
            right: false,
            top: true,
            bottom: false,
        }),
        XdgResizeEdge::BottomLeft => Some(ResizeEdge {
            left: true,
            right: false,
            top: false,
            bottom: true,
        }),
        XdgResizeEdge::Right => Some(ResizeEdge {
            left: false,
            right: true,
            top: false,
            bottom: false,
        }),
        XdgResizeEdge::TopRight => Some(ResizeEdge {
            left: false,
            right: true,
            top: true,
            bottom: false,
        }),
        XdgResizeEdge::BottomRight => Some(ResizeEdge {
            left: false,
            right: true,
            top: false,
            bottom: true,
        }),
        _ => None,
    }
}

fn configure_popup(
    surface: &PopupSurface,
    positioner: PositionerState,
    target: Option<Rectangle<i32, Logical>>,
) {
    surface.with_pending_state(|state| {
        state.positioner = positioner;
        state.geometry = target
            .map(|target| positioner.get_unconstrained_geometry(target))
            .unwrap_or_else(|| positioner.get_geometry());
    });
}

fn popup_surface_origin(
    root: &WlSurface,
    root_origin: Point<i32, Logical>,
    surface: &WlSurface,
) -> Option<Point<i32, Logical>> {
    for (popup, offset) in PopupManager::popups_for_surface(root) {
        let popup_origin = root_origin + offset;
        if popup.wl_surface() == surface {
            return Some(popup_origin);
        }
        if let Some(origin) =
            popup_surface_origin(popup.wl_surface(), popup_origin, surface)
        {
            return Some(origin);
        }
    }
    None
}

fn output_constraint(
    output: Size<i32, Logical>,
    parent_origin: Point<i32, Logical>,
) -> Rectangle<i32, Logical> {
    Rectangle::new((-parent_origin.x, -parent_origin.y).into(), output)
}
