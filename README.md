# Asher

Asher is a Linux desktop environment built around the Kestrel Wayland compositor, a web-rendered panel shell, and native Rust services for session, IPC, notifications, and tray hosting.

## Workspace

```txt
crates/kestrel          Wayland compositor and render backends
crates/asher-shell      Panel shell service and Fenestra web chrome
crates/asher-session    Display-manager/session launcher
crates/asher-config     Shared config model and XDG path handling
crates/asher-ipc        Typed compositor IPC payloads
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
