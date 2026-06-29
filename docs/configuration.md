# Configuration

Asher loads `~/.config/asher/config.toml` when present and falls back to built-in defaults. At startup, Kestrel, `asher-session`, and `asher-shell` fall back to defaults if user config cannot be parsed or validated, so a broken config does not prevent the session from starting. `asherctl config validate` and live reload remain strict.

```sh
cargo run -p asherctl -- config path
cargo run -p asherctl -- config validate
cargo run -p asherctl -- reload
```

When Kestrel is running, `asherctl status`, workspace commands, profile commands, window commands, config reload, and live setting toggles use the live IPC socket.

When `recovery.backup_before_apply` is enabled, config writes create timestamped backups under `~/.config/asher/backups`. `asherctl recovery rollback` restores the latest backup and asks a running Kestrel instance to reload.

## Appearance

```toml
[appearance]
material_mode = "glass"
shell_mode = "panel"
dock_icon_size = 40
dock_magnification = true
taskbar_launcher = true
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

Output scale can also be changed live:

```sh
cargo run -p asherctl -- output list
cargo run -p asherctl -- output scale 1.25
```

## Dock Pins

```sh
cargo run -p asherctl -- dock list
cargo run -p asherctl -- dock pin google-chrome-stable --label Browser --icon google-chrome
cargo run -p asherctl -- dock unpin Browser
```

The same pins are stored in config:

```toml
[[dock.pinned]]
label = "Terminal"
command = "ghostty"
icon = "com.mitchellh.ghostty"
```

The first dock customization materializes built-in defaults into user config. Set `dock.customized = true` with no `dock.pinned` entries to keep the dock empty.

## Settings

Open Settings from quick settings, Start menu search, or directly:

```sh
asher-settings
```

Settings writes the same config file used by Kestrel and can change shell mode, glass effects, animation, performance mode, dock icon size, dock hover lift, Start menu button visibility, pinned app order, workspace startup behavior, wallpaper, display scale, compositor backend, XWayland, default apps, session commands, and recovery behavior.
