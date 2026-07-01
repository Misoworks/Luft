use crate::{
    client::ClientState,
    render::handle_commit,
    state::KestrelState,
    window::{ResizeEdge, surface_has_client_frame_extents},
};
use smithay::{
    backend::allocator::Buffer,
    delegate_alpha_modifier, delegate_compositor, delegate_cursor_shape, delegate_data_device,
    delegate_dmabuf, delegate_fractional_scale, delegate_layer_shell, delegate_output,
    delegate_presentation, delegate_primary_selection, delegate_seat, delegate_shm,
    delegate_text_input_manager, delegate_viewporter, delegate_xdg_activation,
    delegate_xdg_decoration, delegate_xdg_shell, delegate_xdg_toplevel_icon,
    desktop::{PopupKeyboardGrab, PopupKind, PopupPointerGrab},
    input::{
        Seat, SeatHandler,
        keyboard::LedState,
        pointer::{CursorIcon, CursorImageStatus, Focus},
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
        dmabuf::{DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier},
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

impl BufferHandler for KestrelState {
    fn buffer_destroyed(&mut self, _buffer: &wl_buffer::WlBuffer) {}
}

impl DmabufHandler for KestrelState {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.protocol_state.dmabuf
    }

    fn dmabuf_imported(
        &mut self,
        _global: &DmabufGlobal,
        dmabuf: smithay::backend::allocator::dmabuf::Dmabuf,
        notifier: ImportNotifier,
    ) {
        if self.dmabuf_formats.contains(&dmabuf.format()) {
            let _ = notifier.successful::<Self>();
        } else {
            notifier.failed();
        }
    }
}

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

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        self.enter_output(surface.wl_surface());
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
        _positioner: PositionerState,
        token: u32,
    ) {
        surface.send_repositioned(token);
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

impl XdgActivationHandler for KestrelState {
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

impl FractionalScaleHandler for KestrelState {
    fn new_fractional_scale(&mut self, surface: WlSurface) {
        self.update_surface_scale(&surface);
    }
}

impl XdgToplevelIconHandler for KestrelState {}

impl WlrLayerShellHandler for KestrelState {
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

    fn new_popup(&mut self, _parent: LayerSurface, popup: PopupSurface) {
        self.enter_output(popup.wl_surface());
        let _ = self
            .popup_manager
            .track_popup(PopupKind::from(popup.clone()));
        let _ = popup.send_configure();
        self.mark_scene_dirty();
    }

    fn layer_destroyed(&mut self, surface: LayerSurface) {
        self.unmap_layer_surface(&surface);
        self.mark_scene_dirty();
        debug!("unmapped layer surface");
    }
}

impl OutputHandler for KestrelState {
    fn output_bound(&mut self, output: Output, _wl_output: WlOutput) {
        debug!(name = %output.name(), "client bound output");
    }
}

impl SelectionHandler for KestrelState {
    type SelectionUserData = ();
}

impl DataDeviceHandler for KestrelState {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}

impl PrimarySelectionHandler for KestrelState {
    fn primary_selection_state(&self) -> &PrimarySelectionState {
        &self.primary_selection_state
    }
}

impl ClientDndGrabHandler for KestrelState {}

impl ServerDndGrabHandler for KestrelState {
    fn send(&mut self, _mime_type: String, _fd: OwnedFd, _seat: Seat<Self>) {}
}

impl CompositorHandler for KestrelState {
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
        let popup_needs_render = self.popup_manager.find_popup(surface).is_some();
        let needs_render = self.commit_surface_needs_render(surface) || popup_needs_render;
        handle_commit(surface);
        self.popup_manager.commit(surface);
        let popup_mapped = !popup_needs_render && self.popup_manager.find_popup(surface).is_some();
        let initial_size_adopted = self.adopt_initial_toplevel_size(surface);
        let decoration_changed = self.reconcile_decoration_after_commit(surface);
        if needs_render || popup_mapped {
            self.mark_scene_dirty();
        }
        if initial_size_adopted {
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

impl ShmHandler for KestrelState {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

impl SeatHandler for KestrelState {
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

    fn led_state_changed(&mut self, _seat: &Seat<Self>, led_state: LedState) {
        self.set_pending_keyboard_led_state(led_state);
    }
}

impl TabletSeatHandler for KestrelState {}

delegate_xdg_shell!(KestrelState);
delegate_xdg_decoration!(KestrelState);
delegate_xdg_activation!(KestrelState);
delegate_xdg_toplevel_icon!(KestrelState);
delegate_cursor_shape!(KestrelState);
delegate_fractional_scale!(KestrelState);
delegate_viewporter!(KestrelState);
delegate_text_input_manager!(KestrelState);
delegate_presentation!(KestrelState);
delegate_layer_shell!(KestrelState);
delegate_compositor!(KestrelState);
delegate_dmabuf!(KestrelState);
delegate_output!(KestrelState);
delegate_shm!(KestrelState);
delegate_seat!(KestrelState);
delegate_data_device!(KestrelState);
delegate_primary_selection!(KestrelState);
delegate_alpha_modifier!(KestrelState);
