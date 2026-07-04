use crate::state::KestrelState;
use smithay::{
    reexports::{
        wayland_protocols::ext::background_effect::v1::server::{
            ext_background_effect_manager_v1::{self, Capability, ExtBackgroundEffectManagerV1},
            ext_background_effect_surface_v1::{self, ExtBackgroundEffectSurfaceV1},
        },
        wayland_server::{
            Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource, Weak,
            backend::{ClientId, GlobalId},
            protocol::wl_surface::WlSurface,
        },
    },
    wayland::compositor::{self, Cacheable, RegionAttributes},
};
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

mod targets;

pub use targets::{layer_popup_blur_targets, window_blur_targets, window_blur_targets_grouped};

#[derive(Debug)]
pub struct BackgroundEffectGlobal {
    _global: GlobalId,
}

impl BackgroundEffectGlobal {
    pub fn new(display: &DisplayHandle) -> Self {
        Self {
            _global: display.create_global::<KestrelState, ExtBackgroundEffectManagerV1, _>(1, ()),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct BackgroundEffectSurfaceState {
    blur_region: Option<RegionAttributes>,
}

impl Cacheable for BackgroundEffectSurfaceState {
    fn commit(&mut self, _dh: &DisplayHandle) -> Self {
        self.clone()
    }

    fn merge_into(self, into: &mut Self, _dh: &DisplayHandle) {
        *into = self;
    }
}

#[derive(Debug)]
struct BackgroundEffectSurfaceData {
    attached: AtomicBool,
}

impl BackgroundEffectSurfaceData {
    fn new() -> Self {
        Self {
            attached: AtomicBool::new(false),
        }
    }

    fn is_attached(&self) -> bool {
        self.attached.load(Ordering::Acquire)
    }

    fn set_attached(&self, attached: bool) {
        self.attached.store(attached, Ordering::Release);
    }
}

#[derive(Debug)]
pub struct BackgroundEffectSurfaceUserData(Mutex<Weak<WlSurface>>);

impl BackgroundEffectSurfaceUserData {
    fn new(surface: WlSurface) -> Self {
        Self(Mutex::new(surface.downgrade()))
    }

    fn surface(&self) -> Option<WlSurface> {
        self.0.lock().unwrap().upgrade().ok()
    }
}

impl GlobalDispatch<ExtBackgroundEffectManagerV1, ()> for KestrelState {
    fn bind(
        _state: &mut Self,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<ExtBackgroundEffectManagerV1>,
        _global_data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        let manager = data_init.init(resource, ());
        manager.capabilities(Capability::Blur);
    }
}

impl Dispatch<ExtBackgroundEffectManagerV1, ()> for KestrelState {
    fn request(
        _state: &mut Self,
        _client: &Client,
        manager: &ExtBackgroundEffectManagerV1,
        request: ext_background_effect_manager_v1::Request,
        _data: &(),
        _handle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            ext_background_effect_manager_v1::Request::GetBackgroundEffect { id, surface } => {
                let already_attached = compositor::with_states(&surface, |states| {
                    states
                        .data_map
                        .insert_if_missing_threadsafe(BackgroundEffectSurfaceData::new);
                    let data = states
                        .data_map
                        .get::<BackgroundEffectSurfaceData>()
                        .unwrap();
                    let already_attached = data.is_attached();

                    if !already_attached {
                        data.set_attached(true);
                        drop(states.cached_state.get::<BackgroundEffectSurfaceState>());
                    }

                    already_attached
                });

                if already_attached {
                    manager.post_error(
                        ext_background_effect_manager_v1::Error::BackgroundEffectExists,
                        "wl_surface already has a background effect object",
                    );
                    return;
                }

                data_init.init(id, BackgroundEffectSurfaceUserData::new(surface));
            }
            ext_background_effect_manager_v1::Request::Destroy => {}
            _ => {}
        }
    }
}

impl Dispatch<ExtBackgroundEffectSurfaceV1, BackgroundEffectSurfaceUserData> for KestrelState {
    fn request(
        _state: &mut Self,
        _client: &Client,
        resource: &ExtBackgroundEffectSurfaceV1,
        request: ext_background_effect_surface_v1::Request,
        data: &BackgroundEffectSurfaceUserData,
        _handle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            ext_background_effect_surface_v1::Request::SetBlurRegion { region } => {
                let Some(surface) = data.surface() else {
                    resource.post_error(
                        ext_background_effect_surface_v1::Error::SurfaceDestroyed,
                        "associated wl_surface has been destroyed",
                    );
                    return;
                };
                let region = region.map(|region| compositor::get_region_attributes(&region));
                compositor::with_states(&surface, |states| {
                    states
                        .cached_state
                        .get::<BackgroundEffectSurfaceState>()
                        .pending()
                        .blur_region = region;
                });
            }
            ext_background_effect_surface_v1::Request::Destroy => {
                if let Some(surface) = data.surface() {
                    compositor::with_states(&surface, |states| {
                        if let Some(data) = states.data_map.get::<BackgroundEffectSurfaceData>() {
                            data.set_attached(false);
                        }
                        states
                            .cached_state
                            .get::<BackgroundEffectSurfaceState>()
                            .pending()
                            .blur_region = None;
                    });
                }
            }
            _ => {}
        }
    }

    fn destroyed(
        _state: &mut Self,
        _client: ClientId,
        _object: &ExtBackgroundEffectSurfaceV1,
        _data: &BackgroundEffectSurfaceUserData,
    ) {
    }
}

pub(crate) fn current_blur_region(surface: &WlSurface) -> Option<RegionAttributes> {
    compositor::with_states(surface, |states| {
        if !states.cached_state.has::<BackgroundEffectSurfaceState>() {
            return None;
        }

        states
            .cached_state
            .get::<BackgroundEffectSurfaceState>()
            .current()
            .blur_region
            .clone()
    })
}
