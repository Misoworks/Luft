use crate::state::KestrelState;
use protocol::server::{
    vicinae_hotkey_manager_v1::{self, Modifiers as HotkeyModifiers, VicinaeHotkeyManagerV1},
    vicinae_hotkey_v1::{self, DenyReason, VicinaeHotkeyV1},
};
use smithay::{
    backend::input::KeyState,
    input::keyboard::{Keysym, ModifiersState},
    reexports::wayland_server::{
        Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New,
        backend::{ClientId, GlobalId},
        protocol::wl_seat::WlSeat,
    },
    utils::Serial,
};
use tracing::debug;
use wayland_backend::protocol::WEnum;

pub mod protocol {
    #![allow(
        dead_code,
        non_camel_case_types,
        non_snake_case,
        non_upper_case_globals
    )]
    #![allow(missing_docs, unused_imports, unused_unsafe, unused_variables)]
    #![allow(clippy::all)]

    pub mod server {
        use wayland_server;
        use wayland_server::protocol::*;

        pub mod __interfaces {
            use wayland_server::protocol::__interfaces::*;
            wayland_scanner::generate_interfaces!("protocols/vicinae-hotkey-v1.xml");
        }

        use self::__interfaces::*;
        wayland_scanner::generate_server_code!("protocols/vicinae-hotkey-v1.xml");
    }
}

#[derive(Debug)]
pub struct VicinaeHotkeyState {
    _global: GlobalId,
    bindings: Vec<HotkeyBinding>,
}

impl VicinaeHotkeyState {
    pub fn new(display: &DisplayHandle) -> Self {
        Self {
            _global: display.create_global::<KestrelState, VicinaeHotkeyManagerV1, _>(1, ()),
            bindings: Vec::new(),
        }
    }

    fn bind(
        &mut self,
        hotkey: VicinaeHotkeyV1,
        keysym: u32,
        modifiers: HotkeyModifiers,
    ) -> BindReply {
        match validate_binding(keysym, modifiers) {
            Ok(()) => {}
            Err(reply) => return reply,
        }

        if self
            .bindings
            .iter()
            .any(|binding| binding.keysym == keysym && binding.modifiers == modifiers)
        {
            return BindReply::denied(DenyReason::AlreadyBound, "hotkey is already bound");
        }

        self.bindings.push(HotkeyBinding {
            hotkey,
            keysym,
            modifiers,
            pressed: false,
        });
        BindReply::Bound
    }

    fn remove(&mut self, hotkey: &VicinaeHotkeyV1) {
        self.bindings.retain(|binding| binding.hotkey != *hotkey);
    }

    pub fn handle_key(
        &mut self,
        key: Option<Keysym>,
        modifiers: &ModifiersState,
        key_state: KeyState,
        serial: Serial,
        time: u32,
    ) -> bool {
        let Some(keysym) = key.map(|key| key.raw()) else {
            return false;
        };
        let serial = u32::from(serial);
        let active_modifiers = active_hotkey_modifiers(modifiers);

        match key_state {
            KeyState::Pressed => self.press_matches(keysym, active_modifiers, serial, time),
            KeyState::Released => self.release_matches(keysym, serial, time),
        }
    }

    fn press_matches(
        &mut self,
        keysym: u32,
        active_modifiers: HotkeyModifiers,
        serial: u32,
        time: u32,
    ) -> bool {
        let mut consumed = false;
        for binding in self
            .bindings
            .iter_mut()
            .filter(|binding| binding.keysym == keysym && binding.modifiers == active_modifiers)
        {
            consumed = true;
            if !binding.pressed {
                binding.pressed = true;
                binding.hotkey.pressed(serial, time);
            }
        }
        consumed
    }

    fn release_matches(&mut self, keysym: u32, serial: u32, time: u32) -> bool {
        let mut consumed = false;
        for binding in self
            .bindings
            .iter_mut()
            .filter(|binding| binding.keysym == keysym && binding.pressed)
        {
            consumed = true;
            binding.pressed = false;
            binding.hotkey.released(serial, time);
        }
        consumed
    }
}

#[derive(Debug)]
struct HotkeyBinding {
    hotkey: VicinaeHotkeyV1,
    keysym: u32,
    modifiers: HotkeyModifiers,
    pressed: bool,
}

#[derive(Debug)]
enum BindReply {
    Bound,
    Denied {
        reason: DenyReason,
        message: &'static str,
    },
}

impl BindReply {
    fn denied(reason: DenyReason, message: &'static str) -> Self {
        Self::Denied { reason, message }
    }

    fn send(self, hotkey: &VicinaeHotkeyV1) {
        match self {
            Self::Bound => hotkey.bound(),
            Self::Denied { reason, message } => hotkey.denied(reason, message.to_string()),
        }
    }
}

impl KestrelState {
    pub(crate) fn handle_vicinae_hotkey(
        &mut self,
        key: Option<Keysym>,
        modifiers: &ModifiersState,
        key_state: KeyState,
        serial: Serial,
        time: u32,
    ) -> bool {
        self.protocol_state
            .vicinae_hotkey
            .handle_key(key, modifiers, key_state, serial, time)
    }
}

