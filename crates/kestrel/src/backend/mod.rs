pub mod drm;
pub mod headless;
pub mod nested;
mod nested_timing;

use luft_config::LuftConfig;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeBackend {
    Nested,
    Headless,
    Session,
}

pub fn run(
    backend: RuntimeBackend,
    config: LuftConfig,
    socket_name: Option<String>,
) -> Result<(), BackendError> {
    match backend {
        RuntimeBackend::Nested => Ok(nested::run(nested::NestedOptions {
            config,
            socket_name,
        })?),
        RuntimeBackend::Headless => Ok(headless::run(headless::HeadlessOptions {
            config,
            socket_name,
        })?),
        RuntimeBackend::Session => Ok(drm::run(drm::DrmOptions {
            config,
            socket_name,
        })?),
    }
}

#[derive(Debug, Error)]
pub enum BackendError {
    #[error(transparent)]
    Nested(#[from] nested::NestedError),
    #[error(transparent)]
    Headless(#[from] headless::HeadlessError),
    #[error(transparent)]
    Drm(#[from] drm::DrmError),
}
