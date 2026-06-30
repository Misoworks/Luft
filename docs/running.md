# Running Asher

## Nested

```sh
cargo run -p kestrel -- --nested
```

Kestrel prints the Wayland socket name. Launch clients against it from another terminal:

```sh
WAYLAND_DISPLAY=<printed-socket> ghostty
WAYLAND_DISPLAY=<printed-socket> wayland-info
```

When run directly for development, Kestrel wraps `asher-shell` in a private D-Bus session when possible. Set `ASHER_USE_HOST_DBUS=1` only when intentionally debugging against the host session bus.

## Headless

```sh
ASHER_IPC_SOCKET=/tmp/asher-headless.sock cargo run -p kestrel -- --headless --socket asher-headless
```

The headless backend binds a Wayland socket and runs compositor protocol, layout, frame-callback, and IPC loops without opening a host window or starting the shell.

```sh
WAYLAND_DISPLAY=asher-headless wayland-info
```

## Session Launcher

`asher-session` is the display-manager entry point from `data/sessions/asher.desktop`. The installed entry launches `asher-session --session --guard`, sets Asher desktop environment variables, and starts Kestrel as a real Wayland session.

When run manually without an explicit backend, `asher-session` defaults to nested inside an existing Wayland session and to the session backend outside one. When `dbus-run-session` is available, the session runs Kestrel under a private D-Bus session so shell services and launched apps do not attach to the host desktop session while testing nested.

```sh
cargo run -p asher-session -- --nested --socket asher-dev
cargo run -p asher-session -- --desktop-entry
cargo run -p asher-session -- --session --dry-run
```

The guarded session returns to the display manager after an early Kestrel crash. Set `ASHER_FALLBACK_SESSION` or pass `--fallback-session` to launch another desktop after startup failure.

## Greeter Helper

```sh
cargo run -p asher-greeter -- list
cargo run -p asher-greeter -- launch asher --dry-run
cargo run -p asher-greeter -- auth-launch kristof asher --password-stdin --dry-run
```

`asher-greeter` discovers installed `.desktop` sessions. `auth-launch` authenticates through PAM, opens a session, drops to the selected user, starts the selected desktop entry, waits for it to exit, and closes the PAM session.
