# Configuration

Asher loads `~/.config/asher/config.toml` when present and falls back to built-in defaults. At startup, Kestrel, `asher-session`, and `asher-shell` fall back to defaults if user config cannot be parsed or validated, so a broken config does not prevent the session from starting.

## Appearance

```toml
[appearance]
animations = true
panel_icon_size = 40
panel_magnification = false
panel_launcher = true
```

## Wallpaper

```toml
[compositor]
background_image = "/home/kristof/Pictures/bg.jpg"
```

Set `background_image = null` to use the solid compositor clear color.

## Display

Display scale can be configured globally or per connector:

```toml
[display]
default_scale = 1.0

[display."eDP-1"]
scale = 1.25
```

Asher picks the largest available mode at the highest refresh rate by default. Pin a mode when needed:

```toml
[display."DP-1"]
width = 3440
height = 1440
refresh_millihertz = 165000
```

## Startup Apps

Asher launches user desktop entries from `~/.config/autostart` once when the shell starts. Add explicit commands when you want startup apps that are not represented by desktop files:

```toml
[session]
startup_apps = ["rover", "ghostty"]
```

## Panel Pins

Pinned panel apps are stored in config:

```toml
[[panel.pinned]]
label = "Terminal"
command = "ghostty"
icon = "com.mitchellh.ghostty"
```

Set `panel.customized = true` with no `panel.pinned` entries to keep the panel app list empty.
