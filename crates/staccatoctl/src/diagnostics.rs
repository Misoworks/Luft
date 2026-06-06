use crate::doctor::{Severity, doctor_checks};
use serde::Serialize;
use staccato_config::{
    ConfigPaths, ConfigSource, StaccatoConfig, list_config_backups, load_config,
    save_config_to_path,
};
use staccato_ipc::{IpcRequest, IpcResponse, ShellStatus, send_request};
use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::Command,
};

const DEFAULT_LOG_LINES: usize = 80;
const COMPONENTS: [&str; 3] = ["baton", "staccato-shell", "staccato-session"];

pub fn print_logs(
    component: Option<String>,
    lines: Option<usize>,
    path_only: bool,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let paths = ConfigPaths::discover()?;
    let components = selected_components(component)?;
    let logs = components
        .iter()
        .map(|component| log_snapshot(&paths, component, lines.unwrap_or(DEFAULT_LOG_LINES)))
        .collect::<Vec<_>>();

    if json {
        println!("{}", serde_json::to_string_pretty(&logs)?);
        return Ok(());
    }

    for (index, log) in logs.iter().enumerate() {
        if index > 0 {
            println!();
        }
        if path_only {
            println!("{}", log.path.display());
            continue;
        }
        println!("{} {}", log.component, log.path.display());
        if log.lines.is_empty() {
            println!("No log entries");
        } else {
            for line in &log.lines {
                println!("{line}");
            }
        }
    }
    Ok(())
}

pub fn print_config_path(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let paths = ConfigPaths::discover()?;
    if json {
        println!(
            "{}",
            serde_json::json!({
                "config_home": paths.config_home,
                "config_file": paths.config_file,
                "profiles_dir": paths.profiles_dir,
                "materials_dir": paths.materials_dir,
                "state_home": paths.state_home,
                "cache_home": paths.cache_home,
                "logs_dir": paths.logs_dir(),
            })
        );
    } else {
        println!("{}", paths.config_file.display());
    }

    Ok(())
}

pub fn validate_config(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let loaded = load_config()?;
    if json {
        println!(
            "{}",
            serde_json::json!({
                "valid": true,
                "source": match loaded.source {
                    ConfigSource::User(path) => path.display().to_string(),
                    ConfigSource::Defaults => "built-in defaults".to_string(),
                }
            })
        );
    } else {
        println!("Config is valid");
    }
    Ok(())
}

pub fn open_config(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let paths = ConfigPaths::discover()?;
    if !paths.config_file.exists() {
        save_config_to_path(&paths.config_file, &StaccatoConfig::default())?;
    }
    let opener = config_opener();
    let status = Command::new("sh")
        .arg("-lc")
        .arg("exec ${VISUAL:-${EDITOR:-xdg-open}} \"$1\"")
        .arg("staccatoctl-open-config")
        .arg(&paths.config_file)
        .status()?;
    if !status.success() {
        return Err(format!("{opener} exited with {status}").into());
    }
    if json {
        println!(
            "{}",
            serde_json::json!({
                "configFile": paths.config_file,
                "opener": opener,
            })
        );
    } else {
        println!("{}", paths.config_file.display());
    }
    Ok(())
}

pub fn print_doctor(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let checks = doctor_checks();
    if json {
        println!("{}", serde_json::to_string_pretty(&checks)?);
    } else {
        for check in &checks {
            println!(
                "[{}] {}\t{}",
                check.severity.label(),
                check.name,
                check.message
            );
        }
    }

    if checks
        .iter()
        .any(|check| matches!(check.severity, Severity::Fail))
    {
        return Err("doctor found blocking problems".into());
    }
    Ok(())
}

pub fn print_recovery_status(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let status = recovery_status()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&status)?);
    } else {
        println!("Safe mode: {}", status.safe_mode);
        println!("Shell: {:?}", status.shell);
        println!("Crash limit: {}", status.crash_limit);
        println!("Crash window: {}s", status.crash_window_seconds);
        println!("Auto safe mode: {}", status.auto_safe_mode);
        println!("Backups before apply: {}", status.backup_before_apply);
        println!("Config backups: {}", status.config_backups);
        println!(
            "Recent shell crashes in logs: {}",
            status.recent_shell_crashes
        );
        println!("Baton log: {}", status.baton_log.display());
        println!("Shell log: {}", status.shell_log.display());
    }
    Ok(())
}

fn selected_components(
    component: Option<String>,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let Some(component) = component else {
        return Ok(COMPONENTS
            .iter()
            .map(|component| component.to_string())
            .collect());
    };
    let component = component.trim();
    if COMPONENTS.contains(&component) {
        return Ok(vec![component.to_string()]);
    }
    Err(format!(
        "unknown log component {component:?}; expected one of {}",
        COMPONENTS.join(", ")
    )
    .into())
}

fn config_opener() -> String {
    env::var("VISUAL")
        .ok()
        .or_else(|| env::var("EDITOR").ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "xdg-open".to_string())
}

fn log_snapshot(paths: &ConfigPaths, component: &str, lines: usize) -> LogSnapshot {
    let path = paths.log_file(component);
    let content = fs::read_to_string(&path).unwrap_or_default();
    let lines = tail_lines(&content, lines);
    LogSnapshot {
        component: component.to_string(),
        path,
        lines,
    }
}

fn recovery_status() -> Result<RecoveryStatus, Box<dyn std::error::Error>> {
    let loaded = load_config()?;
    let paths = ConfigPaths::discover()?;
    let live = match send_request(&IpcRequest::Status) {
        Ok(IpcResponse::Status(status)) => Some(status),
        _ => None,
    };
    let safe_mode = live
        .as_ref()
        .map_or(loaded.config.general.safe_mode, |status| status.safe_mode);
    let shell = live
        .as_ref()
        .map_or(ShellStatus::NotStarted, |status| status.shell);
    let baton_log = paths.log_file("baton");
    let shell_log = paths.log_file("staccato-shell");
    let recent_shell_crashes = count_log_matches(&baton_log, "staccato shell exited")?
        + count_log_matches(&baton_log, "shell crash limit reached")?;

    Ok(RecoveryStatus {
        safe_mode,
        shell,
        crash_limit: loaded.config.recovery.crash_limit,
        crash_window_seconds: loaded.config.recovery.crash_window_seconds,
        auto_safe_mode: loaded.config.recovery.auto_safe_mode,
        backup_before_apply: loaded.config.recovery.backup_before_apply,
        config_backups: list_config_backups().map_or(0, |backups| backups.len()),
        recent_shell_crashes,
        baton_log,
        shell_log,
    })
}

fn count_log_matches(path: &Path, needle: &str) -> io::Result<usize> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(content.lines().filter(|line| line.contains(needle)).count()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(0),
        Err(error) => Err(error),
    }
}

fn tail_lines(content: &str, lines: usize) -> Vec<String> {
    if lines == 0 {
        return Vec::new();
    }
    let mut output = content
        .lines()
        .rev()
        .take(lines)
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    output.reverse();
    output
}

#[derive(Debug, Serialize)]
struct LogSnapshot {
    component: String,
    path: PathBuf,
    lines: Vec<String>,
}

#[derive(Debug, Serialize)]
struct RecoveryStatus {
    safe_mode: bool,
    shell: ShellStatus,
    crash_limit: u32,
    crash_window_seconds: u64,
    auto_safe_mode: bool,
    backup_before_apply: bool,
    config_backups: usize,
    recent_shell_crashes: usize,
    baton_log: PathBuf,
    shell_log: PathBuf,
}
