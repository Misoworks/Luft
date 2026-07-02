use crate::{client::ClientState, render::handle_commit, state::KestrelState};
use smithay::{
    backend::allocator::Buffer,
    delegate_alpha_modifier, delegate_compositor, delegate_cursor_shape, delegate_data_device,
    delegate_dmabuf, delegate_fractional_scale, delegate_idle_inhibit,
    delegate_keyboard_shortcuts_inhibit, delegate_layer_shell, delegate_output,
    delegate_pointer_constraints, delegate_pointer_gestures, delegate_presentation,
    delegate_primary_selection, delegate_relative_pointer, delegate_seat, delegate_shm,
    delegate_text_input_manager, delegate_viewporter, delegate_xdg_activation,
    delegate_xdg_decoration, delegate_xdg_foreign, delegate_xdg_shell, delegate_xdg_toplevel_icon,
    desktop::PopupKind,
    input::{
        Seat, SeatHandler,
        keyboard::LedState,
        pointer::{CursorIcon, CursorImageStatus},
    },
    output::Output,
    reexports::wayland_server::{
        Client, Resource,
        protocol::{wl_buffer, wl_output::WlOutput, wl_surface::WlSurface},
    },
    wayland::{
        buffer::BufferHandler,
        compositor::{CompositorClientState, CompositorHandler, CompositorState},
        dmabuf::{DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier},
        idle_inhibit::IdleInhibitHandler,
        keyboard_shortcuts_inhibit::{
            KeyboardShortcutsInhibitHandler, KeyboardShortcutsInhibitState,
            KeyboardShortcutsInhibitor,
        },
        output::OutputHandler,
        pointer_constraints::{PointerConstraintsHandler, with_pointer_constraint},
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
        shell::xdg::PopupSurface,
        shm::{ShmHandler, ShmState},
        tablet_manager::TabletSeatHandler,
        xdg_activation::{
            XdgActivationHandler, XdgActivationState, XdgActivationToken, XdgActivationTokenData,
        },
        xdg_foreign::{XdgForeignHandler, XdgForeignState},
    },
};
#[cfg(feature = "session-backend")]
use smithay::{
    backend::renderer::sync::Fence,
    delegate_drm_syncobj,
    wayland::{
        compositor,
        compositor::{BufferAssignment, SurfaceAttributes},
        drm_syncobj::{DrmSyncobjCachedState, DrmSyncobjHandler, DrmSyncobjState},
    },
};
use std::os::unix::io::OwnedFd;
use tracing::debug;

mod xdg;
use self::xdg::configure_existing_popup;

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

#[cfg(feature = "session-backend")]
impl DrmSyncobjHandler for KestrelState {
    fn drm_syncobj_state(&mut self) -> Option<&mut DrmSyncobjState> {
        self.protocol_state.drm_syncobj.as_mut()
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

impl KestrelState {}

impl XdgForeignHandler for KestrelState {
    fn xdg_foreign_state(&mut self) -> &mut XdgForeignState {
        &mut self.protocol_state.xdg_foreign
    }
}

impl IdleInhibitHandler for KestrelState {
    fn inhibit(&mut self, _surface: WlSurface) {}

    fn uninhibit(&mut self, _surface: WlSurface) {}
}

impl KeyboardShortcutsInhibitHandler for KestrelState {
    fn keyboard_shortcuts_inhibit_state(&mut self) -> &mut KeyboardShortcutsInhibitState {
        &mut self.protocol_state.keyboard_shortcuts_inhibit
    }

    fn new_inhibitor(&mut self, inhibitor: KeyboardShortcutsInhibitor) {
        inhibitor.activate();
    }
}

impl PointerConstraintsHandler for KestrelState {
    fn new_constraint(
        &mut self,
        surface: &WlSurface,
        pointer: &smithay::input::pointer::PointerHandle<Self>,
    ) {
        with_pointer_constraint(surface, pointer, |constraint| {
            if let Some(constraint) = constraint {
                constraint.activate();
            }
        });
    }

    fn cursor_position_hint(
        &mut self,
        _surface: &WlSurface,
        _pointer: &smithay::input::pointer::PointerHandle<Self>,
        _location: smithay::utils::Point<f64, smithay::utils::Logical>,
    ) {
    }
}

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
        configure_existing_popup(&popup, self.popup_constraint_for(&popup));
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
        #[cfg(feature = "session-backend")]
        install_syncobj_pre_commit_hook(surface);
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
delegate_xdg_foreign!(KestrelState);
delegate_idle_inhibit!(KestrelState);
delegate_keyboard_shortcuts_inhibit!(KestrelState);
delegate_pointer_constraints!(KestrelState);
delegate_relative_pointer!(KestrelState);
delegate_pointer_gestures!(KestrelState);
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
#[cfg(feature = "session-backend")]
delegate_drm_syncobj!(KestrelState);

#[cfg(feature = "session-backend")]
fn install_syncobj_pre_commit_hook(surface: &WlSurface) {
    compositor::add_pre_commit_hook::<KestrelState, _>(surface, |state, _dh, surface| {
        queue_syncobj_acquire(state, surface);
    });
}

#[cfg(feature = "session-backend")]
fn queue_syncobj_acquire(state: &mut KestrelState, surface: &WlSurface) {
    let Some(client) = surface.client() else {
        return;
    };

    let acquire = compositor::with_states(surface, |states| {
        let mut attributes = states.cached_state.get::<SurfaceAttributes>();
        if !matches!(
            attributes.pending().buffer,
            Some(BufferAssignment::NewBuffer(_))
        ) {
            return None;
        }

        let mut syncobj = states.cached_state.get::<DrmSyncobjCachedState>();
        let pending = syncobj.pending();
        pending
            .release_point
            .as_ref()
            .and_then(|_| pending.acquire_point.clone())
    });

    let Some(acquire) = acquire else {
        return;
    };
    if acquire.is_signaled() {
        return;
    }

    match acquire.generate_blocker() {
        Ok((blocker, source)) => {
            compositor::add_blocker(surface, blocker);
            state.queue_syncobj_source(client, source);
        }
        Err(error) => {
            tracing::warn!(%error, "failed to create drm syncobj acquire blocker");
        }
    }
}
