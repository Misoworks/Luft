use asher_config::{BackendPreference, ConfigPaths, ConfigSource, load_config};
use asher_session::{SessionDescriptor, SessionEnvironment, validate_descriptor};
use clap::Parser;
use std::{
    env, fs,
    fs::OpenOptions,
    io,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};
use tracing::{info, warn};

const PRIVATE_DBUS_ENV: &str = "ASHER_PRIVATE_DBUS";

#[derive(Debug, Parser)]
#[command(name = "asher-session", about = "Start a Asher desktop session")]
struct SessionArgs {
    #[arg(long, conflicts_with_all = ["headless", "session"])]
    nested: bool,
    #[arg(long, conflicts_with_all = ["nested", "session"])]
    headless: bool,
    #[arg(long, conflicts_with_all = ["nested", "headless"])]
    session: bool,
    #[arg(long)]
    socket: Option<String>,
    #[arg(long)]
    kestrel: Option<PathBuf>,
    #[arg(long)]
    desktop_entry: bool,
    #[arg(long)]
    dry_run: bool,
}

fn main() -> ExitCode {
    init_logging();
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("asher-session: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = SessionArgs::parse();
    if args.desktop_entry {
        print_desktop_entry()?;
        return Ok(());
    }

    let loaded = load_config()?;
    match &loaded.source {
        ConfigSource::User(path) => info!(path = %path.display(), "loaded user config"),
        ConfigSource::Defaults => warn!("using built-in default config"),
    }

    let backend = selected_backend(&args, loaded.config.compositor.backend);
    let kestrel = resolve_kestrel(args.kestrel.clone());
    let environment = SessionEnvironment::default();
    let mut command = session_command(&kestrel, backend);
    environment.apply_to_command(&mut command);

    if let Some(socket) = &args.socket {
        command.arg("--socket").arg(socket);
    }

    if args.dry_run {
        println!("{}", describe_launch(&command, backend));
        return Ok(());
    }

    info!(kestrel = %kestrel.display(), backend = backend.name(), "starting Kestrel");
    let error = command.exec();
    Err(Box::new(error))
}

fn print_desktop_entry() -> Result<(), Box<dyn std::error::Error>> {
    let descriptor = SessionDescriptor::default();
    validate_descriptor(&descriptor)?;
    print!("{}", descriptor.desktop_entry());
    Ok(())
}

fn resolve_kestrel(explicit: Option<PathBuf>) -> PathBuf {
    if let Some(path) = explicit {
        return path;
    }
    if let Some(path) = env::var_os("ASHER_KESTREL") {
        return PathBuf::from(path);
    }
    if let Some(path) = sibling_binary("kestrel") {
        return path;
    }
    PathBuf::from("kestrel")
}

fn session_command(kestrel: &Path, backend: KestrelBackend) -> Command {
    if env::var_os("ASHER_USE_HOST_DBUS").is_none()
        && let Some(dbus_run_session) = find_in_path("dbus-run-session")
    {
        let mut command = Command::new(dbus_run_session);
        command
            .env(PRIVATE_DBUS_ENV, "1")
            .arg("--")
            .arg(kestrel)
            .arg(backend.flag());
        return command;
    }

    let mut command = Command::new(kestrel);
    command.arg(backend.flag());
    command
}

fn find_in_path(program: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file())
}

fn sibling_binary(name: &str) -> Option<PathBuf> {
    let mut path = env::current_exe().ok()?;
    path.set_file_name(name);
    path.exists().then_some(path)
}

fn selected_backend(args: &SessionArgs, preference: BackendPreference) -> KestrelBackend {
    if args.nested {
        return KestrelBackend::Nested;
    }
    if args.headless {
        return KestrelBackend::Headless;
    }
    if args.session {
        return KestrelBackend::Session;
    }

    match preference {
        BackendPreference::Nested => KestrelBackend::Nested,
        BackendPreference::Headless => KestrelBackend::Headless,
        BackendPreference::Session => KestrelBackend::Session,
        BackendPreference::Auto => {
            if env::var_os("WAYLAND_DISPLAY").is_some() {
                KestrelBackend::Nested
            } else {
                KestrelBackend::Session
            }
        }
    }
}

fn describe_command(command: &Command) -> String {
    let program = command.get_program().to_string_lossy();
    let args = command
        .get_args()
        .map(|arg| shell_escape(&arg.to_string_lossy()))
        .collect::<Vec<_>>()
        .join(" ");
    if args.is_empty() {
        program.to_string()
    } else {
        format!("{program} {args}")
    }
}

fn describe_launch(command: &Command, backend: KestrelBackend) -> String {
    let mut lines = vec![format!("backend={}", backend.name())];
    let mut envs = command
        .get_envs()
        .filter_map(|(name, value)| {
            value.map(|value| {
                (
                    name.to_string_lossy().into_owned(),
                    value.to_string_lossy().into_owned(),
                )
            })
        })
        .collect::<Vec<_>>();
    envs.sort_by(|left, right| left.0.cmp(&right.0));
    for (name, value) in envs {
        lines.push(format!("env {name}={}", shell_escape(&value)));
    }
    lines.push(format!("command={}", describe_command(command)));
    lines.join("\n")
}

fn shell_escape(value: &str) -> String {
    if value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || "-_./:=+".contains(character))
    {
        return value.to_string();
    }

    format!("'{}'", value.replace('\'', "'\\''"))
}

fn init_logging() {
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_writer(file_log_writer("asher-session"))
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("asher_session=info")),
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

#[derive(Debug, Clone, Copy)]
enum KestrelBackend {
    Nested,
    Headless,
    Session,
}

impl KestrelBackend {
    fn flag(self) -> &'static str {
        match self {
            Self::Nested => "--nested",
            Self::Headless => "--headless",
            Self::Session => "--session",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Nested => "nested",
            Self::Headless => "headless",
            Self::Session => "session",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_escape_keeps_safe_values_plain() {
        assert_eq!(shell_escape("/usr/bin/kestrel"), "/usr/bin/kestrel");
        assert_eq!(shell_escape("--session"), "--session");
    }

    #[test]
    fn shell_escape_quotes_values_with_spaces() {
        assert_eq!(shell_escape("/tmp/Asher Kestrel"), "'/tmp/Asher Kestrel'");
    }

    #[test]
    fn dry_run_description_shows_backend_env_and_command() {
        let mut command = Command::new("/usr/bin/kestrel");
        command.env("XDG_SESSION_TYPE", "wayland").arg("--session");

        let description = describe_launch(&command, KestrelBackend::Session);

        assert!(description.contains("backend=session"));
        assert!(description.contains("env XDG_SESSION_TYPE=wayland"));
        assert!(description.contains("command=/usr/bin/kestrel --session"));
    }
}
