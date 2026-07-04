# App Compatibility Inventory

This list tracks protocol and service work needed for ordinary apps to behave like they do on mature Wayland desktops. Items here are intentionally not advertised by Kestrel until they are wired into the compositor path they require.

## Portals

- Own portal backend: screenshot, screencast, remote desktop, file chooser, wallpaper, and background app policy should grow in `luft-portal` as Luft-owned implementations.
- Settings: `luft-portal` provides `org.freedesktop.impl.portal.Settings` so toolkits and Electron apps can read appearance preferences without GNOME/KDE backends.
- Secret service: add a Luft-owned provider later; do not depend on KWallet or GNOME Keyring.
- Permission store: rely on the portal broker for now; replace only when Luft owns a complete portal backend.

## Frame Pacing

- FIFO (`wp_fifo_v1`): lets a client require one committed frame to be shown for at least one refresh before the next commit becomes ready. It needs barrier release from the real presentation/vblank path, plus a fallback for occluded or unmapped surfaces.
- Commit timing (`wp_commit_timing_v1`): lets a client request that a commit land no earlier than a target presentation-clock timestamp. It needs scheduler deadlines and wakeups tied to Kestrel's frame clock.
- Tearing control: useful later for games, but only after the DRM path can choose presentation mode per surface.

## Input And Devices

- Pointer constraints: do not advertise until pointer confinement/locking is enforced in `handle_input_event`.
- Idle inhibit: do not advertise until Kestrel tracks active inhibitors and suppresses output/session idle behavior.
- Tablet protocol: do not advertise until libinput tablet events are mapped into Smithay tablet seats.

## Window And App Integration

- XDG popup/transient handling: keep aligning coordinates, grabs, stacking, and constraints with the toplevel-rooted model used by KWin and Mutter.
- Foreign toplevel list: advertised because Kestrel publishes real toplevel handles and keeps title/app-id state in sync.
- Data control and clipboard: advertised through Smithay selection state.
- StatusNotifier tray: keep improving icon lookup and menu activation; generic window icons should not be treated as tray items.
