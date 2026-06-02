# Staccato Desktop Environment Specification

## 0. Purpose

Staccato is a standalone Linux desktop environment built around a custom Wayland compositor and a modular shell.

It is not a distro. It must be usable on existing Linux distributions such as Fedora, Arch, openSUSE, NixOS, Debian, and other modern Linux systems.

The goal is to create a beautiful, lightweight, deeply flexible desktop environment with first-class support for:

- A custom Wayland compositor.
- Native compositor-level blur and background materials.
- A modular shell.
- Multiple drastic workflow modes.
- Per-workspace shell profiles.
- Browser-like workspace/tab/split workflows.
- Safe user customization through files, CLI, and future settings UI.
- Future AI-assisted customization through structured profiles, extensions, and patches.
- Strong recovery and safe-mode behavior.
- Practical nested testing while developing.

Staccato must be useful as a desktop environment by itself. Apps such as file manager, terminal, app store, browser, notes, email, etc. are out of scope and will be built separately or provided by the host distribution.

The DE must still integrate cleanly with normal Linux desktop apps.

---

## 1. Naming

### 1.1 Project Names

- **Staccato**: the desktop environment.
- **Baton**: the Wayland compositor.
- **Staccato Shell**: the visible desktop shell UI.
- **Luca**: dynamic glass/blur material.
- **Maris**: softer mica-like material.
- **Shell Profiles**: reusable mode/layout/style/workflow configurations.
- **Modes**: major interaction models such as Dock Mode, Panel Mode, Browser Mode, Tiling Mode, Focus Mode, etc.

### 1.2 Naming Stack

```txt
Staccato Desktop Environment
├── Baton Compositor
├── Staccato Shell
├── Staccato Layout Engine
├── Staccato Profiles
├── Compositor Material System
│   ├── Luca
│   └── Maris
├── staccatoctl
└── Staccato configuration files
```

---

## 2. Product Thesis

Staccato is not just a pretty shell.

Staccato is a desktop environment where each workspace can have a different workflow model.

A traditional DE usually asks:

```txt
Where should the panel be?
What theme do you want?
How big should the dock be?
```

Staccato should ask:

```txt
What kind of workspace is this?
```

Examples:

- Workspace 1: Browser-like dev workspace with sidebar tabs and splits.
- Workspace 2: Dock-based casual workspace.
- Workspace 3: Keyboard-first tiling workspace.
- Workspace 4: Focus workspace with minimal chrome.
- Workspace 5: Classic panel workspace.

The desktop should be beautiful and usable by default, but flexible enough that the shell can reshape itself per workflow.

---

## 3. Core Requirements

### 3.1 Must-Have Goals

Staccato must:

1. Run as a Wayland desktop environment.
2. Use a custom compositor called Baton.
3. Support normal Wayland clients.
4. Support XWayland clients.
5. Run nested inside an existing Wayland session for development/testing.
6. Run as a real login session through a display manager or TTY launch.
7. Provide a visible shell with panel/dock/sidebar/launcher/workspace UI.
8. Support compositor-level blur.
9. Support `ext-background-effect-v1` where possible.
10. Provide internal material primitives for Staccato Shell surfaces.
11. Support per-workspace Shell Profiles.
12. Support multiple workflow modes.
13. Provide a CLI called `staccatoctl`.
14. Store user configuration in editable files.
15. Support live reload for non-dangerous configuration changes.
16. Provide safe-mode and rollback behavior.
17. Be portable across distributions.
18. Avoid hard dependency on a custom distro.
19. Avoid requiring a custom app suite.
20. Be testable incrementally.

### 3.2 Non-Goals

These are out of scope for the DE itself:

- Custom file manager.
- Custom terminal.
- Custom browser.
- Custom app store.
- Full distro installer.
- Full Glacier OS image.
- Office apps.
- Email client.
- Media player.
- Notes app.
- Chat app.

Staccato may provide integration surfaces for these later, but it must not require them.

---

## 4. Recommended Technical Stack

### 4.1 Language

Primary language:

```txt
Rust
```

Rust is preferred for:

- Memory safety.
- Good systems programming fit.
- Existing user preference and ecosystem alignment.
- Smithay compatibility.
- Long-term maintainability.

### 4.2 Compositor Framework

Use:

```txt
Smithay
```

Baton should be built with Smithay.

Reasons:

- Rust-native Wayland compositor framework.
- Good fit for custom compositor behavior.
- Allows Staccato to define its own shell, modes, materials, and layout model.
- Avoids being boxed into another compositor’s policy model.

### 4.3 Shell UI Toolkit

The user-facing shell chrome is implemented with:

```txt
Svelte
TypeScript
Tailwind CSS
```

Rust remains responsible for compositor-facing work:

- Wayland layer-surface lifetime.
- IPC and model snapshots.
- App launching.
- Tray and notification services.
- Workspace, window, and profile actions.
- Compositor materials and blur behavior.

The web layer renders panel, dock, sidebar, overview, quick settings, and date/notification center surfaces. It sends typed actions back to Rust instead of owning desktop state directly.

The current embedded runtime is a GTK layer-shell window hosting the bundled web UI. This is the shell UI implementation, not a second fallback beside a native shell renderer.

### 4.4 Config Format

Use:

```txt
TOML
```

Optionally support RON later for internal state if needed.

### 4.5 IPC

Use for MVP:

```txt
Unix socket + serde messages
```

Later, add D-Bus integration for desktop ecosystem compatibility.

### 4.6 Build System

Use:

```txt
Cargo workspace
```

---

## 5. Repository Structure

The project should be a Rust workspace.

