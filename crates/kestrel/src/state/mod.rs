use crate::{
    layout_config::layout_from_config,
    output::{NestedOutput, OutputDescriptor, OutputGraph},
    protocol_state::ProtocolState,
    titlebar::TitlebarCache,
    window::{WindowGrab, WindowStack},
    workspace_transition::WorkspaceTransition,
};
use luft_config::LuftConfig;
use luft_ipc::{LayoutEngine, LayoutError, Rect, WindowId, WindowInfo, WorkspaceId};
use luft_ipc::{ShellStatus, XwaylandStatus};
use smithay::{
    backend::allocator::format::FormatSet,
    desktop::PopupManager,
    input::{
        Seat, SeatState,
        keyboard::{KeyboardHandle, LedState},
        pointer::{CursorIcon, CursorImageStatus},
    },
    reexports::{
        wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode,
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::{DisplayHandle, protocol::wl_surface::WlSurface},
    },
    utils::{Logical, Point},
    wayland::{
        compositor::CompositorState,
        selection::{data_device::DataDeviceState, primary_selection::PrimarySelectionState},
        shell::{
            wlr_layer::WlrLayerShellState,
            xdg::{ToplevelSurface, XdgShellState},
        },
        shm::ShmState,
    },
};
use std::{cell::RefCell, path::PathBuf};
use tracing::debug;

mod frame_callbacks;
mod input;
mod layers;
mod output_state;
mod scene;
mod session;
mod shell_control;

pub struct KestrelState {
    pub display_handle: DisplayHandle,
    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub protocol_state: ProtocolState,
    pub layer_shell_state: WlrLayerShellState,
    pub shm_state: ShmState,
    pub seat_state: SeatState<Self>,
    pub data_device_state: DataDeviceState,
    pub primary_selection_state: PrimarySelectionState,
    pub seat: Seat<Self>,
    pub keyboard: Option<KeyboardHandle<Self>>,
    pub outputs: OutputGraph,
    pub layout: LayoutEngine,
    pub windows: WindowStack,
    pub popup_manager: PopupManager,
    pub pointer_location: Point<f64, Logical>,
    pub drag: Option<WindowGrab>,
    pub pending_window_drag: Option<PendingWindowDrag>,
    pub pending_client_grab: Option<ClientGrabSerial>,
    pub config: LuftConfig,
    pub cursor_image: CursorImageStatus,
    pub cursor_dirty: bool,
    pub frame_cursor_active: bool,
    pub super_active: bool,
    pub super_used: bool,
    pub shell_control_path: Option<PathBuf>,
    pub shell_status: ShellStatus,
    shell_restart_requested: bool,
    pub xwayland_status: XwaylandStatus,
    pub xwayland_display: Option<String>,
    pub titlebar_cache: RefCell<TitlebarCache>,
    pub dmabuf_formats: FormatSet,
    #[cfg(feature = "session-backend")]
    pending_syncobj_sources: Vec<session::PendingSyncobjSource>,
    pending_keyboard_led_state: Option<LedState>,
    scene_dirty: bool,
    workspace_transition: Option<WorkspaceTransition>,
    serial: u32,
}

impl KestrelState {
    pub fn new(display: &DisplayHandle, config: LuftConfig) -> Self {
        Self::new_for_output(display, config, NestedOutput::default().descriptor())
    }

    pub fn new_for_output(
        display: &DisplayHandle,
        config: LuftConfig,
        output_descriptor: OutputDescriptor,
    ) -> Self {
        Self::new_for_outputs(display, config, vec![output_descriptor])
    }

