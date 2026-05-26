use crate::{
    client::ClientState,
    render::handle_commit,
    state::BatonState,
    window::{ResizeEdge, surface_has_client_frame_extents},
};
use smithay::{
    delegate_compositor, delegate_cursor_shape, delegate_data_device, delegate_fractional_scale,
    delegate_layer_shell, delegate_output, delegate_presentation, delegate_primary_selection,
    delegate_seat, delegate_shm, delegate_text_input_manager, delegate_viewporter,
    delegate_xdg_activation, delegate_xdg_decoration, delegate_xdg_shell,
    delegate_xdg_toplevel_icon,
    input::{
        Seat, SeatHandler,
        pointer::{CursorIcon, CursorImageStatus},
    },
    output::Output,
    reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::{
            Client, Resource,
            protocol::{wl_buffer, wl_output::WlOutput, wl_seat, wl_surface::WlSurface},
        },
    },
    utils::{Serial, Transform},
    wayland::{
        buffer::BufferHandler,
        compositor::{self, CompositorClientState, CompositorHandler, CompositorState},
        fractional_scale::{self, FractionalScaleHandler},
        output::OutputHandler,
        selection::{
            SelectionHandler,
            data_device::{
                ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler,
                set_data_device_focus,
            },
            primary_selection::{
                PrimarySelectionHandler, PrimarySelectionState, set_primary_focus,
            },
        },
        shell::wlr_layer::{Layer, LayerSurface, WlrLayerShellHandler, WlrLayerShellState},
        shell::xdg::{
            PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
            decoration::XdgDecorationHandler,
        },
        shm::{ShmHandler, ShmState},
        tablet_manager::TabletSeatHandler,
        xdg_activation::{
            XdgActivationHandler, XdgActivationState, XdgActivationToken,
            XdgActivationTokenData,
        },
        xdg_toplevel_icon::XdgToplevelIconHandler,
    },
};
use smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode;
use std::os::unix::io::OwnedFd;
use tracing::debug;

impl BufferHandler for BatonState {
    fn buffer_destroyed(&mut self, _buffer: &wl_buffer::WlBuffer) {}
}

impl XdgShellHandler for BatonState {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        surface.with_pending_state(|state| {
            state.size = Some((900, 560).into());
            state.states.set(xdg_toplevel::State::Activated);
        });
        self.map_toplevel(surface.clone());
        if let Some(keyboard) = self.keyboard.clone() {
            self.activate_surface(&keyboard, &surface);
        }
        surface.send_configure();
        debug!("mapped xdg toplevel");
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        let _ = surface.send_configure();
    }

    fn move_request(&mut self, surface: ToplevelSurface, _seat: wl_seat::WlSeat, _serial: Serial) {
        if let Some(keyboard) = self.keyboard.clone() {
            self.activate_surface(&keyboard, &surface);
        }
        self.begin_drag(surface);
    }

    fn resize_request(
        &mut self,
        surface: ToplevelSurface,
        _seat: wl_seat::WlSeat,
        _serial: Serial,
        edges: xdg_toplevel::ResizeEdge,
    ) {
        let Some(edge) = resize_edge_from_xdg(edges) else {
            return;
        };

        if let Some(keyboard) = self.keyboard.clone() {
            self.activate_surface(&keyboard, &surface);
        }
        self.begin_resize(surface, edge);
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: wl_seat::WlSeat, _serial: Serial) {}

    fn reposition_request(
        &mut self,
        surface: PopupSurface,
        _positioner: PositionerState,
        token: u32,
    ) {
        let _ = surface.send_repositioned(token);
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
}

impl XdgDecorationHandler for BatonState {
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

impl XdgActivationHandler for BatonState {
    fn activation_state(&mut self) -> &mut XdgActivationState {
        &mut self.protocol_state.xdg_activation
    }

