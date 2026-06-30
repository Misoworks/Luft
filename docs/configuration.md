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

## Panel Pins

Pinned panel apps are stored in config:

```toml
[[panel.pinned]]
label = "Terminal"
command = "ghostty"
icon = "com.mitchellh.ghostty"
```

Set `panel.customized = true` with no `panel.pinned` entries to keep the panel app list empty.
