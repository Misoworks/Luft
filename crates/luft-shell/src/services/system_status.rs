use std::{fs, io, path::Path, process::Command};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SystemStatus {
    pub battery: Option<BatteryInfo>,
    pub network: Option<NetworkInfo>,
    pub audio: Option<AudioInfo>,
    pub brightness: Option<BrightnessInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatteryInfo {
    pub percent: u8,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkInfo {
    pub name: String,
    pub wireless: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioInfo {
    pub percent: u8,
    pub muted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrightnessInfo {
    pub percent: u8,
}

impl SystemStatus {
    pub fn read() -> Self {
        Self {
            battery: read_battery(),
            network: read_network(),
            audio: read_audio(),
            brightness: read_brightness(),
        }
    }
}

fn read_battery() -> Option<BatteryInfo> {
    let mut batteries = fs::read_dir("/sys/class/power_supply")
        .ok()?
        .filter_map(Result::ok)
        .filter(|entry| read_trimmed(entry.path().join("type")).as_deref() == Some("Battery"))
        .filter_map(|entry| battery_from_path(&entry.path()))
        .collect::<Vec<_>>();

    if batteries.is_empty() {
        return None;
    }

    let percent = batteries
        .iter()
        .map(|battery| battery.percent as u32)
        .sum::<u32>()
        / batteries.len() as u32;
    let charging = batteries
        .iter()
        .any(|battery| battery.state.eq_ignore_ascii_case("charging"));
    let discharging = batteries
        .iter()
        .any(|battery| battery.state.eq_ignore_ascii_case("discharging"));
    let state = if charging {
        "Charging".to_string()
    } else if discharging {
        "Discharging".to_string()
    } else {
        batteries
            .drain(..)
            .next()
            .map(|battery| battery.state)
            .unwrap_or_else(|| "Unknown".to_string())
    };

    Some(BatteryInfo {
        percent: percent.min(100) as u8,
        state,
    })
}

fn battery_from_path(path: &Path) -> Option<BatteryInfo> {
    Some(BatteryInfo {
        percent: read_trimmed(path.join("capacity"))?
            .parse::<u8>()
            .ok()?
            .min(100),
        state: read_trimmed(path.join("status")).unwrap_or_else(|| "Unknown".to_string()),
    })
}

fn read_network() -> Option<NetworkInfo> {
    fs::read_dir("/sys/class/net")
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| network_from_path(&entry.path()))
        .find(|network| network.wireless)
        .or_else(|| {
            fs::read_dir("/sys/class/net")
                .ok()?
                .filter_map(Result::ok)
                .filter_map(|entry| network_from_path(&entry.path()))
                .next()
        })
}

fn network_from_path(path: &Path) -> Option<NetworkInfo> {
    let name = path.file_name()?.to_string_lossy().to_string();
    if name == "lo" {
        return None;
    }

    let state = read_trimmed(path.join("operstate"))?;
    if !matches!(state.as_str(), "up" | "unknown" | "dormant") {
        return None;
    }

    Some(NetworkInfo {
        name,
        wireless: path.join("wireless").exists(),
    })
}

fn read_audio() -> Option<AudioInfo> {
    let output = Command::new("wpctl")
        .args(["get-volume", "@DEFAULT_AUDIO_SINK@"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let value = text
        .split_whitespace()
        .find_map(|part| part.parse::<f32>().ok())?;
    Some(AudioInfo {
        percent: (value * 100.0).round().clamp(0.0, 100.0) as u8,
        muted: text.contains("MUTED"),
    })
}

fn read_brightness() -> Option<BrightnessInfo> {
    fs::read_dir("/sys/class/backlight")
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| brightness_from_path(&entry.path()))
        .max_by_key(|brightness| brightness.percent)
}

fn brightness_from_path(path: &Path) -> Option<BrightnessInfo> {
    let current = read_trimmed(path.join("brightness"))?.parse::<u32>().ok()?;
    let max = read_trimmed(path.join("max_brightness"))?
        .parse::<u32>()
        .ok()?
        .max(1);
    Some(BrightnessInfo {
        percent: ((current * 100) / max).min(100) as u8,
    })
}

pub fn set_audio_volume(percent: u8) -> std::io::Result<()> {
    let value = format!("{}%", percent.min(100));
    run_command("wpctl", &["set-volume", "@DEFAULT_AUDIO_SINK@", &value])
}

pub fn toggle_audio_mute() -> std::io::Result<()> {
    run_command("wpctl", &["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"])
}

pub fn set_brightness(percent: u8) -> std::io::Result<()> {
    let value = format!("{}%", percent.min(100));
    run_command("brightnessctl", &["set", &value])
}

fn read_trimmed(path: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn run_command(command: &str, args: &[&str]) -> io::Result<()> {
    let status = Command::new(command).args(args).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("{command} exited with {status}")))
    }
}
