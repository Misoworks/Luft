# Running Luft

## Nested

```sh
cargo run -p kestrel -- --nested
```

Kestrel prints the Wayland socket name. Launch clients against it from another terminal:

```sh
WAYLAND_DISPLAY=<printed-socket> ghostty
WAYLAND_DISPLAY=<printed-socket> wayland-info
```

When run directly for development, Kestrel wraps `luft-shell` in a private D-Bus session when possible. Set `LUFT_USE_HOST_DBUS=1` only when intentionally debugging against the host session bus.

## Headless

```sh
LUFT_IPC_SOCKET=/tmp/luft-headless.sock cargo run -p kestrel -- --headless --socket luft-headless
```

The headless backend binds a Wayland socket and runs compositor protocol, layout, frame-callback, and IPC loops without opening a host window or starting the shell.

```sh
WAYLAND_DISPLAY=luft-headless wayland-info
```

## Session Launcher

`luft-session` is the display-manager entry point from `data/sessions/luft.desktop`. The installed entry launches `luft-session --session`, sets Luft desktop environment variables, and starts Kestrel as a real Wayland session.

## Install A Login Session

Run the installer from the repository root:

```sh
./install.sh
```

It builds the shell web assets with Bun, builds the session binaries with the DRM/KMS backend enabled, refreshes the Fenestra CEF host, installs the binaries to `/usr/local/bin`, writes the Wayland session entry, and installs Luft's portal preference file.

Override install paths or build a debug profile when needed:

```sh
PROFILE=debug ./install.sh
BIN_DIR="$HOME/.local/bin" SESSION_DIR="$HOME/.local/share/wayland-sessions" PORTAL_DIR="$HOME/.local/share/xdg-desktop-portal" ./install.sh
```

After that, pick Luft from the display manager's session menu.

When run manually without an explicit backend, `luft-session` defaults to nested inside an existing Wayland session and to the session backend outside one. When `dbus-run-session` is available, the session runs Kestrel under a private D-Bus session so shell services and launched apps do not attach to the host desktop session while testing nested.

```sh
cargo run -p luft-session -- --nested --socket luft-dev
cargo run -p luft-session -- --desktop-entry
cargo run -p luft-session -- --session --dry-run
```
