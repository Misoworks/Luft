# Staccato

Staccato is a Linux desktop environment built around a custom Wayland compositor, Baton, and a modular shell.

This repository currently contains the initial Rust workspace and the first nested Baton prototype from `staccato.md`.

## Build

```sh
cargo build
```

The default shell UI uses WebKitGTK for Svelte/TypeScript/Tailwind chrome on real layer-shell surfaces. On Arch-based systems the native packages are:

```sh
sudo pacman -S gtk-layer-shell webkit2gtk-4.1 gtk3 pkgconf
```

For X11 application support, install `xwayland-satellite` and `Xwayland` from the distribution package manager. Staccato starts satellite automatically when `compositor.xwayland = true`.

The real DRM/KMS session backend is built behind an explicit feature while it is under active development:

```sh
cargo build -p baton --features session-backend
```

That feature requires libseat development files. On Arch-based systems install `seatd`; on Debian/Ubuntu-style systems install `libseat-dev`.

Build the web shell assets with Bun before compiling `staccato-shell` when the web UI has changed. The build emits a single HTML file that is embedded into the shell binary:

```sh
cd crates/staccato-shell/web
bun install
bun run build
```

## Test

```sh
cargo test
```

## Run Baton Nested

```sh
cargo run -p baton -- --nested
```

Baton prints the `WAYLAND_DISPLAY` socket name. Launch a Wayland client against that socket from another terminal:

```sh
WAYLAND_DISPLAY=<printed-socket> ghostty
```

The nested prototype accepts xdg-shell toplevel clients, advertises a `wl_output`, supports `wlr-layer-shell` shell surfaces, starts and restarts `staccato-shell` when the shell binary is available, starts `xwayland-satellite` for X11 apps when configured and installed, draws `/home/kristof/Pictures/bg.jpg` as the default compositor background, draws active-workspace windows in a nested window, forwards keyboard and pointer input, supports workspace slide transitions, respects xdg-decoration client-side and server-side mode requests, supports normal clipboard focus, primary selection, xdg activation, xdg toplevel icons, named cursor-shape requests, viewporter, fractional-scale preferences, presentation-time feedback, text-input v3 focus tracking, and `ext-background-effect-v1`, and draws compositor-side blurred titlebars with right-side traffic-light controls only for server-decorated windows. Layer-shell surfaces are arranged by layer order around normal application windows: background, bottom, app windows, top, then overlay. Baton reports the shell process, XWayland status, active workspace/profile, and live effect state through IPC, shows a wallpaper-backed loading progress overlay until the top panel layer is ready, cleans stale shell-control sockets, and stops its child processes when the compositor exits. If the shell crashes repeatedly inside the configured recovery window, Baton restarts it in runtime safe mode with expensive effects disabled.

The shell uses a WebKitGTK web UI for the user-facing panel, dock, sidebar, overview, quick settings, and notification/date center. Rust still owns Wayland IPC, workspace/window actions, tray hosting, notifications, app launching, config reloads, and surface lifetime; the web layer renders the chrome and sends typed actions back to Rust. The user-facing shell chrome is web-only, so there is no separate native fallback UI to keep in sync.

The built-in default starts in a panel workspace with a bottom taskbar; sidebar chrome is available through workspace profiles instead of being the first-run default. Workspaces switch from panel, dock, or sidebar vertical scroll, `Super+Left/Right`, `Super+Up/Down`, `Super+scroll`, or direct numeric shortcuts. Baton keeps one trailing empty dynamic workspace once windows exist and does not keep creating empty workspaces when scrolling past it. Pressing and releasing `Super` opens the web overview with real workspace previews, active windows, searchable discovered desktop apps, workspace profiles, and command results for launcher, quick settings, notifications, shell mode, settings pages, do-not-disturb, diagnostics, config reload, logs, and safe mode. The clock opens a notification and calendar center. The panel status area opens quick settings, renders live network, audio, power, volume, and brightness controls when those host services are available, and hides unavailable controls instead of showing disabled placeholders. StatusNotifier/AppIndicator tray items registered on the session bus are hosted in the panel; tray icons come from the item icon theme name when available, left click activates the item, and right click asks the item to open its context menu. The shell owns `org.freedesktop.Notifications`, supports body text, static icons, action buttons, replacement IDs, timeouts, close requests, and emits `NotificationClosed` and `ActionInvoked` signals for app notifications. Baton inserts downsampled rounded blurred wallpaper material under the dock, sidebar, and popover shell surfaces when blur is enabled; full-width panel and overview surfaces use normal translucent material to keep frame cost low. Wayland apps that draw their own header bars keep them by default; Baton draws its traffic-light frame only for clients that explicitly request server-side decorations. The panel and dock tint their material from the configured wallpaper, and the dock/taskbar render pinned apps as real icon-theme images with hover lift and running/active indicators. Clicking a pinned dock or taskbar app focuses or restores its matching running window before launching a new process; clicking its active visible window minimizes it. Right-clicking an overview app pins or unpins it, and right-clicking a dock item unpins it. Chromium-family browsers launched by the shell use a Staccato-specific profile under `${XDG_STATE_HOME:-$HOME/.local/state}/staccato` and force Wayland Ozone, so nested sessions do not hand off browser windows to an existing host desktop browser process.

To inspect the advertised Wayland globals:

```sh
WAYLAND_DISPLAY=<printed-socket> wayland-info
```

## Run Baton Headless

```sh
STACCATO_IPC_SOCKET=/tmp/staccato-headless.sock cargo run -p baton -- --headless --socket staccato-headless
```

