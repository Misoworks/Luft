#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="${BIN_DIR:-/usr/local/bin}"
SESSION_DIR="${SESSION_DIR:-/usr/share/wayland-sessions}"
PORTAL_DIR="${PORTAL_DIR:-/usr/share/xdg-desktop-portal}"
PROFILE="${PROFILE:-release}"

if [[ "$PROFILE" != "release" && "$PROFILE" != "debug" ]]; then
  echo "PROFILE must be release or debug" >&2
  exit 1
fi

if ! command -v bun >/dev/null 2>&1; then
  echo "bun is required to build luft-shell web assets" >&2
  exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required to build Luft" >&2
  exit 1
fi

SUDO=()
if [[ "${EUID:-$(id -u)}" -ne 0 ]]; then
  SUDO=(sudo)
fi

build_args=()
target_dir="$ROOT/target/debug"
if [[ "$PROFILE" == "release" ]]; then
  build_args+=(--release)
  target_dir="$ROOT/target/release"
fi

cd "$ROOT/crates/luft-shell/web"
bun install --frozen-lockfile
bun run build

cd "$ROOT"
cargo build "${build_args[@]}" \
  -p kestrel \
  -p luft-shell \
  -p luft-session \
  --features kestrel/session-backend

"$target_dir/luft-shell" --refresh-fenestra-host

"${SUDO[@]}" install -Dm755 "$target_dir/kestrel" "$BIN_DIR/kestrel"
"${SUDO[@]}" install -Dm755 "$target_dir/luft-shell" "$BIN_DIR/luft-shell"
"${SUDO[@]}" install -Dm755 "$target_dir/luft-session" "$BIN_DIR/luft-session"

desktop_entry="$(mktemp)"
trap 'rm -f "$desktop_entry"' EXIT
cat >"$desktop_entry" <<EOF
[Desktop Entry]
Name=Luft
Comment=Luft Desktop Environment
Exec=$BIN_DIR/luft-session --session
TryExec=$BIN_DIR/luft-session
Type=Application
DesktopNames=Luft
Keywords=wayland;desktop;session;
EOF

"${SUDO[@]}" install -Dm644 "$desktop_entry" "$SESSION_DIR/luft.desktop"
"${SUDO[@]}" install -Dm644 \
  "$ROOT/data/xdg-desktop-portal/luft-portals.conf" \
  "$PORTAL_DIR/luft-portals.conf"

echo "Installed Luft session:"
echo "  binaries: $BIN_DIR"
echo "  session:  $SESSION_DIR/luft.desktop"
echo "  portals:  $PORTAL_DIR/luft-portals.conf"
echo
echo "Pick Luft from your display manager's session menu."