```txt
staccato/
├── Cargo.toml
├── README.md
├── staccato.md
├── crates/
│   ├── baton/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── state.rs
│   │       ├── backend/
│   │       │   ├── mod.rs
│   │       │   ├── nested.rs
│   │       │   ├── drm.rs
│   │       │   └── headless.rs
│   │       ├── input/
│   │       │   ├── mod.rs
│   │       │   ├── keyboard.rs
│   │       │   ├── pointer.rs
│   │       │   ├── touch.rs
│   │       │   └── gestures.rs
│   │       ├── output/
│   │       │   ├── mod.rs
│   │       │   ├── monitor.rs
│   │       │   ├── scale.rs
│   │       │   └── layout.rs
│   │       ├── protocols/
│   │       │   ├── mod.rs
│   │       │   ├── xdg_shell.rs
│   │       │   ├── layer_shell.rs
│   │       │   ├── activation.rs
│   │       │   ├── screencopy.rs
│   │       │   ├── security.rs
│   │       │   └── background_effect.rs
│   │       ├── render/
│   │       │   ├── mod.rs
│   │       │   ├── renderer.rs
│   │       │   ├── damage.rs
│   │       │   ├── frame.rs
│   │       │   ├── blur.rs
│   │       │   ├── materials.rs
│   │       │   └── shadows.rs
│   │       ├── window/
│   │       │   ├── mod.rs
│   │       │   ├── window.rs
│   │       │   ├── focus.rs
│   │       │   ├── placement.rs
│   │       │   ├── rules.rs
│   │       │   └── xwayland.rs
│   │       └── recovery/
│   │           ├── mod.rs
│   │           └── crash_guard.rs
│   ├── staccato-shell/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── app.rs
│   │       ├── panel/
│   │       ├── dock/
│   │       ├── sidebar/
│   │       ├── launcher/
│   │       ├── overview/
│   │       ├── quick_settings/
│   │       ├── notifications/
│   │       ├── workspaces/
│   │       ├── command_palette/
│   │       └── recovery_ui/
│   ├── staccato-layout/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── workspace.rs
│   │       ├── window.rs
│   │       ├── group.rs
│   │       ├── split_tree.rs
│   │       ├── tab_stack.rs
│   │       ├── mode.rs
│   │       └── modes/
│   │           ├── dock.rs
│   │           ├── panel.rs
│   │           ├── browser.rs
│   │           ├── tiling.rs
│   │           ├── focus.rs
│   │           └── classic.rs
│   ├── staccato-config/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── config.rs
│   │       ├── profile.rs
│   │       ├── materials.rs
│   │       ├── keybindings.rs
│   │       ├── rules.rs
│   │       ├── validation.rs
│   │       └── reload.rs
│   ├── staccato-ipc/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── protocol.rs
│   │       ├── client.rs
│   │       ├── server.rs
│   │       └── events.rs
│   ├── staccato-session/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── login.rs
│   │       ├── environment.rs
│   │       ├── autostart.rs
│   │       └── xdg.rs
│   ├── staccatoctl/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── main.rs
│   └── staccato-ai/
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── patch.rs
│           ├── manifest.rs
│           ├── sandbox.rs
│           ├── validation.rs
│           └── providers.rs
├── data/
│   ├── sessions/
│   │   └── staccato.desktop
│   ├── systemd/
│   │   └── user/
│   ├── schemas/
│   └── default-config/
├── docs/
├── examples/
└── tests/
```

---

## 6. Process Model

Staccato should be split into clear components.

### 6.1 Initial MVP Process Model

For early development, the compositor may launch the shell directly.

```txt
baton
└── staccato-shell
```

### 6.2 Target Process Model

```txt
baton
staccato-shell
staccato-sessiond
staccato-settingsd
staccato-notificationd
staccato-polkit-agent
staccato-ai-agent
```

Not all of these need to exist immediately, but the architecture must not prevent them.

### 6.3 Baton Compositor Responsibilities

Baton owns:

- Wayland display.
- Wayland protocol implementation.
- Output management.
- Input handling.
- Surface lifecycle.
- Window state.
- XWayland integration.
- Frame scheduling.
- Damage tracking.
- Rendering.
- Blur and compositor effects.
- Material rendering hooks.
- Security-sensitive protocols.
- Screenshot/screencopy permission policy.
- Recovery behavior if shell crashes.

Baton must remain stable even if the shell crashes.

### 6.4 Staccato Shell Responsibilities

The shell owns:

- Panel UI.
- Dock UI.
- Sidebar UI.
- App launcher.
- Workspace overview.
- Window switcher.
- Quick settings.
- Notification UI.
- Command palette.
- Profile/mode UI.
- Recovery UI.
- Future settings UI integration.

The shell must be restartable without killing the compositor session.

---

## 7. Baton Compositor

### 7.1 Baton Goals

Baton must be:

- Fast.
- Stable.
- Minimal where possible.
- Explicitly designed for Staccato Shell.
- Powerful enough for dynamic blur and materials.
- Safe enough for experimental user shell customizations.
- Testable nested.
- Usable as a real session.

### 7.2 Backends

Baton must support:

#### Nested Wayland Backend

For development.

```sh
baton --nested
```

This runs Baton inside an existing Wayland session.

Required:

- Create nested output/window.
- Accept Wayland clients.
- Render them inside the nested output.
- Allow resizing if possible.
- Allow debug overlays.

#### Headless Backend

For automated tests.

```sh
baton --headless
```

Required:

- Run without physical display.
- Simulate output.
- Run layout tests.
- Run protocol tests.
- Useful for CI.

#### DRM/KMS Backend

For real sessions.

```sh
baton --session
```

Required:

- Direct display output.
- libinput.
- session handling.
- output hotplug.
- monitor layout.
- scale handling.
- refresh-rate handling.

### 7.3 Wayland Protocols

Baton must support these protocol groups.

#### Required Early

```txt
wl_compositor
wl_shm
wl_seat
wl_output
wl_data_device_manager
xdg_wm_base / xdg-shell
viewporter
presentation-time
xdg-activation
```

#### Required for Shell/DE

```txt
wlr-layer-shell or compatible layer-shell support
idle-inhibit
fractional-scale
xdg-decoration
relative-pointer
pointer-constraints
text-input/input-method eventually
output-management
foreign-toplevel-management or Staccato equivalent
```

#### Required for Compatibility

```txt
XWayland
```

#### Required for Effects

```txt
ext-background-effect-v1
```

#### Required for Screen Capture, With Permission

```txt
screencopy
export-dmabuf where appropriate
```

Screen capture protocols must be permission-gated.

### 7.4 `ext-background-effect-v1`

Support for `ext-background-effect-v1` is important.

Baton must implement support for the protocol where possible.

The purpose:

- Allow clients to request compositor-provided background effects.
- Enable blur behind surfaces.
- Avoid every toolkit/app implementing fake blur.
- Make Tauri/Electron/native apps able to use real background effects.
- Align with modern Wayland compositor-level material effects.

Expected behavior:

- One background effect object per surface where the protocol allows.
- Support setting a blur region.
- Support clearing blur region.
- Treat state as double-buffered and applied on commit.
- Clip blur region to surface dimensions.
- Paint blurred background before painting client surface content.
- Damage/redraw must account for sampled pixels behind blur region.
- Avoid leaking pixels across security boundaries.
- Do not let clients blur arbitrary unrelated screen areas outside their surface bounds.
- Support fallback behavior when the protocol is not available.

### 7.5 Internal Material Path

Staccato Shell should not need to use the public protocol path for every internal surface.

Baton should expose an internal material API for trusted shell surfaces.

Example internal material request:

```rust
pub enum ShellMaterial {
    Solid(SolidMaterial),
    Luca(LucaMaterial),
    Maris(MarisMaterial),
    Acrylic(AcrylicMaterial),
}
```

Trusted shell surfaces can request:

- Blur radius.
- Tint.
- Saturation.
- Noise.
- Border highlight.
- Shadow.
- Corner radius.
- Optional wallpaper-adaptive tint.

Untrusted clients must use public protocols and permissions.

### 7.6 Blur Rendering

Baton must provide compositor-level blur.

Requirements:

- Capture background behind target region.
- Blur offscreen.
- Composite blurred result beneath surface.
- Support rounded regions.
- Support damage tracking.
- Support moving windows without obvious blur lag.
- Support multi-output cases.
- Respect scale factors.
- Avoid sampling protected/secure surfaces.
- Avoid leaking pixels across lock screen or secure layers.
- Support fallback if blur is disabled.

