mod install;

use clap::{Subcommand, ValueEnum};
use install::{InstallSessionArgs, SetupArgs, install_session, run_setup};
use staccato_config::ConfigPaths;
use staccato_ipc::{IpcRequest, IpcResponse, send_request};
use std::{
    collections::BTreeMap,
    env, fs, io,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, SystemTime},
};

#[derive(Debug, Subcommand)]
pub enum DevCommand {
    Setup(SetupArgs),
    InstallSession(InstallSessionArgs),
    Apply {
        #[arg(value_enum)]
        targets: Vec<DevTarget>,
        #[arg(long)]
        release: bool,
    },
    Watch {
        #[arg(value_enum)]
        targets: Vec<DevTarget>,
        #[arg(long, default_value_t = 700)]
        debounce_ms: u64,
        #[arg(long)]
        release: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum DevTarget {
    Shell,
    Web,
    Config,
    Baton,
    All,
}

pub fn run_dev_command(command: DevCommand) -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    match command {
        DevCommand::Setup(args) => run_setup(&root, args),
        DevCommand::InstallSession(args) => install_session(&root, args),
        DevCommand::Apply { targets, release } => {
            run_apply(&root, DevPlan::apply(&targets), release)
        }
        DevCommand::Watch {
            targets,
            debounce_ms,
            release,
        } => run_watch(&root, DevPlan::watch(&targets), debounce_ms, release),
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct DevPlan {
    web: bool,
    shell: bool,
    config: bool,
    baton: bool,
}

impl DevPlan {
    fn apply(targets: &[DevTarget]) -> Self {
        Self::from_targets(
            targets,
            Self {
                web: true,
                shell: true,
                config: true,
                baton: false,
            },
        )
    }

    fn watch(targets: &[DevTarget]) -> Self {
        Self::from_targets(
            targets,
            Self {
                web: true,
                shell: true,
                config: true,
                baton: true,
            },
        )
    }

    fn from_targets(targets: &[DevTarget], default: Self) -> Self {
        if targets.is_empty() {
            return default;
        }

        let mut plan = Self::default();
        for target in targets {
            match target {
                DevTarget::Shell => plan.shell = true,
                DevTarget::Web => {
                    plan.web = true;
                    plan.shell = true;
                }
                DevTarget::Config => plan.config = true,
                DevTarget::Baton => plan.baton = true,
                DevTarget::All => {
                    plan.web = true;
                    plan.shell = true;
                    plan.config = true;
                    plan.baton = true;
                }
            }
        }
        plan
    }

    fn any(self) -> bool {
        self.web || self.shell || self.config || self.baton
    }
}

fn run_apply(root: &Path, plan: DevPlan, release: bool) -> Result<(), Box<dyn std::error::Error>> {
    if !plan.any() {
        return Ok(());
    }

    if plan.web {
        build_shell_web(root)?;
    }
    if plan.shell {
        build_shell(root, release)?;
        request_live("restarting shell", IpcRequest::RestartShell)?;
    }
    if plan.config {
        request_live("reloading config", IpcRequest::Reload)?;
    }
    if plan.baton {
        build_baton(root, release)?;
        println!("Baton rebuilt. Restart the session or nested compositor to use that binary.");
    }

    Ok(())
}

fn run_watch(
    root: &Path,
    plan: DevPlan,
    debounce_ms: u64,
    release: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let debounce = Duration::from_millis(debounce_ms);
    let poll = Duration::from_millis(250);
    let mut baseline = DevSnapshot::scan(root, plan)?;

    println!("Watching Staccato dev sources under {}", root.display());
    loop {
        thread::sleep(poll);
        let next = DevSnapshot::scan(root, plan)?;
        let mut changed = baseline.diff(&next);
        if !changed.any() {
            continue;
        }

        thread::sleep(debounce);
        let settled = DevSnapshot::scan(root, plan)?;
        changed = baseline.diff(&settled);
        if !changed.any() {
            baseline = settled;
            continue;
        }

        print_changed(changed);
        if let Err(error) = run_apply(root, changed, release) {
            eprintln!("dev apply failed: {error}");
        }
        baseline = DevSnapshot::scan(root, plan)?;
    }
}

fn print_changed(plan: DevPlan) {
    let mut names = Vec::new();
    if plan.web {
        names.push("web");
    }
    if plan.shell && !plan.web {
        names.push("shell");
    }
    if plan.config {
        names.push("config");
    }
    if plan.baton {
        names.push("baton");
    }
    println!("Changed: {}", names.join(", "));
}

pub(super) fn build_shell_web(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut command = Command::new("bun");
    command
        .arg("run")
        .arg("build")
        .current_dir(root.join("crates/staccato-shell/web"));
    run_process("building shell web assets", &mut command)
}

pub(super) fn build_shell(root: &Path, release: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut command = Command::new("cargo");
    command.arg("build").arg("-p").arg("staccato-shell");
    if release {
        command.arg("--release");
    }
    command.current_dir(root);
    run_process("building staccato-shell", &mut command)
}

pub(super) fn build_baton(root: &Path, release: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut command = Command::new("cargo");
    command
        .arg("build")
        .arg("-p")
        .arg("baton")
        .arg("--features")
        .arg("session-backend");
    if release {
        command.arg("--release");
    }
    command.current_dir(root);
    run_process("building baton session backend", &mut command)
}

pub(super) fn run_process(
    label: &str,
    command: &mut Command,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("{label}");
    let status = command.status().map_err(|error| {
        io::Error::new(error.kind(), format!("failed to start {label}: {error}"))
    })?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("{label} failed with {status}")).into())
    }
}

