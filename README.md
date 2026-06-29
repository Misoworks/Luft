# Asher

Asher is a Linux desktop environment built around the Kestrel Wayland compositor, a web-rendered shell, and native Rust services for session, IPC, notifications, tray hosting, and settings.

## Workspace

```txt
crates/kestrel          Wayland compositor and render backends
crates/asher-shell      Shell service and Fenestra web chrome
crates/asher-settings   Settings app
crates/asher-session    Display-manager/session launcher
crates/asherctl         CLI for status, config, recovery, and dev workflows
crates/asher-config     Shared config model and XDG path handling
crates/asher-ipc        Typed compositor IPC payloads
crates/asher-layout     Workspace and window layout engine
crates/asher-material   Wallpaper-derived material color helpers
crates/asher-greeter    Session discovery and PAM launch helper
```

## Build

```sh
cargo build
```

Build shell web assets with Bun before compiling `asher-shell` after UI changes:

```sh
cd crates/asher-shell/web
bun install
bun run build
```

The DRM/KMS session backend is behind an explicit feature while it is still under active development:

```sh
cargo build -p kestrel --features session-backend
```

## Run

Run the nested compositor inside an existing desktop session:

```sh
cargo run -p kestrel -- --nested
```

Kestrel prints a `WAYLAND_DISPLAY` value. Use that socket from another terminal to launch clients:

```sh
WAYLAND_DISPLAY=<printed-socket> ghostty
```

Run the protocol/headless backend for smoke tests:

```sh
ASHER_IPC_SOCKET=/tmp/asher-headless.sock cargo run -p kestrel -- --headless --socket asher-headless
```

Install a display-manager session entry for local development:

```sh
cargo run -p asherctl -- dev setup
sudo target/debug/asherctl dev install-session
```

## Checks

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Docs

- [Development](docs/development.md)
- [Running Asher](docs/running.md)
- [Configuration](docs/configuration.md)
- [Shell and Compositor Behavior](docs/shell-and-compositor.md)
- [Shortcuts](docs/shortcuts.md)

## CLI

```sh
cargo run -p asherctl -- status
cargo run -p asherctl -- status --json
cargo run -p asherctl -- config path
cargo run -p asherctl -- config validate
cargo run -p asherctl -- doctor
cargo run -p asherctl -- logs
cargo run -p asherctl -- reload
cargo run -p asherctl -- shell restart
cargo run -p asherctl -- workspace list
cargo run -p asherctl -- window list
cargo run -p asherctl -- output list
```