Blur must not be implemented as fake transparency in shell UI.

### 7.7 Damage and Performance

Blur can be expensive. Baton must include a performance-conscious render path.

Requirements:

- Damage tracking includes blur sample expansion.
- Cache blurred regions when possible.
- Avoid full-screen reblur on every small damage event.
- Use scissor/clip regions.
- Support frame timing debug overlay.
- Expose counters for:
  - frame time
  - missed frames
  - blur passes
  - damaged area
  - surfaces rendered
  - GPU memory usage if available
- Allow disabling expensive effects automatically on low-power systems or battery, but this must be configurable.

### 7.8 Window Management

Baton must support:

- Floating windows.
- Tiled windows.
- Tabbed windows through layout engine.
- Split containers through layout engine.
- Server-side positioning.
- Interactive move/resize.
- Focus tracking.
- Window rules.
- Workspace assignment.
- Fullscreen.
- Maximize.
- Minimize or equivalent hidden state.
- Dialog/transient handling.
- Popups.
- Drag and drop.
- Clipboard.
- App activation.

Baton should avoid hardcoding one window management model. The layout engine and active mode should decide behavior.

### 7.9 Shell Surface Layers

Baton must support shell surfaces with layers:

```txt
Background
Normal windows
Dock/panel/sidebar
Overlay
Notifications
Lock screen
Critical system UI
```

Layer behavior must be explicit.

Example:

- Wallpaper is background.
- App windows are normal.
- Dock/panel/sidebar are shell chrome.
- Launcher/overview are overlay.
- Lock screen is secure and blocks normal access.
- Critical recovery UI is top-level trusted shell UI.

### 7.10 Security Boundaries

Baton must treat these as security-sensitive:

- Screen capture.
- Input capture.
- Global shortcuts.
- Clipboard access.
- Secure lock screen.
- Background blur sampling.
- Protected surfaces.
- Password fields if detectable through protocols/toolkits.
- AI customization applying code.

Do not make arbitrary clients trusted.

---

## 8. Staccato Shell

### 8.1 Shell Goals

Staccato Shell is the visible desktop interface.

It must:

- Look polished by default.
- Be lightweight.
- Be modular.
- Support multiple modes.
- Use Staccato materials.
- Support profile switching.
- Be restartable.
- Expose state to CLI/settings UI.
- Avoid being tied to any single distro.

### 8.2 Shell Components

Required shell components:

```txt
Panel
Dock
Sidebar
Launcher
Overview
Workspace switcher
Window switcher
Quick settings
Notification UI
Command palette
Recovery UI
```

Not every component is active in every mode.

### 8.3 Panel

Panel should support:

- Top, bottom, left, right positions.
- Variable height/width.
- Glass material.
- Solid fallback.
- Clock.
- System indicators.
- Workspace indicator.
- Active app/window title optional.
- Tray/status items if supported.
- Quick settings entry.
- Launcher entry.
- Profile/mode indicator optional.

### 8.4 Dock

Dock should support:

- Bottom/left/right positions.
- Centered or edge-aligned layout.
- Pinned apps.
- Running apps.
- App badges.
- Window previews.
- Autohide.
- Intellihide.
- Luca/Maris/solid material.
- Icon zoom optional.
- Compact mode.
- Touch-friendly mode.

### 8.5 Sidebar

Sidebar is essential for Browser Mode.

Sidebar should support:

- Workspace groups.
- App tabs.
- Pinned apps.
- Split groups.
- Current session tree.
- Quick launcher.
- Search/command palette entry.
- Collapsed/expanded states.
- Keyboard navigation.
- Drag/drop tab reordering.
- Drag/drop tab into split.

### 8.6 Launcher

Launcher should support:

- App search.
- Command search.
- Settings search.
- Recent apps.
- Profile switching.
- Workspace switching.
- Actions.
- Future plugin-provided commands.

Launcher should be keyboard-first.

### 8.7 Overview

Overview should support:

- All workspaces.
- Windows in current workspace.
- Window thumbnails.
- Workspace profile labels.
- Drag windows between workspaces.
- Add/remove/reorder workspaces.
- Switch profile for a workspace.
- Search integration.

### 8.8 Quick Settings

Quick settings should support:

- Network entry point.
- Bluetooth entry point.
- Audio volume.
- Brightness.
- Battery/power.
- Night light.
- Accessibility toggles.
- Effect toggles.
- Performance mode.
- Profile/mode entry.
- Logout/reboot/shutdown entries.

Actual network/bluetooth/power backends may be integrated later through existing Linux services, but UI and interfaces should be planned.

### 8.9 Notifications

Staccato should implement notification UI compatible with Linux desktop notification systems.

Requirements:

- Show notifications.
- Group notifications.
- Respect do-not-disturb.
- Per-profile notification behavior.
- Focus Mode may suppress or batch notifications.
- Browser Mode may show notifications in sidebar.
- Tiling Mode may show compact notifications.
- Dock Mode may show floating toasts.

### 8.10 Command Palette

Command palette is a core UX primitive.

It should allow:

- Launch app.
- Switch window.
- Switch workspace.
- Switch profile.
- Run shell command if enabled.
- Change setting.
- Toggle effect.
- Search open tabs/groups.
- Trigger layout actions.
- Reload config.
- Open logs.
- Enter safe mode.

---

## 9. Layout Engine

### 9.1 Purpose

The layout engine is the core abstraction that makes modes possible.

It should not be tied to any single shell UI.

It represents windows, tabs, splits, groups, and workspaces.

### 9.2 Core Types

```rust
pub struct Workspace {
    pub id: WorkspaceId,
    pub name: String,
    pub profile_id: ProfileId,
    pub root: LayoutNode,
    pub floating_windows: Vec<WindowId>,
    pub pinned_apps: Vec<AppId>,
    pub rules: Vec<WorkspaceRule>,
}

pub enum LayoutNode {
    Empty,
    Window(WindowId),
    TabStack(TabStack),
    Split(SplitNode),
    Group(GroupNode),
}

pub struct TabStack {
    pub id: TabStackId,
    pub tabs: Vec<WindowId>,
    pub active: Option<WindowId>,
}

pub struct SplitNode {
    pub id: SplitNodeId,
    pub axis: SplitAxis,
    pub ratio: f32,
    pub first: Box<LayoutNode>,
    pub second: Box<LayoutNode>,
}

pub enum SplitAxis {
    Horizontal,
    Vertical,
}

pub struct GroupNode {
    pub id: GroupId,
    pub name: String,
    pub root: Box<LayoutNode>,
}
```

### 9.3 Window Model

```rust
pub struct WindowInfo {
    pub id: WindowId,
    pub app_id: Option<String>,
    pub title: Option<String>,
    pub pid: Option<u32>,
    pub is_xwayland: bool,
    pub state: WindowState,
    pub geometry: Rect,
    pub workspace: WorkspaceId,
}
```

### 9.4 Layout Capabilities

The layout engine must support:

- Floating placement.
- Tiled placement.
- Split placement.
- Tab stacks.
- Groups.
- Saved layout trees.
- Per-workspace restore.
- Rules for new windows.
- Drag window to split.
- Drag window to tab.
- Move window between groups.
- Move window between workspaces.
- Snapshot/restore.

