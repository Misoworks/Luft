use crate::{
    background_effect::BackgroundEffectGlobal, state::KestrelState,
    vicinae_hotkey::VicinaeHotkeyState,
};
#[cfg(feature = "session-backend")]
use smithay::wayland::{dmabuf::DmabufGlobal, drm_syncobj::DrmSyncobjState};
use smithay::{
    reexports::wayland_server::DisplayHandle,
    utils::{Clock, Monotonic},
    wayland::{
        alpha_modifier::AlphaModifierState, cursor_shape::CursorShapeManagerState,
        dmabuf::DmabufState, fractional_scale::FractionalScaleManagerState,
        idle_inhibit::IdleInhibitManagerState,
        keyboard_shortcuts_inhibit::KeyboardShortcutsInhibitState, output::OutputManagerState,
        pointer_constraints::PointerConstraintsState, pointer_gestures::PointerGesturesState,
        presentation::PresentationState, relative_pointer::RelativePointerManagerState,
        shell::xdg::decoration::XdgDecorationState, text_input::TextInputManagerState,
        viewporter::ViewporterState, xdg_activation::XdgActivationState,
        xdg_foreign::XdgForeignState, xdg_toplevel_icon::XdgToplevelIconManager,
    },
};

pub struct ProtocolState {
    pub xdg_activation: XdgActivationState,
    pub xdg_foreign: XdgForeignState,
    pub keyboard_shortcuts_inhibit: KeyboardShortcutsInhibitState,
    pub dmabuf: DmabufState,
    #[cfg(feature = "session-backend")]
    pub dmabuf_global: Option<DmabufGlobal>,
    #[cfg(feature = "session-backend")]
    pub drm_syncobj: Option<DrmSyncobjState>,
    _xdg_decoration: XdgDecorationState,
    _cursor_shape: CursorShapeManagerState,
    _fractional_scale: FractionalScaleManagerState,
    _viewporter: ViewporterState,
    _xdg_toplevel_icon: XdgToplevelIconManager,
    _text_input: TextInputManagerState,
    _presentation: PresentationState,
    _output: OutputManagerState,
    _background_effect: BackgroundEffectGlobal,
    pub vicinae_hotkey: VicinaeHotkeyState,
    _alpha_modifier: AlphaModifierState,
    _relative_pointer: RelativePointerManagerState,
    _pointer_gestures: PointerGesturesState,
    _pointer_constraints: PointerConstraintsState,
    _idle_inhibit: IdleInhibitManagerState,
}

impl ProtocolState {
    pub fn new(display: &DisplayHandle) -> Self {
        let mut xdg_toplevel_icon = XdgToplevelIconManager::new::<KestrelState>(display);
        for size in [16, 24, 32, 48, 64, 128] {
            xdg_toplevel_icon.add_icon_size(size);
        }

        Self {
            xdg_activation: XdgActivationState::new::<KestrelState>(display),
            xdg_foreign: XdgForeignState::new::<KestrelState>(display),
            keyboard_shortcuts_inhibit: KeyboardShortcutsInhibitState::new::<KestrelState>(display),
            dmabuf: DmabufState::new(),
            #[cfg(feature = "session-backend")]
            dmabuf_global: None,
            #[cfg(feature = "session-backend")]
            drm_syncobj: None,
            _xdg_decoration: XdgDecorationState::new::<KestrelState>(display),
            _cursor_shape: CursorShapeManagerState::new::<KestrelState>(display),
            _fractional_scale: FractionalScaleManagerState::new::<KestrelState>(display),
            _viewporter: ViewporterState::new::<KestrelState>(display),
            _xdg_toplevel_icon: xdg_toplevel_icon,
            _text_input: TextInputManagerState::new::<KestrelState>(display),
            _presentation: PresentationState::new::<KestrelState>(
                display,
                Clock::<Monotonic>::new().id() as u32,
            ),
            _output: OutputManagerState::new_with_xdg_output::<KestrelState>(display),
            _background_effect: BackgroundEffectGlobal::new(display),
            vicinae_hotkey: VicinaeHotkeyState::new(display),
            _alpha_modifier: AlphaModifierState::new::<KestrelState>(display),
            _relative_pointer: RelativePointerManagerState::new::<KestrelState>(display),
            _pointer_gestures: PointerGesturesState::new::<KestrelState>(display),
            _pointer_constraints: PointerConstraintsState::new::<KestrelState>(display),
            _idle_inhibit: IdleInhibitManagerState::new::<KestrelState>(display),
        }
    }
}
