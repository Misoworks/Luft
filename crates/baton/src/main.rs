mod backend;
mod background;
mod background_effect;
mod blur_material;
mod client;
mod cursor;
mod damage;
mod debug_overlay;
mod frame_clock;
mod input;
mod ipc;
mod ipc_summary;
mod layers;
mod layout_config;
mod loading_overlay;
mod output;
mod protocol_state;
mod protocols;
mod recovery;
mod render;
mod scene_blur;
mod scene_render;
mod shell;
mod state;
mod state_focus;
mod titlebar;
mod window;
mod window_animation;
mod window_clip;
mod window_geometry;
mod workspace_transition;
mod xwayland;

use backend::RuntimeBackend;
use clap::Parser;
use staccato_config::{ConfigPaths, load_config_or_default};
use std::{fs, fs::OpenOptions, io};
use tracing::{info, warn};

#[derive(Debug, Parser)]
#[command(name = "baton", about = "Baton Wayland compositor for Staccato")]
struct BatonArgs {
    #[arg(long, conflicts_with_all = ["headless", "session"])]
    nested: bool,
    #[arg(long, conflicts_with_all = ["nested", "session"])]
    headless: bool,
    #[arg(long, conflicts_with_all = ["nested", "headless"])]
    session: bool,
    #[arg(long)]
    socket: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_logging();

    let args = BatonArgs::parse();
    let backend = selected_backend(&args);
    let (loaded_config, config_error) = load_config_or_default();

    if let Some(error) = config_error {
        warn!(%error, "failed to load user config; using built-in default config");
    } else {
        match &loaded_config.source {
            staccato_config::ConfigSource::User(path) => {
                info!(path = %path.display(), "loaded user config")
            }
            staccato_config::ConfigSource::Defaults => warn!("using built-in default config"),
        }
    }

    backend::run(backend, loaded_config.config, args.socket)?;
    Ok(())
}

fn selected_backend(args: &BatonArgs) -> RuntimeBackend {
    if args.headless {
        RuntimeBackend::Headless
    } else if args.session {
        RuntimeBackend::Session
    } else {
        RuntimeBackend::Nested
    }
}

fn init_logging() {
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_writer(file_log_writer("baton"))
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("baton=info,smithay=warn")),
        )
        .finish();

    let _ = tracing::subscriber::set_global_default(subscriber);
}

fn file_log_writer(component: &'static str) -> impl Fn() -> Box<dyn io::Write + Send> + Clone {
    let path = ConfigPaths::discover()
        .ok()
        .map(|paths| paths.log_file(component));
    move || -> Box<dyn io::Write + Send> {
        let Some(path) = &path else {
            return Box::new(io::stderr());
        };
        if let Some(parent) = path.parent()
            && fs::create_dir_all(parent).is_err()
        {
            return Box::new(io::stderr());
        }
        match OpenOptions::new().create(true).append(true).open(path) {
            Ok(file) => Box::new(file),
            Err(_) => Box::new(io::stderr()),
        }
    }
}
