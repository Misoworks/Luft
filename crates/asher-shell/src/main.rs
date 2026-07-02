mod apps;
mod color;
mod control;
mod ipc;
mod panel;
mod services;
mod theme;
mod web;

use asher_config::{ConfigPaths, load_config};
use clap::Parser;
use std::{env, fs, fs::OpenOptions, io};
use tracing::info;

#[derive(Debug, Parser)]
#[command(name = "asher-shell", about = "Asher shell process")]
struct ShellArgs {
    #[arg(long)]
    once: bool,
    #[arg(long, hide = true)]
    refresh_fenestra_host: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let raw_args = env::args().collect::<Vec<_>>();
    if fenestra_cef::run_fenestra_host_from_args(&raw_args) {
        return Ok(());
    }

    disable_accessibility_bridge();
    init_logging();

    let args = ShellArgs::parse();
    if args.refresh_fenestra_host {
        return refresh_fenestra_host();
    }

    let loaded = load_config()?;
    info!("asher shell configuration loaded");

    if args.once {
        return Ok(());
    }

    web::run(loaded.config)
}

fn refresh_fenestra_host() -> Result<(), Box<dyn std::error::Error>> {
    let config = fenestra_cef::RuntimeConfig::default();
    let runtime = match fenestra_cef::resolve_runtime(&config) {
        Ok(runtime) => runtime,
        Err(_) if config.allow_user_install => {
            fenestra_cef::install_user_runtime_with_progress(&config, |progress| {
                if let Some(fraction) = progress.fraction {
                    eprintln!(
                        "Fenestra runtime: {} ({:.0}%)",
                        progress.message,
                        fraction * 100.0
                    );
                } else {
                    eprintln!("Fenestra runtime: {}", progress.message);
                }
            })?
        }
        Err(error) => return Err(Box::new(error)),
    };

    let runtime_dir = runtime.location.path();
    let source_stamp = runtime_dir
        .join(".fenestra-host-build")
        .join("fenestra-host-source.fnv");
    let _ = fs::remove_file(source_stamp);
    let host = fenestra_cef::ensure_cef_host(runtime_dir)?;
    println!("Refreshed Fenestra CEF host: {}", host.display());
    Ok(())
}

fn disable_accessibility_bridge() {
    unsafe {
        env::set_var("NO_AT_BRIDGE", "1");
        env::set_var("GTK_A11Y", "none");
        env::set_var("GTK_MODULES", "");
    }
}

fn init_logging() {
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_writer(file_log_writer("asher-shell"))
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("asher_shell=info")),
        )
        .init();
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