    fn request_activation(
        &mut self,
        token: XdgActivationToken,
        _token_data: XdgActivationTokenData,
        surface: WlSurface,
    ) {
        self.protocol_state.xdg_activation.remove_token(&token);
        let Some(id) = self.windows.id_for_wl_surface(&surface) else {
            debug!(token = %token.as_str(), "ignored activation for unmanaged surface");
            return;
        };
        let Some(keyboard) = self.keyboard.clone() else {
            debug!(token = %token.as_str(), ?id, "ignored activation without keyboard seat");
            return;
        };

        if let Err(error) = self.activate_window(&keyboard, id) {
            debug!(token = %token.as_str(), ?id, ?error, "failed to activate requested window");
        }
    }
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

impl BatonState {
    pub(crate) fn update_surface_scale(&self, surface: &WlSurface) {
        let scale = self.output.current_scale();
        let integer_scale = scale.integer_scale().max(1);
        let fractional_scale = scale.fractional_scale();

        compositor::with_states(surface, |states| {
            compositor::send_surface_state(surface, states, integer_scale, Transform::Normal);
            fractional_scale::with_fractional_scale(states, |state| {
                state.set_preferred_scale(fractional_scale);
            });
        });
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

    fn reconcile_decoration_after_commit(&mut self, surface: &WlSurface) -> bool {
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
}

impl FractionalScaleHandler for BatonState {
    fn new_fractional_scale(&mut self, surface: WlSurface) {
        self.update_surface_scale(&surface);
    }
}

impl XdgToplevelIconHandler for BatonState {}

impl WlrLayerShellHandler for BatonState {
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.layer_shell_state
    }

    fn new_layer_surface(
        &mut self,
        surface: LayerSurface,
        _output: Option<WlOutput>,
        layer: Layer,
        namespace: String,
    ) {
        self.map_layer_surface(surface, namespace.clone());
        self.mark_scene_dirty();
        debug!(?layer, namespace, "mapped layer surface");
    }

    fn layer_destroyed(&mut self, surface: LayerSurface) {
        self.unmap_layer_surface(&surface);
        self.mark_scene_dirty();
        debug!("unmapped layer surface");
    }
}

impl OutputHandler for BatonState {
    fn output_bound(&mut self, output: Output, _wl_output: WlOutput) {
        debug!(name = %output.name(), "client bound output");
    }
}

impl SelectionHandler for BatonState {
    type SelectionUserData = ();
}

impl DataDeviceHandler for BatonState {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}

impl PrimarySelectionHandler for BatonState {
    fn primary_selection_state(&self) -> &PrimarySelectionState {
        &self.primary_selection_state
    }
}

impl ClientDndGrabHandler for BatonState {}

impl ServerDndGrabHandler for BatonState {
    fn send(&mut self, _mime_type: String, _fd: OwnedFd, _seat: Seat<Self>) {}
}

impl CompositorHandler for BatonState {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client.get_data::<ClientState>().unwrap().compositor_state
    }

    fn new_surface(&mut self, surface: &WlSurface) {
        self.update_surface_scale(surface);
        self.mark_scene_dirty();
    }

    fn commit(&mut self, surface: &WlSurface) {
        let needs_render = self.commit_surface_needs_render(surface);
        handle_commit(surface);
        let decoration_changed = self.reconcile_decoration_after_commit(surface);
        if needs_render {
            self.mark_scene_dirty();
        }
        if decoration_changed {
            self.mark_scene_dirty();
        }
        if let Some(layer_surface) = self.layer_surface_for_commit(surface) {
            self.arrange_layers();
            self.mark_scene_dirty();
            layer_surface.send_pending_configure();
        }
    }
}

impl ShmHandler for BatonState {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

impl SeatHandler for BatonState {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut smithay::input::SeatState<Self> {
        &mut self.seat_state
    }

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&WlSurface>) {
        let client = focused.and_then(Resource::client);
        set_data_device_focus(&self.display_handle, seat, client.clone());
        set_primary_focus(&self.display_handle, seat, client);
    }

    fn cursor_image(&mut self, _seat: &Seat<Self>, image: CursorImageStatus) {
        self.frame_cursor_active = false;
        self.cursor_image = match image {
            CursorImageStatus::Surface(_) => CursorImageStatus::Named(CursorIcon::Default),
            image => image,
        };
        self.cursor_dirty = true;
    }
}

impl TabletSeatHandler for BatonState {}

delegate_xdg_shell!(BatonState);
delegate_xdg_decoration!(BatonState);
delegate_xdg_activation!(BatonState);
delegate_xdg_toplevel_icon!(BatonState);
delegate_cursor_shape!(BatonState);
delegate_fractional_scale!(BatonState);
delegate_viewporter!(BatonState);
delegate_text_input_manager!(BatonState);
delegate_presentation!(BatonState);
delegate_layer_shell!(BatonState);
delegate_compositor!(BatonState);
delegate_output!(BatonState);
delegate_shm!(BatonState);
delegate_seat!(BatonState);
delegate_data_device!(BatonState);
delegate_primary_selection!(BatonState);