### 9.5 Layout Serialization

Layout state must be serializable.

Use a stable format for saved sessions:

```txt
~/.local/state/staccato/sessions/
```

Example:

```toml
[workspace.dev]
profile = "browser-dev"

[workspace.dev.group.main]
name = "Staccato"

[[workspace.dev.windows]]
app_id = "org.wezfurlong.wezterm"
placement = "bottom-split"

[[workspace.dev.tabs]]
stack = "left"
app_id = "code"
```

Actual runtime state can be more detailed than user config.

---

## 10. Modes

### 10.1 Mode Concept

A Mode is a policy layer over the layout engine and shell components.

A mode decides:

- Which shell chrome is visible.
- How new windows are placed.
- How app switching works.
- Whether windows float, tile, tab, or split.
- How workspaces look.
- Which shortcuts are active.
- Which material/density defaults apply.

Modes must not be separate codebases.

They must use the same primitives.

### 10.2 Mode Trait

```rust
pub trait ShellMode {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;

    fn chrome(&self, profile: &ShellProfile) -> ChromeSpec;

    fn on_window_opened(
        &self,
        window: WindowId,
        workspace: &mut Workspace,
        ctx: &mut ModeContext,
    );

    fn arrange(
        &self,
        workspace: &mut Workspace,
        ctx: &LayoutContext,
    ) -> Arrangement;

    fn handle_action(
        &self,
        action: ShellAction,
        workspace: &mut Workspace,
        ctx: &mut ModeContext,
    ) -> ActionResult;
}
```

### 10.3 Required Modes

#### 10.3.1 Classic Mode

Traditional desktop.

Features:

- Normal floating windows.
- Panel or dock.
- App launcher.
- Workspace switcher.
- Alt-tab.
- Familiar behavior.

Purpose:

- Safe default.
- Easy onboarding.
- Compatibility.

#### 10.3.2 Dock Mode

Polished visual desktop.

Features:

- Dock.
- Optional top panel.
- Floating windows.
- Window previews.
- Overview.
- Smooth animations.
- Luca/Maris material by default.

Purpose:

- macOS-ish visual workflow without copying macOS.
- Casual/personal workspace.

#### 10.3.3 Panel Mode

Traditional Linux desktop mode.

Features:

- Main panel.
- App menu.
- Window list.
- Tray/status indicators.
- Clock.
- Quick settings.
- Optional bottom panel.

Purpose:

- Familiar Linux/Windows-like workflow.
- Useful for users who want predictability.

#### 10.3.4 Tiling Mode

Keyboard-first power mode.

Features:

- Tiled windows.
- Smart gaps.
- Split directions.
- Scratchpads.
- Floating exceptions.
- Workspace rules.
- Keyboard navigation.
- Compact chrome.
- Optional panel/sidebar.

Purpose:

- Developer workflows.
- Terminal-heavy usage.
- Hyprland/Sway-style flexibility without config goblin trauma.

#### 10.3.5 Browser Mode

Browser-like desktop workflow.

Features:

- Left sidebar.
- Apps as tabs.
- Split views.
- Groups.
- Pinned apps.
- Workspace sessions.
- Command palette.
- Tab stacks.
- Drag/drop tabs into splits.
- Restore grouped work sessions.

Purpose:

- Staccato’s major differentiator.
- Desktop-level Arc/Zen/VS Code style workflow.
- Useful for dev, research, school, writing, admin.

#### 10.3.6 Focus Mode

Minimal distraction mode.

Features:

- Minimal or hidden chrome.
- Optional centered active app.
- Notification suppression/batching.
- Dimmed background.
- Reduced animation.
- Allowed apps optional.
- Focus timer optional through future plugin.

Purpose:

- Writing.
- Studying.
- Deep work.
- Avoiding notification chaos.

#### 10.3.7 Tablet Mode

Touch-friendly mode.

Features:

- Larger targets.
- Gesture navigation.
- Fullscreen or near-fullscreen apps.
- Touch keyboard integration.
- Simplified launcher.
- Larger quick settings.

Purpose:

- Convertible devices.
- Touchscreen laptops.
- Future mobile-ish experiments.

Tablet Mode can be implemented after core modes but the architecture must allow it.

### 10.4 Per-Workspace Modes

Workspaces are dynamic. The default session starts with one empty workspace and creates/removes extra workspaces as needed.

A workspace may still have a profile that uses any mode:

```toml
[workspaces.1]
name = "Workspace 1"
profile = "panel-default"
```

Switching workspace can change the visible shell behavior, but shipped config must not pre-create named preset workspaces.

This is intentional.

---

## 11. Shell Profiles

### 11.1 Purpose

Shell Profiles define the behavior, layout, chrome, theme, material, density, and interaction model of a workspace.

Profiles are user-editable and future AI-editable.

### 11.2 Profile Example

```toml
id = "browser-dev"
name = "Browser Dev"
mode = "browser"

[chrome]
panel = false
dock = false
sidebar = true
overview = true
command_palette = true

[sidebar]
position = "left"
width = 280
collapsed_width = 56
material = "luca"
show_pinned_apps = true
show_groups = true
show_tabs = true

[windows]
default_placement = "tab"
floating_allowed = true
splits = true
tab_stacks = true
restore_session = true

[materials]
default = "luca"
blur_radius = 36
corner_radius = 18
tint_opacity = 0.22
noise_opacity = 0.035

[notifications]
mode = "muted"
show_badges = true

[animation]
profile = "staccato-fast"

[keybindings]
"Super+Space" = "launcher.open"
"Super+Return" = "terminal.open"
"Super+Tab" = "tabs.next"
"Super+Shift+Tab" = "tabs.previous"
"Super+Backslash" = "split.vertical"
"Super+Minus" = "split.horizontal"
```

### 11.3 Profile Fields

Profiles should support:

- `id`
- `name`
- `mode`
- chrome settings
- dock settings
- panel settings
- sidebar settings
- launcher settings
- overview settings
- window behavior
- layout behavior
- material settings
- animation settings
- notification behavior
- keybindings
- workspace rules
- app rules
- performance settings
- accessibility settings

### 11.4 Profile Management

Profiles must be:

- Stored in files.
- Listed through CLI.
- Activated through CLI.
- Assigned to workspaces.
- Validated before use.
- Reloadable when safe.
- Revertible.
- Exportable.
- Importable.

---

## 12. Configuration System

### 12.1 Config Locations

Use XDG paths.

```txt
~/.config/staccato/
├── config.toml
├── profiles/
│   ├── dock-default.toml
│   ├── browser-dev.toml
│   ├── tiling-dev.toml
│   └── focus-writing.toml
├── materials/
│   ├── luca.toml
│   └── maris.toml
├── keybindings.toml
├── rules.toml
├── patches/
└── extensions/
```

Runtime state:

```txt
~/.local/state/staccato/
├── sessions/
├── layouts/
├── logs/
├── crash/
├── backups/
└── recovery/
```

Cache:

