use smithay::{
    reexports::wayland_server::backend::{ClientData, ClientId, DisconnectReason},
    wayland::compositor::CompositorClientState,
};
use tracing::debug;

#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
}

impl ClientData for ClientState {
    fn initialized(&self, client_id: ClientId) {
        debug!(?client_id, "wayland client initialized");
    }

    fn disconnected(&self, client_id: ClientId, reason: DisconnectReason) {
        debug!(?client_id, ?reason, "wayland client disconnected");
    }
}