    pub fn new_for_outputs(
        display: &DisplayHandle,
        config: LuftConfig,
        output_descriptors: Vec<OutputDescriptor>,
    ) -> Self {
        let compositor_state = CompositorState::new_v6::<Self>(display);
        let xdg_shell_state = XdgShellState::new::<Self>(display);
        let protocol_state = ProtocolState::new(display);
        let layer_shell_state = WlrLayerShellState::new::<Self>(display);
        let shm_state = ShmState::new::<Self>(display, vec![]);
        let data_device_state = DataDeviceState::new::<Self>(display);
        let primary_selection_state = PrimarySelectionState::new::<Self>(display);
        let mut seat_state = SeatState::new();
        let seat = seat_state.new_wl_seat(display, "luft-seat");
        let outputs = OutputGraph::new(display, &config.display, output_descriptors);
        let mut layout = layout_from_config(&config);
        let output_size = outputs.primary_size();
        layout.set_bounds(Rect::new(0, 0, output_size.w, output_size.h));

        Self {
            display_handle: display.clone(),
            compositor_state,
            xdg_shell_state,
            protocol_state,
            layer_shell_state,
            shm_state,
            seat_state,
            data_device_state,
            primary_selection_state,
            seat,
            keyboard: None,
            outputs,
            layout,
            windows: WindowStack::default(),
            popup_manager: PopupManager::default(),
            pointer_location: (0.0, 0.0).into(),
            drag: None,
            pending_window_drag: None,
            pending_client_grab: None,
            config,
            cursor_image: CursorImageStatus::Named(CursorIcon::Default),
            cursor_dirty: true,
            frame_cursor_active: false,
            super_active: false,
            super_used: false,
            shell_control_path: None,
            shell_status: ShellStatus::NotStarted,
            shell_restart_requested: false,
            xwayland_status: XwaylandStatus::Disabled,
            xwayland_display: None,
            titlebar_cache: RefCell::new(TitlebarCache::default()),
            dmabuf_formats: FormatSet::default(),
            #[cfg(feature = "session-backend")]
            pending_syncobj_sources: Vec::new(),
            pending_keyboard_led_state: None,
            scene_dirty: true,
            workspace_transition: None,
            serial: 1,
        }
    }

    pub fn map_toplevel(&mut self, surface: ToplevelSurface) {
        let parent = self.parent_window_for_toplevel(&surface);
        let workspace = parent
            .and_then(|id| {
                self.windows
                    .window(id)
                    .map(|window| window.workspace.clone())
            })
            .unwrap_or_else(|| self.layout.active_workspace().clone());
        let geometry = parent
            .map(|id| self.next_transient_window_geometry_for_size(id, (900, 560).into()))
            .unwrap_or_else(|| self.next_initial_window_geometry());
        let info = WindowInfo::new(luft_ipc::WindowId(0), workspace.clone(), geometry);

        match self.layout.register_window(info) {
            Ok(id) => {
                let requested_server_decoration = surface
                    .with_pending_state(|state| state.decoration_mode == Some(Mode::ServerSide));
                self.windows.add(
                    id,
                    workspace,
                    surface.clone(),
                    geometry,
                    requested_server_decoration,
                    true,
                );
                if let Some(parent) = parent {
                    self.raise_transient(parent, id);
                }
                self.enter_output(surface.wl_surface());
                self.mark_scene_dirty();
            }
            Err(error) => debug!(?error, "failed to register toplevel in layout"),
        }
    }

    pub fn adopt_initial_toplevel_size(&mut self, surface: &WlSurface) -> bool {
        let Some(id) = self.windows.id_for_wl_surface(surface) else {
            return false;
        };
        if !self.windows.initial_size_pending(id) {
            return false;
        }
        let Some(size) = self.windows.committed_surface_size(id) else {
            return false;
        };

        let geometry = self
            .windows
            .window(id)
            .and_then(|window| self.parent_window_for_toplevel(&window.surface))
            .map(|parent| self.next_transient_window_geometry_for_size(parent, size))
            .unwrap_or_else(|| self.next_initial_window_geometry_for_size(size));
        let Some((_surface, geometry)) = self.windows.set_geometry(id, geometry) else {
            return false;
        };

        self.windows.set_initial_size_pending(id, false);
        let _ = self.layout.set_window_geometry(id, geometry);
        self.apply_active_arrangement();
        true
    }

