use crate::{background_effect::BackgroundEffectGlobal, state::BatonState};
use smithay::{
    reexports::wayland_server::DisplayHandle,
    utils::{Clock, Monotonic},
    wayland::{
        alpha_modifier::AlphaModifierState, cursor_shape::CursorShapeManagerState,
        fractional_scale::FractionalScaleManagerState, output::OutputManagerState,
        presentation::PresentationState, shell::xdg::decoration::XdgDecorationState,
        text_input::TextInputManagerState, viewporter::ViewporterState,
        xdg_activation::XdgActivationState, xdg_toplevel_icon::XdgToplevelIconManager,
    },
};

pub struct ProtocolState {
    pub xdg_activation: XdgActivationState,
    _xdg_decoration: XdgDecorationState,
    _cursor_shape: CursorShapeManagerState,
    _fractional_scale: FractionalScaleManagerState,
    _viewporter: ViewporterState,
    _xdg_toplevel_icon: XdgToplevelIconManager,
    _text_input: TextInputManagerState,
    _presentation: PresentationState,
    _output: OutputManagerState,
    _background_effect: BackgroundEffectGlobal,
    _alpha_modifier: AlphaModifierState,
}

impl ProtocolState {
    pub fn new(display: &DisplayHandle) -> Self {
        let mut xdg_toplevel_icon = XdgToplevelIconManager::new::<BatonState>(display);
        for size in [16, 24, 32, 48, 64, 128] {
            xdg_toplevel_icon.add_icon_size(size);
        }

        Self {
            xdg_activation: XdgActivationState::new::<BatonState>(display),
            _xdg_decoration: XdgDecorationState::new::<BatonState>(display),
            _cursor_shape: CursorShapeManagerState::new::<BatonState>(display),
            _fractional_scale: FractionalScaleManagerState::new::<BatonState>(display),
            _viewporter: ViewporterState::new::<BatonState>(display),
            _xdg_toplevel_icon: xdg_toplevel_icon,
            _text_input: TextInputManagerState::new::<BatonState>(display),
            _presentation: PresentationState::new::<BatonState>(
                display,
                Clock::<Monotonic>::new().id() as u32,
            ),
            _output: OutputManagerState::new_with_xdg_output::<BatonState>(display),
            _background_effect: BackgroundEffectGlobal::new(display),
            _alpha_modifier: AlphaModifierState::new::<BatonState>(display),
        }
    }
}