```txt
~/.cache/staccato/
```

System defaults:

```txt
/usr/share/staccato/default-config/
```

### 12.2 Main Config Example

```toml
[general]
default_profile = "panel-default"
enable_effects = true
enable_blur = true
enable_animations = true
safe_mode = false

[compositor]
backend = "auto"
xwayland = true
debug_overlay = false
prefer_direct_scanout = true

[effects]
background_effect_protocol = true
blur = true
blur_quality = "balanced"
disable_blur_on_battery = false

[workspaces]
count = 1
restore_sessions = true

[default_apps]
terminal = "ghostty"
file_manager = "nautilus"
browser = "google-chrome-stable"
settings = "gnome-control-center"
launcher = "vicinae"

[recovery]
crash_limit = 3
crash_window_seconds = 60
auto_safe_mode = true
backup_before_apply = true
```

### 12.3 Validation

All config must be validated before application.

Validation must check:

- Unknown profile IDs.
- Invalid mode IDs.
- Invalid keybindings.
- Duplicate keybindings.
- Invalid material values.
- Invalid workspace references.
- Unsafe patch references.
- Unsupported compositor options.
- Syntax errors.

Invalid config must not brick the session.

If config validation fails:

- Keep current config.
- Show error through CLI/shell.
- Log details.
- Offer safe fallback.

---

## 13. staccatoctl CLI

### 13.1 Purpose

`staccatoctl` is the primary configuration and debugging interface.

It allows users, scripts, future settings apps, and Codex/AI tools to interact with Staccato safely.

### 13.2 Required Commands

```sh
staccatoctl status
staccatoctl logs
staccatoctl doctor
staccatoctl reload
staccatoctl safe-mode enable
staccatoctl safe-mode disable
staccatoctl profiles list
staccatoctl profiles show <id>
staccatoctl profiles validate <path-or-id>
staccatoctl profiles set-workspace <workspace> <profile>
staccatoctl modes list
staccatoctl workspaces list
staccatoctl workspaces switch <workspace>
staccatoctl workspaces set-profile <workspace> <profile>
staccatoctl effects status
staccatoctl effects blur enable
staccatoctl effects blur disable
staccatoctl materials list
staccatoctl config path
staccatoctl config validate
staccatoctl config open
staccatoctl recovery status
staccatoctl recovery rollback
staccatoctl debug overlay enable
staccatoctl debug overlay disable
```

### 13.3 Future Commands

```sh
staccatoctl patches list
staccatoctl patches apply <patch>
staccatoctl patches revert <patch>
staccatoctl extensions list
staccatoctl extensions enable <id>
staccatoctl extensions disable <id>
staccatoctl ai propose "<instruction>"
staccatoctl ai explain <patch>
```

### 13.4 Machine-Readable Output

All relevant commands should support:

```sh
--json
```

Example:

```sh
staccatoctl profiles list --json
```

This allows a future settings app to use `staccatoctl` or the underlying IPC as a backend.

---

## 14. Materials and Visual System

### 14.1 Goals

Staccato should look good out of the box.

The visual system must be:

- Clean.
- Glassy where appropriate.
- Lightweight.
- Consistent.
- Token-based.
- Wallpaper-adaptive.
- Not dependent on GTK themes.
- Not visually trapped by KDE/GNOME conventions.

### 14.2 Materials

#### Luca

Dynamic glass material.

Use for:

- Dock.
- Sidebar.
- Panel.
- Launcher.
- Overview cards.
- Command palette.
- Window switcher.

Properties:

- Background blur.
- Tint.
- Saturation.
- Noise.
- Border highlight.
- Soft shadow.
- Rounded corners.
- Wallpaper-adaptive color.

#### Maris

Softer mica-like material.

Use for:

- Low-motion contexts.
- Battery-saving contexts.
- Panels where live blur is too expensive.
- Focus Mode.
- Settings-like surfaces.
- Solid-ish shells.

Properties:

- Wallpaper-derived tint.
- Subtle translucency.
- Optional static blur/backdrop.
- Less expensive than Luca.
- Lower visual noise.

#### Solid

Fallback material.

Use when:

- Blur disabled.
- GPU unsupported.
- Safe mode.
- Remote desktop.
- Performance mode.
- Battery saver if configured.

### 14.3 Material Tokens

Example:

```toml
[materials.luca]
blur_radius = 36
saturation = 1.25
brightness = 1.05
tint = "#ffffff"
tint_opacity = 0.22
noise_opacity = 0.035
border_opacity = 0.16
corner_radius = 18
shadow_opacity = 0.28

[materials.maris]
tint_source = "wallpaper"
tint_opacity = 0.34
noise_opacity = 0.015
corner_radius = 16
shadow_opacity = 0.18
```

### 14.4 Wallpaper-Adaptive Tint

Staccato should be able to derive colors from the current wallpaper.

Requirements:

- Extract dominant colors.
- Extract accent candidates.
- Pick readable foreground colors.
- Support manual override.
- Recompute when wallpaper changes.
- Cache extracted palette.

### 14.5 Animation Language

Staccato should use crisp, quick, precise animations.

The name implies short, sharp rhythm.

Animation defaults:

```toml
[animation.staccato]
duration_fast_ms = 90
duration_normal_ms = 160
duration_slow_ms = 240
curve = "emphasized-decelerate"
reduced_motion = false
```

Reduced motion must be supported.

---

## 15. Browser Mode Specification

### 15.1 Purpose

Browser Mode is the most distinctive Staccato workflow.

It makes the whole desktop feel closer to a browser/IDE workspace:

- Apps become tabs.
- Tabs can be grouped.
- Groups can be split.
- Workspaces become sessions.
- Sidebar becomes the main navigation.
- The command palette becomes the main action interface.

### 15.2 Core UX

Screen structure:

```txt
┌──────────────┬─────────────────────────────────────┐
│ Sidebar      │ Active layout area                  │
│              │                                     │
│ Groups       │ ┌──────────────┬──────────────────┐ │
│ Tabs         │ │ App/Tab      │ App/Tab          │ │
│ Pinned apps  │ │              │                  │ │
│ Commands     │ └──────────────┴──────────────────┘ │
│              │                                     │
└──────────────┴─────────────────────────────────────┘
```

### 15.3 App Tabs

Each tab represents a managed window/surface.

Requirements:

- Show app icon.
- Show title.
- Show dirty/attention state if known.
- Close tab.
- Reorder tab.
- Move tab to group.
- Move tab to split.
- Pin tab.
- Restore tab where possible.

### 15.4 Splits

Browser Mode must support:

- Horizontal splits.
- Vertical splits.
- Resizable split ratios.
- Drag tab into split.
- Keyboard split commands.
- Close split.
- Move focus between splits.
- Save split layout.

### 15.5 Groups

Groups are named collections of tabs/splits.

Examples:

```txt
Project: Staccato
Project: Ave
Research
Admin
Writing
```

Group requirements:

- Rename group.
- Reorder group.
- Assign icon/color optional.
- Collapse group.
- Restore group session.
- Move tabs between groups.

### 15.6 Pinned Apps