    pub fn sync_toplevel_parent(&mut self, surface: &ToplevelSurface) {
        let Some(id) = self.windows.id_for_surface(surface) else {
            return;
        };
        let Some(parent) = self.parent_window_for_toplevel(surface) else {
            return;
        };
        if let Some(workspace) = self
            .windows
            .window(parent)
            .map(|window| window.workspace.clone())
        {
            let _ = self.layout.move_window_to_workspace(id, &workspace);
            self.windows.set_workspace(id, workspace);
        }
        self.raise_transient(parent, id);
        self.mark_scene_dirty();
    }

    pub fn unmap_toplevel(&mut self, surface: &ToplevelSurface) {
        self.dismiss_popups_for_surface(surface.wl_surface());
        self.leave_output(surface.wl_surface());
        if let Some(window) = self.windows.remove(surface) {
            self.layout.unregister_window(window.id);
            self.apply_active_arrangement();
            self.mark_scene_dirty();
        }
    }

    pub fn remove_dead_windows(&mut self) -> bool {
        let removed = self.windows.retain_alive();
        for id in &removed {
            self.layout.unregister_window(*id);
        }
        if !removed.is_empty() {
            self.apply_active_arrangement();
            self.mark_scene_dirty();
        }
        !removed.is_empty()
    }

    pub fn active_window(&self) -> Option<WindowId> {
        self.windows
            .topmost_on_workspace(self.layout.active_workspace())
    }

    pub fn activate_window(
        &mut self,
        keyboard: &KeyboardHandle<Self>,
        id: WindowId,
    ) -> Result<(), LayoutError> {
        let managed = self
            .windows
            .window(id)
            .cloned()
            .ok_or(LayoutError::UnknownWindow(id))?;

        if managed.closing {
            return Err(LayoutError::UnknownWindow(id));
        }
        if managed.hidden {
            self.show_window(id)?;
        }
        if &managed.workspace != self.layout.active_workspace() {
            self.switch_layout_workspace(&managed.workspace)?;
        }
        let surface = self
            .windows
            .raise_by_id(id)
            .unwrap_or_else(|| managed.surface.clone());
        self.set_activated_window(id);
        let serial = self.next_serial();
        keyboard.set_focus(self, Some(surface.wl_surface().clone()), serial);
        self.mark_scene_dirty();
        Ok(())
    }

    pub fn activate_surface(
        &mut self,
        keyboard: &KeyboardHandle<Self>,
        surface: &ToplevelSurface,
    ) -> bool {
        let Some(id) = self.windows.id_for_surface(surface) else {
            return false;
        };

        self.activate_window(keyboard, id).is_ok()
    }

    pub fn cycle_active_window(
        &mut self,
        keyboard: &KeyboardHandle<Self>,
        previous: bool,
    ) -> Option<WindowId> {
        let workspace = self.layout.active_workspace().clone();
        let (id, surface) = self.windows.cycle_on_workspace(&workspace, previous)?;
        self.set_activated_window(id);
        let serial = self.next_serial();
        keyboard.set_focus(self, Some(surface.wl_surface().clone()), serial);
        self.mark_scene_dirty();
        Some(id)
    }

    pub fn close_window(&mut self, id: WindowId) -> Result<(), LayoutError> {
        if self.windows.window(id).is_none() {
            return Err(LayoutError::UnknownWindow(id));
        }
        if let Some(surface) = self.windows.start_close(id, true) {
            surface.send_close();
        }
        self.mark_scene_dirty();
        Ok(())
    }

    pub fn close_active_window(&mut self) -> Option<WindowId> {
        let id = self.active_window()?;
        self.close_window(id).ok()?;
        Some(id)
    }

    pub fn send_finished_window_closes(&mut self) -> bool {
        let surfaces = self.windows.drain_close_requests();
        for surface in &surfaces {
            surface.send_close();
        }
        if !surfaces.is_empty() {
            self.mark_scene_dirty();
        }
        !surfaces.is_empty()
    }

    pub fn move_window_to_workspace(
        &mut self,
        id: WindowId,
        workspace: WorkspaceId,
    ) -> Result<(), LayoutError> {
        if self.windows.window(id).is_none() {
            return Err(LayoutError::UnknownWindow(id));
        }

        self.layout.move_window_to_workspace(id, &workspace)?;
        self.windows.set_workspace(id, workspace);
        self.apply_active_arrangement();
        self.mark_scene_dirty();
        Ok(())
    }

