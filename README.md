# Luft

Luft is a Linux desktop environment built around the Kestrel Wayland compositor, a web-rendered panel shell, and native Rust services for session, IPC, notifications, and tray hosting.

## Workspace

```txt
crates/kestrel          Wayland compositor and render backends
crates/luft-shell      Panel shell service and Fenestra web chrome
crates/luft-session    Display-manager/session launcher
crates/luft-config     Shared config model and XDG path handling
crates/luft-ipc        Typed compositor IPC payloads
```

## Build

```sh
cargo build
```

Build shell web assets with Bun before compiling `luft-shell` after UI changes:

```sh
cd crates/luft-shell/web
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
LUFT_IPC_SOCKET=/tmp/luft-headless.sock cargo run -p kestrel -- --headless --socket luft-headless
```

## Checks

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Docs

- [Development](docs/development.md)
- [Running Luft](docs/running.md)
- [Configuration](docs/configuration.md)
- [Shell and Compositor Behavior](docs/shell-and-compositor.md)
- [App Compatibility Inventory](docs/app-compatibility-inventory.md)
- [Shortcuts](docs/shortcuts.md)