Pinned apps should appear in sidebar.

Clicking a pinned app:

- Focuses existing tab/window if open.
- Opens app if not open.
- Applies placement rules.

### 15.7 Rules

Browser Mode should support rules like:

```toml
[[rules]]
app_id = "org.wezfurlong.wezterm"
open = "bottom-split"

[[rules]]
app_id = "firefox"
open = "right-split"

[[rules]]
title_contains = "Documentation"
group = "Research"
```

### 15.8 Session Restore

Browser Mode should restore:

- Groups.
- Tabs where possible.
- Split layout.
- Pinned apps.
- Working directories where known.
- Profile assignment.

Not every app supports perfect restore. Staccato should do best effort.

---

## 16. Tiling Mode Specification

### 16.1 Purpose

Tiling Mode provides keyboard-first window management without requiring users to hand-write a full window manager config.

### 16.2 Features

Required:

- Automatic tiling.
- Manual splits.
- Smart gaps.
- Floating exceptions.
- Scratchpads.
- Keyboard focus movement.
- Move window between tiles.
- Resize tile.
- Toggle floating.
- Toggle fullscreen.
- Move window to workspace.
- Per-app rules.

### 16.3 Example Config

```toml
[profile.tiling-dev]
mode = "tiling"

[profile.tiling-dev.tiling]
gaps_inner = 8
gaps_outer = 12
smart_gaps = true
default_split = "horizontal"
focus_follows_mouse = false

[[profile.tiling-dev.rules]]
app_id = "pavucontrol"
behavior = "floating"

[[profile.tiling-dev.rules]]
app_id = "org.gnome.Calculator"
behavior = "floating"
```

---

## 17. Dock Mode Specification

### 17.1 Purpose

Dock Mode is the polished default visual desktop.

### 17.2 Features

Required:

- Dock.
- Optional top panel.
- Floating windows.
- Overview.
- App launcher.
- Window previews.
- Workspace switching.
- Luca/Maris material.
- Familiar behavior.

### 17.3 Default Use

Dock Mode should be a strong candidate for the default Staccato experience.

---

## 18. Panel Mode Specification

### 18.1 Purpose

Panel Mode is for users who want a traditional desktop.

### 18.2 Features

Required:

- Main panel.
- App menu/launcher.
- Window list.
- Tray/status area.
- Clock.
- Workspace indicator.
- Quick settings.
- Floating windows.
- Optional bottom/top variants.

---

## 19. Focus Mode Specification

### 19.1 Purpose

Focus Mode reduces distraction.

### 19.2 Features

Required:

- Hide dock/panel or make it minimal.
- Suppress or batch notifications.
- Show only active app or focused set.
- Optional background dim.
- Optional reduced motion.
- Optional allowed app list.
- Easy exit shortcut.

Example:

```toml
[profile.focus-writing]
mode = "focus"

[profile.focus-writing.notifications]
mode = "batched"

[profile.focus-writing.chrome]
dock = false
panel = false
sidebar = false

[profile.focus-writing.focus]
dim_background = true
center_active_window = true
allowed_apps = ["writer", "browser"]
```

---

## 20. AI-Assisted Customization

### 20.1 Purpose

AI customization is not the core product. Hackability is the core product.

AI should be able to safely generate:

- Shell profiles.
- Material configs.
- Keybinding configs.
- Layout rules.
- Extension skeletons.
- Patch files.

AI must not randomly mutate core compositor code unless explicitly in developer mode.

### 20.2 Safety Model

AI changes must go through:

```txt
Instruction
→ Proposal
→ Patch/Profile/Extension
→ Validation
→ Preview
→ Apply
→ Backup
→ Rollback possible
```

### 20.3 Allowed Zones

Safe AI zones:

```txt
~/.config/staccato/profiles/
~/.config/staccato/materials/
~/.config/staccato/keybindings.toml
~/.config/staccato/rules.toml
~/.config/staccato/extensions/
```

Restricted zones:

```txt
Compositor source
Shell source
System files
Session files
```

Restricted zones require developer mode.

### 20.4 Patch Storage

```txt
~/.config/staccato/patches/
├── 2026-05-24-browser-dev-profile.patch
├── 2026-05-24-glassier-dock.patch
└── 2026-05-24-focus-writing.patch
```

### 20.5 Extension Manifests

Example:

```toml
id = "vertical-rail"
name = "Vertical Rail"
version = "1.0.0"
author = "local-ai"
description = "Replaces the default dock with a compact left rail."

[permissions]
windows = "read"
apps = "read"
shell = "modify"
network = "none"
filesystem = "none"

[targets]
staccato_shell = ">=0.1.0"
```

### 20.6 Future Provider Support

AI provider abstraction should allow:

- Local models.
- OpenAI-compatible APIs.
- Anthropic-compatible APIs.
- OpenRouter-compatible APIs.
- Custom endpoints.

The DE must work fully without AI.

---

## 21. Extensions

### 21.1 Purpose

Extensions allow users to customize Staccato without modifying core code.

### 21.2 Extension Types

Supported extension categories:

- Panel widgets.
- Dock widgets.
- Sidebar sections.
- Launcher providers.
- Command palette actions.
- Window rules.
- Profile templates.
- Material presets.
- Mode plugins, restricted/developer only.

### 21.3 Extension Permissions

Extensions must declare permissions.

Possible permissions:

```toml
[permissions]
windows = "none|read|modify"
apps = "none|read|launch"
shell = "none|read|modify"
notifications = "none|read|send"
filesystem = "none|config|home"
network = "none|allowed"
commands = "none|limited|full"
```

Default must be restrictive.

### 21.4 Extension Isolation

Extensions must not be allowed to crash the entire shell or compositor.

Preferred:

- Run extensions in a sandboxed process if possible.
- At minimum, catch panics/errors and disable broken extension.
- Disable extension after repeated crash.
- Provide safe mode that disables all user extensions.

---

## 22. Recovery and Safe Mode

### 22.1 Required Recovery Features

Staccato must include:

- Safe mode.
- Config validation.
- Automatic rollback.
- Crash loop detection.
- Emergency shortcuts.
- Shell restart.
- Disable user patches.
- Disable user extensions.
- Default profile fallback.
- Recovery UI.

### 22.2 Emergency Shortcuts

Default emergency shortcuts:

```txt
Super+Esc                    Open recovery menu
Super+Shift+R                Restart shell
Super+Shift+Backspace        Disable user config and reload default profile
Ctrl+Alt+Backspace optional  End session if enabled
```

### 22.3 Crash Loop Detection

If shell crashes repeatedly:

```txt
3 crashes within 60 seconds
```

Then:

- Disable user extensions.
- Disable user patches.
- Load default profile.
- Show recovery UI.
- Log crash reason.

### 22.4 Safe Mode Behavior

Safe mode must:

- Disable blur/effects optionally.
- Disable user extensions.
- Disable AI-generated patches.
- Load default solid theme.
- Use Classic or Panel Mode.
- Preserve user data.
- Allow user to inspect/revert broken config.

Command:

```sh
staccatoctl safe-mode enable
```

---

