# Development

## Dependencies

The workspace uses Rust and Bun. The shell UI is built with the shared Fenestra CEF runtime and embeds a generated single-file web bundle into `luft-shell`.

For X11 application support, install `xwayland-satellite` and `Xwayland`. Luft starts the satellite process automatically when `compositor.xwayland = true`.

The DRM/KMS backend requires libseat and graphics/input development packages. On Fedora, install `libseat-devel`, `systemd-devel`, `mesa-libgbm-devel`, `mesa-libEGL-devel`, `mesa-libGLES-devel`, `libxkbcommon-devel`, `libudev-devel`, `libinput-devel`, `xwayland-satellite`, `xorg-x11-server-Xwayland`, `xdg-desktop-portal`, `xdg-desktop-portal-gtk`, `xdg-desktop-portal-gnome`, `gnome-keyring`, and a PolicyKit agent such as `lxpolkit` or `xfce-polkit`. On Arch-based systems, install `seatd`. On Debian/Ubuntu-style systems, install `libseat-dev`.

For a complete login session, install `dbus-run-session`, `dbus-update-activation-environment`, a Secret Service provider such as `gnome-keyring-daemon`, and a PolicyKit authentication agent.

## Shell UI

Build the shell web bundle after UI changes:

```sh
cd crates/luft-shell/web
bun install
bun run build
```

The generated bundle is embedded by `luft-shell`.

## Validation

```sh
cargo fmt --check
cargo check --workspace
cargo check -p kestrel --features session-backend
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
