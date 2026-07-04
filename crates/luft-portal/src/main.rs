mod settings;

use settings::PortalSettings;
use std::{env, fs, fs::OpenOptions, io, thread, time::Duration};
use tracing::info;
use zbus::blocking::connection;

const DBUS_NAME: &str = "org.freedesktop.impl.portal.desktop.luft";
const OBJECT_PATH: &str = "/org/freedesktop/portal/desktop";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_logging();
    let _connection = connection::Builder::session()?
        .name(DBUS_NAME)?
        .serve_at(OBJECT_PATH, PortalSettings::new())?
        .build()?;

    info!(dbus_name = DBUS_NAME, "luft portal backend ready");
    loop {
        thread::sleep(Duration::from_secs(3600));
    }
}

fn init_logging() {
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_writer(log_writer("luft-portal"))
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("luft_portal=info")),
        )
        .init();
}

fn log_writer(component: &'static str) -> impl Fn() -> Box<dyn io::Write + Send> + Clone {
    let path = env::var_os("XDG_STATE_HOME")
        .map(|dir| {
            std::path::PathBuf::from(dir)
                .join("luft")
                .join("logs")
                .join(format!("{component}.log"))
        })
        .or_else(|| {
            env::var_os("HOME").map(|home| {
                std::path::PathBuf::from(home)
                    .join(".local/state/luft/logs")
                    .join(format!("{component}.log"))
            })
        });

    move || -> Box<dyn io::Write + Send> {
        let Some(path) = &path else {
            return Box::new(io::stderr());
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        match OpenOptions::new().create(true).append(true).open(path) {
            Ok(file) => Box::new(file),
            Err(_) => Box::new(io::stderr()),
        }
    }
}