## 23. Testing and Development

### 23.1 Nested Testing

Development must support nested mode.

Example:

```sh
cargo run -p baton -- --nested
```

Then:

```sh
WAYLAND_DISPLAY=staccato-1 alacritty
WAYLAND_DISPLAY=staccato-1 weston-terminal
WAYLAND_DISPLAY=staccato-1 foot
WAYLAND_DISPLAY=staccato-1 gtk4-demo
```

The exact socket name may differ, but it should be printed clearly.

### 23.2 Real Session Testing

Install session file:

```txt
/usr/share/wayland-sessions/staccato.desktop
```

Example:

```ini
[Desktop Entry]
Name=Staccato
Comment=Staccato Desktop Environment
Exec=staccato-session
Type=Application
DesktopNames=Staccato
```

### 23.3 Debug Overlay

Baton should include debug overlay showing:

- FPS.
- Frame time.
- Damaged area.
- Number of surfaces.
- Active backend.
- Active workspace.
- Active profile.
- Blur passes.
- GPU renderer info if available.
- XWayland status.

Toggle:

```sh
staccatoctl debug overlay enable
```

### 23.4 Logs

Logs should be stored in:

```txt
~/.local/state/staccato/logs/
```

Commands:

```sh
staccatoctl logs
staccatoctl logs --follow
staccatoctl doctor
```

### 23.5 Automated Tests

Add tests for:

- Config parsing.
- Profile validation.
- Layout tree operations.
- Mode behavior.
- Rule matching.
- IPC messages.
- Crash recovery logic.
- Material token validation.

Headless compositor tests should eventually verify:

- Windows can map/unmap.
- Focus changes.
- Workspace switching.
- Layout application.
- Effects fallback.

---

## 24. Distribution Integration

### 24.1 Must Work On Existing Distros

Staccato must not assume Glacier OS.

It must support packaging for:

- Fedora.
- Arch.
- openSUSE.
- Debian/Ubuntu where practical.
- NixOS.
- Source builds.

### 24.2 Runtime Dependencies

Keep dependencies reasonable.

Expected categories:

- Wayland libraries.
- libinput.
- xkbcommon.
- udev/libseat/seatd/logind integration.
- graphics stack.
- XWayland.
- PipeWire for screen capture later.
- D-Bus for desktop services later.

### 24.3 Desktop Portals

Staccato should eventually provide or integrate with XDG Desktop Portals.

Important for:

- Screen sharing.
- File picker.
- Open URI.
- Settings.
- Permissions.

This can integrate with existing portals initially where possible.

---

## 25. Accessibility

Staccato must not ignore accessibility.

Requirements:

- Reduced motion.
- High contrast mode.
- Keyboard navigation.
- Screen reader compatibility plan.
- Large cursor support.
- Text scaling support.
- Touch target scaling.
- Color contrast validation.
- Configurable animation intensity.
- Avoid relying only on transparency.

Accessibility support can mature over time, but config and architecture must not block it.

---

## 26. Keybindings

### 26.1 Default Keybindings

Example defaults:

```toml
"Super+Space" = "launcher.open"
"Super+Q" = "window.close"
"Super+Tab" = "window.switch.next"
"Super+Shift+Tab" = "window.switch.previous"
"Super+1" = "workspace.switch.1"
"Super+2" = "workspace.switch.2"
"Super+3" = "workspace.switch.3"
"Super+4" = "workspace.switch.4"
"Super+Shift+1" = "window.move_to_workspace.1"
"Super+Return" = "terminal.open"
"Super+E" = "files.open"
"Super+L" = "session.lock"
"Super+Esc" = "recovery.open"
"Super+Shift+R" = "shell.restart"
```

The terminal/file commands should be configurable because Staccato does not ship its own apps.

### 26.2 Mode-Specific Keybindings

Profiles may override keybindings.

Example Browser Mode:

```toml
"Super+T" = "tabs.new"
"Super+W" = "tabs.close"
"Super+Shift+]" = "tabs.next"
"Super+Shift+[" = "tabs.previous"
"Super+Backslash" = "split.vertical"
"Super+Minus" = "split.horizontal"
```

Example Tiling Mode:

```toml
"Super+H" = "focus.left"
"Super+J" = "focus.down"
"Super+K" = "focus.up"
"Super+L" = "focus.right"
"Super+Shift+H" = "window.move.left"
"Super+Shift+J" = "window.move.down"
"Super+Shift+K" = "window.move.up"
"Super+Shift+L" = "window.move.right"
```

---

## 27. Window Rules

### 27.1 Purpose

Window rules allow profile/mode behavior to be predictable.

### 27.2 Example Rules

```toml
[[rules]]
app_id = "org.wezfurlong.wezterm"
workspace = "dev"
placement = "bottom-split"

[[rules]]
app_id = "firefox"
workspace = "dev"
placement = "right-split"

[[rules]]
app_id = "pavucontrol"
floating = true
width = 720
height = 480

[[rules]]
title_contains = "Picture-in-Picture"
floating = true
always_on_top = true
```

### 27.3 Matching Fields

Rules may match:

- app_id
- title
- title_contains
- class
- pid
- is_xwayland
- window role/type
- transient parent
- workspace
- profile
- mode

---

## 28. Desktop Services

Staccato should integrate with existing Linux services instead of reinventing everything.

### 28.1 Required Eventually

- D-Bus session.
- Notification daemon.
- Polkit agent.
- Secret service integration.
- XDG autostart.
- XDG portals.
- Power management integration.
- Audio integration through PipeWire/WirePlumber/Pulse compatibility.
- NetworkManager integration.
- Bluetooth integration.
- systemd user units where available.

### 28.2 Configurable App Defaults

Because Staccato does not ship apps, users must configure:

```toml
[default_apps]
terminal = "foot"
file_manager = "nautilus"
browser = "firefox"
settings = "staccato-settings"
```

Settings app may not exist at first. CLI/config must be enough.

---

## 29. Lock Screen

Staccato should eventually include a secure lock screen.

Requirements:

- Lock screen is compositor-level trusted UI.
- Normal clients must not render above it.
- Screen capture disabled while locked.
- Blur sampling must not leak locked content.
- Password prompt secure.
- Session unlock through PAM.
- Suspend/resume behavior.

Initial MVP may omit full lock screen, but architecture must reserve secure layer.

---

## 30. Performance Goals

Staccato should feel fast.

Targets:

- Smooth 60Hz minimum.
- Support high refresh rates.
- Avoid shell jank.
- Keep compositor stable.
- Keep memory reasonable.
- Avoid full-screen redraws unnecessarily.
- Make blur scalable.
- Provide performance controls.

Performance modes:

```toml
[performance]
mode = "balanced" # quality | balanced | performance | battery
blur_quality = "balanced"
animations = true
reduce_effects_on_battery = false
```

---

## 31. Development Milestones

This section is for Codex/implementation ordering. It does not remove scope. It defines build order.

### Milestone 1: Project Skeleton

Build:

- Rust workspace.
- Crates listed above.
- Basic README.
- `staccato.md`.
- Logging.
- CLI argument parsing.
- Basic config crate.
- Placeholder `staccatoctl`.

