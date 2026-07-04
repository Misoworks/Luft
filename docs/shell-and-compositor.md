# Shell and Compositor Behavior

## Kestrel

Kestrel accepts xdg-shell toplevel clients, advertises `wl_output`, supports `wlr-layer-shell`, starts and restarts `luft-shell`, starts `xwayland-satellite` for X11 apps when configured and installed, forwards keyboard and pointer input, supports workspace slide transitions, and respects xdg-decoration client-side and server-side mode requests.

It also supports normal clipboard focus, primary selection, xdg activation, xdg toplevel icons, named cursor-shape requests, viewporter, fractional-scale preferences, presentation-time feedback, text-input v3 focus tracking, and `ext-background-effect-v1`.

Layer-shell surfaces are arranged in this order: background, bottom, app windows, top, overlay. Kestrel reports shell process state, XWayland status, and active workspace through IPC. It shows a wallpaper-backed loading overlay until the panel layer is ready, cleans stale shell-control sockets, and stops child processes on compositor exit.

## DRM/KMS Backend

The session backend opens the active seat through libseat, selects the primary DRM card with udev, discovers connected outputs, creates GBM/KMS surfaces, renders the full scene on the primary scanout, queues a compositor-clear frame on secondary scanouts, forwards libinput events, starts shell services, starts XWayland satellite, advertises linux-dmabuf formats accepted by the renderer, and refreshes outputs when udev reports connector changes.

Fullscreen Wayland clients can be scanned out directly on the primary plane when there are no visible shell/effect layers and KMS accepts the client framebuffer. Otherwise Kestrel falls back to normal composition.

Install `data/xdg-desktop-portal/luft-portals.conf` to `/usr/share/xdg-desktop-portal/luft-portals.conf` or `/etc/xdg/xdg-desktop-portal/luft-portals.conf` so the portal broker uses Luft's explicit backend preferences instead of a desktop-specific fallback.

Viewport-aware scene rendering on non-primary outputs and cursor/overlay plane assignment are the remaining KMS work.

## Shell

The shell uses a Fenestra web UI for the full-width panel, Start menu, quick settings, notification/date center, notification toast, and panel app context menu. Rust owns Wayland IPC, workspace/window actions, tray hosting, notifications, app launching, session commands, config reloads, and surface lifetime. The web layer renders chrome and sends typed actions back to Rust.

Hidden shell popovers and the Start menu are launched lazily and evicted after a short idle period to keep resident memory down. Set `LUFT_SHELL_PREWARM=1` while developing when first-open latency matters more than startup memory.

The built-in default starts with a bottom full-width panel. Kestrel keeps one trailing empty dynamic workspace once windows exist and does not keep creating empty workspaces when scrolling past it.

The Start menu searches discovered desktop apps, workspaces, and shell commands for launcher, quick settings, notifications, settings pages, do-not-disturb, session commands, config reload, and logs. The clock opens notification and calendar center.

The panel status area opens quick settings and renders network, audio, power, volume, brightness, do-not-disturb, launcher, Start menu, notifications, settings, and session controls when backing services are available. Unavailable hardware controls are hidden instead of shown as disabled placeholders.

StatusNotifier/AppIndicator tray items registered on the session bus are hosted in the panel. Tray icons come from the item icon theme name when available. Left click activates the item; right click asks the item to open its context menu.

The shell owns `org.freedesktop.Notifications`, supports body text, static icons, action buttons, replacement IDs, timeouts, close requests, toast default actions, do-not-disturb suppression for non-critical popups, and `NotificationClosed`/`ActionInvoked` signals.

Kestrel provides live blur for panel popovers. The full-width panel uses normal translucent material to keep frame cost low.

Wayland apps that draw their own header bars keep them by default. Kestrel draws traffic-light frames only for clients that request server-side decorations. Window titlebars can be dragged, resized from edges/corners with matching cursor feedback, closed, minimized, and maximized.

The panel renders pinned apps as icon-theme images with running/active indicators. Clicking a pinned app focuses or restores its matching running window before launching a new process; clicking its active visible window minimizes it. Pinned apps can be reordered by dragging, and right-click actions pin or unpin apps.
