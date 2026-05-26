use clap::Parser;
use staccato_config::{BackendPreference, ConfigPaths, ConfigSource, load_config_or_default};
use staccato_session::{SessionDescriptor, SessionEnvironment, validate_descriptor};
use std::{
    env, fs,
    fs::OpenOptions,
    io,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};
use tracing::{info, warn};

const PRIVATE_DBUS_ENV: &str = "STACCATO_PRIVATE_DBUS";

#[derive(Debug, Parser)]
#[command(name = "staccato-session", about = "Start a Staccato desktop session")]
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
    baton: Option<PathBuf>,
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
            eprintln!("staccato-session: {error}");
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

    let (loaded, config_error) = load_config_or_default();
    if let Some(error) = config_error {
        warn!(%error, "failed to load user config; using built-in default config");
    } else {
        match &loaded.source {
            ConfigSource::User(path) => info!(path = %path.display(), "loaded user config"),
            ConfigSource::Defaults => warn!("using built-in default config"),
        }
    }

    let backend = selected_backend(&args, loaded.config.compositor.backend);
    let baton = resolve_baton(args.baton);
    let environment = SessionEnvironment::default();
    let mut command = session_command(&baton, backend);
    environment.apply_to_command(&mut command);

    if let Some(socket) = args.socket {
        command.arg("--socket").arg(socket);
    }

    if args.dry_run {
        println!("{}", describe_launch(&command, backend));
        return Ok(());
    }

    info!(baton = %baton.display(), backend = backend.name(), "starting Baton");
    let error = command.exec();
    Err(Box::new(error))
}

fn print_desktop_entry() -> Result<(), Box<dyn std::error::Error>> {
    let descriptor = SessionDescriptor::default();
    validate_descriptor(&descriptor)?;
    print!("{}", descriptor.desktop_entry());
    Ok(())
}

fn resolve_baton(explicit: Option<PathBuf>) -> PathBuf {
    if let Some(path) = explicit {
        return path;
    }
    if let Some(path) = env::var_os("STACCATO_BATON") {
        return PathBuf::from(path);
    }
    if let Some(path) = sibling_binary("baton") {
        return path;
    }
    PathBuf::from("baton")
}

fn session_command(baton: &Path, backend: BatonBackend) -> Command {
    if env::var_os("STACCATO_USE_HOST_DBUS").is_none()
        && let Some(dbus_run_session) = find_in_path("dbus-run-session")
    {
        let mut command = Command::new(dbus_run_session);
        command
            .env(PRIVATE_DBUS_ENV, "1")
            .arg("--")
            .arg(baton)
            .arg(backend.flag());
        return command;
    }

    let mut command = Command::new(baton);
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

fn selected_backend(args: &SessionArgs, preference: BackendPreference) -> BatonBackend {
    if args.nested {
        return BatonBackend::Nested;
    }
    if args.headless {
        return BatonBackend::Headless;
    }
    if args.session {
        return BatonBackend::Session;
    }

    match preference {
        BackendPreference::Nested => BatonBackend::Nested,
        BackendPreference::Headless => BatonBackend::Headless,
        BackendPreference::Session => BatonBackend::Session,
        BackendPreference::Auto => {
            if env::var_os("WAYLAND_DISPLAY").is_some() {
                BatonBackend::Nested
            } else {
                BatonBackend::Session
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

fn describe_launch(command: &Command, backend: BatonBackend) -> String {
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
        .with_writer(file_log_writer("staccato-session"))
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("staccato_session=info")),
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
enum BatonBackend {
    Nested,
    Headless,
    Session,
}

impl BatonBackend {
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
        assert_eq!(shell_escape("/usr/bin/baton"), "/usr/bin/baton");
        assert_eq!(shell_escape("--session"), "--session");
    }

    #[test]
    fn shell_escape_quotes_values_with_spaces() {
        assert_eq!(shell_escape("/tmp/Staccato Baton"), "'/tmp/Staccato Baton'");
    }

    #[test]
    fn dry_run_description_shows_backend_env_and_command() {
        let mut command = Command::new("/usr/bin/baton");
        command.env("XDG_SESSION_TYPE", "wayland").arg("--session");

        let description = describe_launch(&command, BatonBackend::Session);

        assert!(description.contains("backend=session"));
        assert!(description.contains("env XDG_SESSION_TYPE=wayland"));
        assert!(description.contains("command=/usr/bin/baton --session"));
    }
}