Done when:

```sh
cargo build
cargo test
```

works.

### Milestone 2: Nested Baton Prototype

Build:

- Smithay compositor skeleton.
- Nested Wayland backend.
- Create Wayland socket.
- Accept basic clients.
- Render simple background.
- Basic input.
- Basic xdg-shell toplevel support.
- Map/unmap windows.
- Focus windows.
- Move windows with modifier drag.

Done when:

```sh
cargo run -p baton -- --nested
WAYLAND_DISPLAY=<printed-socket> foot
```

opens a visible terminal in nested Staccato.

### Milestone 3: Basic Layout Engine

Build:

- Workspace model.
- Window model.
- Floating layout.
- Split tree.
- Tab stack.
- Serialization tests.
- Basic mode trait.
- Classic Mode implementation.

Done when windows are represented in layout state and can move between workspaces.

### Milestone 4: Shell Placeholder

Build:

- Launch `staccato-shell`.
- Shell connects to Baton IPC.
- Render simple panel or dock.
- Show clock.
- Show workspace names.
- Click workspace to switch.
- Shell restart works.

Done when shell can be killed/restarted without killing Baton.

### Milestone 5: Config and staccatoctl

Build:

- XDG config loading.
- Config validation.
- Profiles loading.
- `staccatoctl status`.
- `staccatoctl reload`.
- `staccatoctl profiles list`.
- `staccatoctl workspaces list`.
- `staccatoctl workspaces set-profile`.
- JSON output.

Done when profile changes can be applied through CLI.

### Milestone 6: Real Session Backend

Build:

- DRM/KMS backend.
- libinput input.
- session handling.
- display manager session file.
- XWayland.
- basic monitor support.
- crash logging.

Done when Staccato can run from a TTY/display manager and launch Wayland/XWayland apps.

### Milestone 7: Materials and Blur

Build:

- Luca material.
- Maris material.
- compositor blur pipeline.
- rounded blur regions.
- damage tracking for blur.
- shell surfaces using internal material API.
- effect fallback.
- debug overlay counters.
- `ext-background-effect-v1` server support.

Done when shell panel/dock/sidebar can use real compositor blur and a test client can request background effect blur.

### Milestone 8: Modes

Build:

- Dock Mode.
- Panel Mode.
- Tiling Mode.
- Browser Mode basic.
- Focus Mode basic.
- Per-workspace profile switching.
- Mode-specific chrome.

Done when switching workspaces can switch shell layout.

### Milestone 9: Browser Mode Full Pass

Build:

- Sidebar.
- Tabs.
- Tab stacks.
- Splits.
- Groups.
- Pinned apps.
- Session restore.
- Browser Mode keybindings.
- Drag/drop tab operations.

Done when a dev workspace can use apps as tabs/splits/groups.

### Milestone 10: Recovery and Safe Mode

Build:

- Crash loop detection.
- Safe mode.
- Config rollback.
- Disable user extensions/patches.
- Recovery UI.
- Emergency shortcuts.

Done when broken config does not brick the desktop.

### Milestone 11: Extensions and AI Patch Surface

Build:

- Extension manifests.
- Profile/material patch format.
- Patch validation.
- Patch apply/revert.
- `staccatoctl patches`.
- AI provider abstraction skeleton.
- Safe proposal format.

Done when an AI/tool can generate a profile patch that is validated, previewed, applied, and reverted.

---

## 32. Minimal First Codex Goal

If Codex is starting from zero, the first task should be narrow.

Use this exact implementation goal first:

```txt
Create the initial Staccato Rust workspace and implement a Baton nested Wayland compositor prototype using Smithay.

Requirements:
- Create a Cargo workspace with crates for baton, staccato-layout, staccato-config, staccato-ipc, staccato-shell, staccato-session, and staccatoctl.
- Implement only enough functionality for the first nested compositor prototype.
- Baton must run with `cargo run -p baton -- --nested`.
- Baton must create a Wayland socket and print the `WAYLAND_DISPLAY` value to use.
- Baton must accept xdg-shell toplevel clients.
- Baton must render a basic background.
- Baton must map/unmap client windows.
- Baton must support basic pointer and keyboard focus.
- Baton must support moving windows with a modifier-drag or simple hardcoded key/mouse behavior.
- Add structured modules for backend, input, output, protocols, render, window, and recovery.
- Add logging with useful debug output.
- Add a placeholder config loader that reads `~/.config/staccato/config.toml` if present and falls back to defaults.
- Add `staccatoctl status` as a placeholder command that can later connect over IPC.
- Add a README with build/run/test instructions.
- Do not implement the full shell yet.
- Do not implement AI yet.
- Do not implement the distro.
- Keep architecture compatible with the full staccato.md spec.
```

This is not the full project scope. It is only the first build step.

---

## 33. Definition of “Usable Enough to Test”

Staccato is testable when:

- It runs nested.
- It opens at least one terminal app.
- Input works.
- Windows can be moved/resized.
- Workspaces exist.
- Shell placeholder appears.
- Logs are readable.
- Config loads.
- It does not require logging out to test every change.

Staccato is usable as an experimental DE when:

- It runs as a real session.
- Wayland apps work.
- XWayland apps work.
- Basic shell works.
- App launching works.
- Workspaces work.
- Recovery exists.
- Safe mode exists.
- Effects can be disabled if broken.

Staccato is actually Staccato when:

- Baton has compositor blur.
- Luca/Maris materials work.
- Workspace profiles work.
- Modes work.
- Browser Mode works.
- The shell can reshape per workspace.
- Configuration is safe, editable, reloadable, and revertible.

---

## 34. Design Principles

1. **The compositor must be stable.**
   The shell can be expressive; Baton must be boring where stability matters.

2. **Modes must share primitives.**
   Do not implement five separate desktops. Implement one layout engine and multiple policies.

3. **Blur must be compositor-level.**
   Fake transparency is not enough.

4. **The desktop must work without AI.**
   AI is a customization tool, not a dependency.

5. **Config must be human-editable.**
   A settings app can come later, but files and CLI are required.

6. **Safe mode is a product feature.**
   A hackable desktop without recovery is just a footgun.

7. **Nested testing is mandatory.**
   Development must not require destroying the active session every time.

8. **Do not build the distro first.**
   Staccato must be usable on existing Linux systems.

9. **Do not require custom apps.**
   Staccato is the DE. Apps are separate.

10. **The visual identity matters.**
    Staccato should not look like stock GNOME, old KDE, or random Hyprland rice.

---

## 35. Final Target

The final Staccato experience should feel like this:

```txt
A fast Wayland desktop where every workspace can have its own shape.

One workspace can feel like a polished dock-based desktop.
Another can feel like a browser with app tabs and splits.
Another can be keyboard-first tiling.
Another can hide everything for focus.

The compositor provides real blur and materials.
The shell is modular.
Profiles are editable.
The CLI is scriptable.
The system is recoverable.
AI/tools can safely generate customizations later.

It is not a distro.
It is not an app suite.
It is the desktop layer.
```