    pub fn move_active_window_to_workspace(
        &mut self,
        keyboard: &KeyboardHandle<Self>,
        workspace: WorkspaceId,
    ) -> Option<WindowId> {
        let current_workspace = self.layout.active_workspace().clone();
        let id = self.active_window()?;

        self.move_window_to_workspace(id, workspace.clone()).ok()?;
        if workspace == current_workspace {
            self.activate_window(keyboard, id).ok()?;
            return Some(id);
        }

        self.focus_active_workspace(keyboard);
        Some(id)
    }

    pub fn switch_workspace(
        &mut self,
        keyboard: &KeyboardHandle<Self>,
        workspace: &WorkspaceId,
    ) -> Result<(), LayoutError> {
        self.switch_layout_workspace(workspace)?;
        self.focus_active_workspace(keyboard);
        Ok(())
    }

    pub fn switch_relative_workspace(
        &mut self,
        keyboard: &KeyboardHandle<Self>,
        offset: i32,
    ) -> Result<(), LayoutError> {
        let Some(workspace) = self.layout.relative_workspace(offset) else {
            return Ok(());
        };
        self.switch_workspace(keyboard, &workspace)
    }

    fn switch_layout_workspace(&mut self, workspace: &WorkspaceId) -> Result<(), LayoutError> {
        let from = self.layout.active_workspace().clone();
        self.layout.switch_workspace(workspace)?;
        self.apply_active_arrangement();
        if from != *workspace {
            self.workspace_transition = self
                .workspace_transition_direction(&from, workspace)
                .map(|direction| WorkspaceTransition::new(from, workspace.clone(), direction));
            self.mark_scene_dirty();
        }
        Ok(())
    }

    pub(crate) fn focus_active_workspace(&mut self, keyboard: &KeyboardHandle<Self>) {
        if let Some(id) = self.active_window() {
            let _ = self.activate_window(keyboard, id);
            return;
        }

        self.clear_activated_windows();
        let serial = self.next_serial();
        keyboard.set_focus(self, None, serial);
    }

    fn set_activated_window(&self, active: WindowId) {
        for managed in self.windows.iter() {
            managed.surface.with_pending_state(|surface_state| {
                if managed.id == active {
                    surface_state.states.set(xdg_toplevel::State::Activated);
                } else {
                    surface_state.states.unset(xdg_toplevel::State::Activated);
                }
            });
            managed.surface.send_pending_configure();
        }
    }

    fn clear_activated_windows(&self) {
        for managed in self.windows.iter() {
            managed.surface.with_pending_state(|surface_state| {
                surface_state.states.unset(xdg_toplevel::State::Activated);
            });
            managed.surface.send_pending_configure();
        }
    }

    fn workspace_transition_direction(&self, from: &WorkspaceId, to: &WorkspaceId) -> Option<i32> {
        let workspaces = self
            .layout
            .workspaces()
            .map(|workspace| workspace.id.clone())
            .collect::<Vec<_>>();
        let from_index = workspaces.iter().position(|workspace| workspace == from)?;
        let to_index = workspaces.iter().position(|workspace| workspace == to)?;
        let len = workspaces.len();
        if len <= 1 || from_index == to_index {
            return None;
        }

        let forward = (to_index + len - from_index) % len;
        let backward = (from_index + len - to_index) % len;
        if forward <= backward {
            Some(1)
        } else {
            Some(-1)
        }
    }

    fn parent_window_for_toplevel(&self, surface: &ToplevelSurface) -> Option<WindowId> {
        let parent = surface.parent()?;
        self.windows.id_for_wl_surface(&parent)
    }

    fn raise_transient(&mut self, parent: WindowId, child: WindowId) {
        let _ = self.windows.raise_by_id(parent);
        let _ = self.windows.raise_by_id(child);
    }
}

#[derive(Clone)]
pub struct ClientGrabSerial {
    surface: ToplevelSurface,
}

#[derive(Clone)]
pub struct PendingWindowDrag {
    pub surface: ToplevelSurface,
    pub pointer_start: Point<f64, Logical>,
}