The headless backend binds a Wayland socket and runs the compositor protocol, layout, frame-callback, and IPC loops without opening a host window or starting the shell. `staccatoctl status` reports `Shell: NotStarted` for this backend. It is intended for protocol and automation smoke tests:

```sh
WAYLAND_DISPLAY=staccato-headless wayland-info
STACCATO_IPC_SOCKET=/tmp/staccato-headless.sock staccatoctl status
```

## Session Launcher

`staccato-session` is the display-manager entry point from `data/sessions/staccato.desktop`.
The installed desktop entry launches `staccato-session --session`, sets the Staccato desktop environment variables, and starts Baton as a real Wayland session. When run manually without an explicit backend, `staccato-session` defaults to nested inside an existing Wayland session and to the session backend outside one. When `dbus-run-session` is available, the session runs Baton under a private D-Bus session so shell services and launched apps do not attach to the host desktop session while testing nested. If Baton is started directly for development, it wraps `staccato-shell` in its own private D-Bus session when possible. Set `STACCATO_USE_HOST_DBUS=1` only when intentionally debugging against the host session bus.

```sh
cargo run -p staccato-session -- --nested --socket staccato-dev
cargo run -p staccato-session -- --desktop-entry
cargo run -p staccato-session -- --session --dry-run
```

The DRM/KMS session backend has a libseat/udev hardware probe behind `baton --features session-backend` and now selects connected DRM outputs, modes, and CRTCs before the render loop is initialized. Modeset rendering still depends on Baton session-backend implementation work. The installed session entry has a real launcher binary.

Current compositor shortcuts:

```txt
Super+1..9          Switch workspace by numeric workspace id
Super+Left/Right    Switch to the previous or next workspace
Super+Up/Down       Switch to the previous or next workspace
Super+scroll        Switch to the previous or next workspace
Super               Open the window overview when released by itself
Super+Space         Open the configured launcher, Vicinae by default
Super+Return        Open the configured terminal, Ghostty by default
Super+E             Open the configured file manager
Escape              Close the active overview, quick settings, date center, or menu surface
Super+Shift+1..9    Move the active window to a workspace
Super+Shift+R       Restart Staccato Shell without ending the compositor session
Super+Shift+Backspace
                    Ignore user config and reload the built-in default profile
Alt+Tab             Cycle windows in the active workspace
Alt+Shift+Tab       Cycle windows in reverse
Super+Tab           Cycle windows in the active workspace
Super+Shift+Tab     Cycle windows in reverse
Super+Q             Close the active window
F3                  Toggle Baton debug overlay
F4                  Toggle active workspace between dock and panel style
```

Window titlebars can be dragged, resized from edges/corners with matching cursor feedback, closed, minimized, and maximized with the right-side traffic-light controls. Normal app windows animate when opened, restored, and minimized when animations are enabled.

## CLI

```sh
cargo run -p staccatoctl -- status
cargo run -p staccatoctl -- status --json
cargo run -p staccatoctl -- config path
cargo run -p staccatoctl -- config validate
cargo run -p staccatoctl -- logs
cargo run -p staccatoctl -- doctor
cargo run -p staccatoctl -- recovery status
cargo run -p staccatoctl -- recovery backups
cargo run -p staccatoctl -- recovery rollback
cargo run -p staccatoctl -- recovery defaults
cargo run -p staccatoctl -- reload
cargo run -p staccatoctl -- effects blur off
cargo run -p staccatoctl -- debug overlay on
cargo run -p staccatoctl -- safe-mode set on
cargo run -p staccatoctl -- shell restart
cargo run -p staccatoctl -- profile list
cargo run -p staccatoctl -- workspace list
cargo run -p staccatoctl -- workspace switch 2
cargo run -p staccatoctl -- workspace profile 2 browser-dev
cargo run -p staccatoctl -- workspace style dock
cargo run -p staccatoctl -- workspace style panel
cargo run -p staccatoctl -- window list
cargo run -p staccatoctl -- window focus 1
cargo run -p staccatoctl -- window move 1 2
cargo run -p staccatoctl -- window minimize 1
cargo run -p staccatoctl -- window maximize 1
cargo run -p staccatoctl -- window close 1
```

Configuration is loaded from `~/.config/staccato/config.toml` when present and falls back to built-in defaults.
At startup, Baton, `staccato-session`, and `staccato-shell` fall back to defaults if the user config cannot be parsed or validated, so a broken config does not prevent the session from starting. `staccatoctl config validate` and live reload remain strict.
When Baton is running, `staccatoctl status`, workspace commands, profile commands, window commands, config reload, and live setting toggles use the live IPC socket.
When `recovery.backup_before_apply` is enabled, config writes create timestamped backups under `~/.config/staccato/backups`. `staccatoctl recovery rollback` restores the latest backup and asks a running Baton instance to reload.

The background image can be overridden with `compositor.background_image` in the config file. Set it to `null` to fall back to the solid compositor clear color.

Dock pins can be edited without hand-writing config:

```sh
cargo run -p staccatoctl -- dock list
cargo run -p staccatoctl -- dock pin google-chrome-stable --label Browser --icon google-chrome
cargo run -p staccatoctl -- dock unpin Browser
```

In the web overview, right-clicking an app pins or unpins it. Right-clicking a dock item unpins it. The first dock customization materializes the built-in defaults into the user config, so removing the last pin leaves an intentionally empty dock instead of silently restoring the defaults.

The same pins are stored as `dock.pinned` entries:

```toml
[[dock.pinned]]
label = "Terminal"
command = "ghostty"
icon = "com.mitchellh.ghostty"
```

Set `dock.customized = true` with no `dock.pinned` entries to keep the dock empty.