fn request_live(label: &str, request: IpcRequest) -> Result<(), Box<dyn std::error::Error>> {
    match send_request(&request) {
        Ok(IpcResponse::Accepted) => {
            println!("{label} accepted");
            Ok(())
        }
        Ok(IpcResponse::Error { message }) => Err(format!("{label} failed: {message}").into()),
        Ok(response) => Err(format!("{label} returned unexpected response: {response:?}").into()),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound
                    | io::ErrorKind::ConnectionRefused
                    | io::ErrorKind::ConnectionReset
            ) =>
        {
            eprintln!("{label} skipped: Baton IPC is not available ({error})");
            Ok(())
        }
        Err(error) => Err(format!("{label} failed: {error}").into()),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileStamp {
    modified: SystemTime,
    len: u64,
}

type FileSet = BTreeMap<PathBuf, FileStamp>;

#[derive(Debug, Default)]
struct DevSnapshot {
    web: FileSet,
    shell: FileSet,
    config: FileSet,
    baton: FileSet,
}

impl DevSnapshot {
    fn scan(root: &Path, plan: DevPlan) -> io::Result<Self> {
        Ok(Self {
            web: if plan.web {
                scan_paths(root, &web_paths())?
            } else {
                FileSet::new()
            },
            shell: if plan.shell {
                scan_paths(root, &shell_paths())?
            } else {
                FileSet::new()
            },
            config: if plan.config {
                scan_paths(root, &config_paths(root))?
            } else {
                FileSet::new()
            },
            baton: if plan.baton {
                scan_paths(root, &baton_paths())?
            } else {
                FileSet::new()
            },
        })
    }

    fn diff(&self, next: &Self) -> DevPlan {
        let web = changed(&self.web, &next.web);
        DevPlan {
            web,
            shell: web || changed(&self.shell, &next.shell),
            config: changed(&self.config, &next.config),
            baton: changed(&self.baton, &next.baton),
        }
    }
}

fn changed(before: &FileSet, after: &FileSet) -> bool {
    before.len() != after.len()
        || before
            .iter()
            .any(|(path, stamp)| after.get(path) != Some(stamp))
}

fn scan_paths(root: &Path, paths: &[PathBuf]) -> io::Result<FileSet> {
    let mut files = FileSet::new();
    for path in paths {
        collect_files(&absolute_path(root, path), &mut files)?;
    }
    Ok(files)
}

fn collect_files(path: &Path, files: &mut FileSet) -> io::Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };

    if metadata.is_file() {
        files.insert(
            path.to_path_buf(),
            FileStamp {
                modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                len: metadata.len(),
            },
        );
        return Ok(());
    }

    if !metadata.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();
        if ignored_path(&entry_path) {
            continue;
        }
        collect_files(&entry_path, files)?;
    }
    Ok(())
}

fn ignored_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            matches!(
                name,
                ".git" | "target" | "node_modules" | "dist" | ".svelte-kit" | ".vite"
            )
        })
}

fn absolute_path(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn web_paths() -> Vec<PathBuf> {
    [
        "crates/staccato-shell/web/src",
        "crates/staccato-shell/web/index.html",
        "crates/staccato-shell/web/index.ts",
        "crates/staccato-shell/web/package.json",
        "crates/staccato-shell/web/bun.lock",
        "crates/staccato-shell/web/tsconfig.json",
        "crates/staccato-shell/web/vite.config.ts",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect()
}

fn shell_paths() -> Vec<PathBuf> {
    [
        "crates/staccato-shell/src",
        "crates/staccato-shell/Cargo.toml",
        "crates/staccato-config/src",
        "crates/staccato-config/Cargo.toml",
        "crates/staccato-ipc/src",
        "crates/staccato-ipc/Cargo.toml",
        "crates/staccato-layout/src",
        "crates/staccato-layout/Cargo.toml",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect()
}

fn config_paths(root: &Path) -> Vec<PathBuf> {
    let mut paths = vec![root.join("data/default-config")];
    if let Ok(config) = ConfigPaths::discover() {
        paths.push(config.config_file);
        paths.push(config.profiles_dir);
    }
    paths
}

fn baton_paths() -> Vec<PathBuf> {
    [
        "crates/baton/src",
        "crates/baton/Cargo.toml",
        "crates/staccato-session/src",
        "crates/staccato-session/Cargo.toml",
        "crates/staccato-config/src",
        "crates/staccato-config/Cargo.toml",
        "crates/staccato-ipc/src",
        "crates/staccato-ipc/Cargo.toml",
        "crates/staccato-layout/src",
        "crates/staccato-layout/Cargo.toml",
        "data/sessions",
        "data/xdg-desktop-portal",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect()
}

fn workspace_root() -> io::Result<PathBuf> {
    if let Some(root) = env::var_os("STACCATO_WORKSPACE").map(PathBuf::from)
        && is_workspace_root(&root)
    {
        return Ok(root);
    }

    if let Ok(current) = env::current_dir()
        && let Some(root) = current.ancestors().find(|path| is_workspace_root(path))
    {
        return Ok(root.to_path_buf());
    }

    if let Ok(exe) = env::current_exe() {
        for path in exe.ancestors() {
            if path.file_name().is_some_and(|name| name == "target")
                && let Some(root) = path.parent()
                && is_workspace_root(root)
            {
                return Ok(root.to_path_buf());
            }
        }
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "could not find the Staccato workspace root; run from the repo or set STACCATO_WORKSPACE",
    ))
}

fn is_workspace_root(path: &Path) -> bool {
    path.join("Cargo.toml").is_file()
        && path
            .join("crates/staccato-shell/web/package.json")
            .is_file()
}
