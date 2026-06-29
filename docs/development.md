# Development

## Dependencies

The workspace uses Rust and Bun. The shell UI is built with the shared Fenestra CEF runtime and embeds a generated single-file web bundle into `asher-shell`.

For X11 application support, install `xwayland-satellite` and `Xwayland`. Asher starts the satellite process automatically when `compositor.xwayland = true`.

The DRM/KMS backend requires libseat and graphics/input development packages. On Fedora, install `libseat-devel`, `systemd-devel`, `mesa-libgbm-devel`, `mesa-libEGL-devel`, `mesa-libGLES-devel`, `libxkbcommon-devel`, `libudev-devel`, `libinput-devel`, `xwayland-satellite`, `xorg-x11-server-Xwayland`, `xdg-desktop-portal`, `xdg-desktop-portal-gtk`, `xdg-desktop-portal-gnome`, `gnome-keyring`, and a PolicyKit agent such as `lxpolkit` or `xfce-polkit`. On Arch-based systems, install `seatd`. On Debian/Ubuntu-style systems, install `libseat-dev`.

For a complete login session, install `dbus-run-session`, `dbus-update-activation-environment`, a Secret Service provider such as `gnome-keyring-daemon`, and a PolicyKit authentication agent.

## Shell UI

Build the shell web bundle after UI changes:

```sh
cd crates/asher-shell/web
bun install
bun run build
```

The generated bundle is embedded by `asher-shell`. Settings uses the same shell bundle in settings mode.

## Dev CLI

```sh
cargo run -p asherctl -- dev setup
sudo target/debug/asherctl dev install-session
cargo run -p asherctl -- dev apply
cargo run -p asherctl -- dev watch
```

`dev setup` installs Bun dependencies, builds the shell web bundle, and builds Kestrel with the DRM/KMS session backend plus `asher-session`, `asher-shell`, `asher-settings`, and `asherctl`.

`dev install-session` installs the display-manager entry, portal preferences, and `asher-greeter` PAM policy. By default the entry points at `target/debug` binaries, so rebuilds affect the next Asher login. Pass `--copy-binaries --release` to install built binaries into `/usr/local/bin`.

`dev apply` rebuilds shell assets, rebuilds `asher-shell` and `asher-settings`, asks a live compositor to restart only the shell process, and reloads config. `dev apply kestrel` rebuilds Kestrel with `--features session-backend` and reports that the compositor must be restarted manually.

`dev watch` watches shell web UI, shell Rust code, settings code, default/user config, and Kestrel/session source groups. It applies shell and config changes through IPC; it does not restart the compositor.

## Validation

```sh
cargo fmt --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
