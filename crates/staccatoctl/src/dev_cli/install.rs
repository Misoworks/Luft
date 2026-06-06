use super::{build_baton, build_shell, build_shell_web, run_process};
use clap::Args;
use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

const BINARIES: [&str; 4] = ["baton", "staccato-session", "staccato-shell", "staccatoctl"];

#[derive(Debug, Args)]
pub struct SetupArgs {
    #[arg(long)]
    pub release: bool,
    #[arg(long)]
    pub no_bun_install: bool,
}

#[derive(Debug, Args)]
pub struct InstallSessionArgs {
    #[arg(long)]
    pub release: bool,
    #[arg(long)]
    pub copy_binaries: bool,
    #[arg(long, default_value = "/usr/share/wayland-sessions")]
    pub session_dir: PathBuf,
    #[arg(long, default_value = "/usr/share/xdg-desktop-portal")]
    pub portal_dir: PathBuf,
    #[arg(long, default_value = "/usr/local/bin")]
    pub bin_dir: PathBuf,
}

pub fn run_setup(root: &Path, args: SetupArgs) -> Result<(), Box<dyn std::error::Error>> {
    if !args.no_bun_install {
        let mut command = Command::new("bun");
        command
            .arg("install")
            .current_dir(root.join("crates/staccato-shell/web"));
        run_process("installing shell web dependencies", &mut command)?;
    }

    build_shell_web(root)?;
    build_shell(root, args.release)?;
    build_baton(root, args.release)?;
    build_package(root, "staccato-session", args.release)?;
    build_package(root, "staccatoctl", args.release)?;

    let profile = profile_dir(args.release);
    println!(
        "Setup complete. Install the login entry with: sudo {}/staccatoctl dev install-session{}",
        root.join("target").join(profile).display(),
        if args.release { " --release" } else { "" }
    );
    Ok(())
}

pub fn install_session(
    root: &Path,
    args: InstallSessionArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    let target_dir = root.join("target").join(profile_dir(args.release));
    ensure_binaries_exist(&target_dir)?;

    let session_binary = if args.copy_binaries {
        copy_binaries(&target_dir, &args.bin_dir)?;
        args.bin_dir.join("staccato-session")
    } else {
        target_dir.join("staccato-session")
    };

    install_desktop_entry(&args.session_dir, &session_binary, args.copy_binaries)?;
    install_portal_config(root, &args.portal_dir)?;

    println!(
        "Installed Staccato login entry at {}",
        args.session_dir.join("staccato.desktop").display()
    );
    println!(
        "Installed portal preferences at {}",
        args.portal_dir.join("staccato-portals.conf").display()
    );
    if !args.copy_binaries {
        println!("The login entry points at {}", target_dir.display());
    }
    Ok(())
}

fn build_package(
    root: &Path,
    package: &str,
    release: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut command = Command::new("cargo");
    command.arg("build").arg("-p").arg(package);
    if release {
        command.arg("--release");
    }
    command.current_dir(root);
    run_process(&format!("building {package}"), &mut command)
}

fn ensure_binaries_exist(target_dir: &Path) -> io::Result<()> {
    for binary in BINARIES {
        let path = target_dir.join(binary);
        if !path.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "{} is missing; run `staccatoctl dev setup` first",
                    path.display()
                ),
            ));
        }
    }
    Ok(())
}

fn copy_binaries(source_dir: &Path, bin_dir: &Path) -> io::Result<()> {
    fs::create_dir_all(bin_dir)?;
    for binary in BINARIES {
        let source = source_dir.join(binary);
        let target = bin_dir.join(binary);
        fs::copy(&source, &target)?;
        set_executable(&target)?;
        println!("installed {}", target.display());
    }
    Ok(())
}

fn install_desktop_entry(
    session_dir: &Path,
    session_binary: &Path,
    include_try_exec: bool,
) -> io::Result<()> {
    fs::create_dir_all(session_dir)?;
    fs::write(
        session_dir.join("staccato.desktop"),
        desktop_entry(session_binary, include_try_exec),
    )
}

fn install_portal_config(root: &Path, portal_dir: &Path) -> io::Result<()> {
    fs::create_dir_all(portal_dir)?;
    fs::copy(
        root.join("data/xdg-desktop-portal/staccato-portals.conf"),
        portal_dir.join("staccato-portals.conf"),
    )?;
    Ok(())
}

fn desktop_entry(session_binary: &Path, include_try_exec: bool) -> String {
    let try_exec = if include_try_exec {
        format!("TryExec={}\n", session_binary.display())
    } else {
        String::new()
    };
    format!(
        "[Desktop Entry]\nName=Staccato\nComment=Staccato Desktop Environment\nExec={} --session\n{}Type=Application\nDesktopNames=Staccato\nKeywords=wayland;desktop;session;\n",
        quoted_path(session_binary),
        try_exec
    )
}

fn quoted_path(path: &Path) -> String {
    let value = path.display().to_string();
    if value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || "-_./:+".contains(character))
    {
        return value;
    }
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn profile_dir(release: bool) -> &'static str {
    if release { "release" } else { "debug" }
}

#[cfg(unix)]
fn set_executable(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> io::Result<()> {
    Ok(())
}