impl GlobalDispatch<VicinaeHotkeyManagerV1, ()> for KestrelState {
    fn bind(
        _state: &mut Self,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<VicinaeHotkeyManagerV1>,
        _global_data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }
}

impl Dispatch<VicinaeHotkeyManagerV1, ()> for KestrelState {
    fn request(
        state: &mut Self,
        _client: &Client,
        _manager: &VicinaeHotkeyManagerV1,
        request: vicinae_hotkey_manager_v1::Request,
        _data: &(),
        _handle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            vicinae_hotkey_manager_v1::Request::Bind {
                id,
                keysym,
                modifiers,
                seat,
                app_id,
                description,
            } => {
                let hotkey = data_init.init(id, ());
                let reply = state.bind_vicinae_hotkey(hotkey.clone(), keysym, modifiers, seat);
                debug!(%app_id, %description, keysym, "handled vicinae hotkey bind request");
                reply.send(&hotkey);
            }
            vicinae_hotkey_manager_v1::Request::Destroy => {}
        }
    }
}

impl Dispatch<VicinaeHotkeyV1, ()> for KestrelState {
    fn request(
        state: &mut Self,
        _client: &Client,
        hotkey: &VicinaeHotkeyV1,
        request: vicinae_hotkey_v1::Request,
        _data: &(),
        _handle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            vicinae_hotkey_v1::Request::Destroy => {
                state.protocol_state.vicinae_hotkey.remove(hotkey);
            }
        }
    }

    fn destroyed(state: &mut Self, _client: ClientId, hotkey: &VicinaeHotkeyV1, _data: &()) {
        state.protocol_state.vicinae_hotkey.remove(hotkey);
    }
}

impl KestrelState {
    fn bind_vicinae_hotkey(
        &mut self,
        hotkey: VicinaeHotkeyV1,
        keysym: u32,
        modifiers: WEnum<HotkeyModifiers>,
        seat: Option<WlSeat>,
    ) -> BindReply {
        if let Some(seat) = &seat
            && !self.seat.owns(seat)
        {
            return BindReply::denied(DenyReason::Invalid, "unknown seat");
        }

        let modifiers = match modifiers {
            WEnum::Value(modifiers) => modifiers,
            WEnum::Unknown(_) => {
                return BindReply::denied(DenyReason::Invalid, "unknown modifier bits");
            }
        };

        if reserved_by_asher(keysym, modifiers) {
            return BindReply::denied(DenyReason::NotPermitted, "reserved by Asher");
        }

        self.protocol_state
            .vicinae_hotkey
            .bind(hotkey, keysym, modifiers)
    }
}

fn active_hotkey_modifiers(modifiers: &ModifiersState) -> HotkeyModifiers {
    let mut active = HotkeyModifiers::empty();
    if modifiers.shift {
        active |= HotkeyModifiers::Shift;
    }
    if modifiers.ctrl {
        active |= HotkeyModifiers::Ctrl;
    }
    if modifiers.alt {
        active |= HotkeyModifiers::Alt;
    }
    if modifiers.logo {
        active |= HotkeyModifiers::Super;
    }
    active
}

fn validate_binding(keysym: u32, modifiers: HotkeyModifiers) -> Result<(), BindReply> {
    if keysym == 0 || is_modifier_key(keysym) {
        return Err(BindReply::denied(
            DenyReason::Invalid,
            "invalid trigger keysym",
        ));
    }

    let trusted_modifier =
        modifiers.intersects(HotkeyModifiers::Ctrl | HotkeyModifiers::Alt | HotkeyModifiers::Super);
    if !trusted_modifier && !is_function_key(keysym) {
        return Err(BindReply::denied(
            DenyReason::NotPermitted,
            "hotkey needs Ctrl, Alt, Super, or a function key",
        ));
    }

    Ok(())
}

fn reserved_by_asher(keysym: u32, modifiers: HotkeyModifiers) -> bool {
    let shift = modifiers.contains(HotkeyModifiers::Shift);
    let ctrl = modifiers.contains(HotkeyModifiers::Ctrl);
    let alt = modifiers.contains(HotkeyModifiers::Alt);
    let logo = modifiers.contains(HotkeyModifiers::Super);

    if alt && !logo && !ctrl && matches!(keysym, 0xff09 | 0xfe20) {
        return true;
    }

    if !logo || ctrl || alt {
        return false;
    }

    matches!(
        keysym,
        0x20 | 0xff0d | 0x31
            ..=0x39
                | 0x65
                | 0x45
                | 0x71
                | 0x51
                | 0xff09
                | 0xfe20
                | 0xff51
                | 0xff52
                | 0xff53
                | 0xff54
    ) || (shift && matches!(keysym, 0x72 | 0x52))
}

fn is_function_key(keysym: u32) -> bool {
    (0xffbe..=0xffe0).contains(&keysym)
}

fn is_modifier_key(keysym: u32) -> bool {
    matches!(
        keysym,
        0xffe1..=0xffee | 0xfe01..=0xfe13 | 0xff7f
    )
}
