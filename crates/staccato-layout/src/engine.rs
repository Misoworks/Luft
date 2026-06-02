use crate::{
    LayoutContext, ModeContext, ProfileId, ShellAction, WindowId, WindowInfo, WindowState,
    Workspace, WorkspaceId,
    mode::{mode_for_profile, shell_mode, state_for_mode, window_geometry_map},
};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct LayoutEngine {
    workspaces: BTreeMap<WorkspaceId, Workspace>,
    windows: BTreeMap<WindowId, WindowInfo>,
    active_workspace: WorkspaceId,
    next_window_id: u64,
    bounds: crate::Rect,
}

impl LayoutEngine {
    pub fn new(
        workspaces: Vec<Workspace>,
        active_workspace: WorkspaceId,
    ) -> Result<Self, LayoutError> {
        let workspaces = workspaces
            .into_iter()
            .map(|workspace| (workspace.id.clone(), workspace))
            .collect::<BTreeMap<_, _>>();

        if !workspaces.contains_key(&active_workspace) {
            return Err(LayoutError::UnknownWorkspace(active_workspace));
        }

        Ok(Self {
            workspaces,
            windows: BTreeMap::new(),
            active_workspace,
            next_window_id: 1,
            bounds: crate::Rect::new(0, 0, 1280, 800),
        })
    }

    pub fn with_default_workspaces() -> Self {
        Self::new(
            vec![Workspace::empty("1", "Workspace 1", "panel-default")],
            WorkspaceId("1".to_string()),
        )
        .expect("default layout has a valid active workspace")
    }

    pub fn active_workspace(&self) -> &WorkspaceId {
        &self.active_workspace
    }

    pub fn active_mode(&self) -> crate::ModeId {
        self.workspaces
            .get(&self.active_workspace)
            .map(|workspace| mode_for_profile(&workspace.profile_id))
            .unwrap_or(crate::ModeId::Dock)
    }

    pub fn workspaces(&self) -> impl Iterator<Item = &Workspace> {
        self.ordered_workspace_ids()
            .into_iter()
            .filter_map(|id| self.workspaces.get(&id))
    }

    pub fn ensure_workspace(
        &mut self,
        id: WorkspaceId,
        name: String,
        profile: ProfileId,
    ) -> &Workspace {
        self.workspaces
            .entry(id.clone())
            .or_insert_with(|| Workspace::empty(id.0.clone(), name, profile.0.clone()))
    }

    pub fn relative_workspace(&mut self, offset: i32) -> Option<WorkspaceId> {
        self.normalize_dynamic_workspaces();
        let workspaces = self.ordered_workspace_ids();
        let current = workspaces
            .iter()
            .position(|workspace| workspace == &self.active_workspace)?;
        let next = current as i32 + offset;
        if next < 0 {
            return None;
        }
        if let Some(workspace) = workspaces.get(next as usize) {
            return Some(workspace.clone());
        }
        if offset <= 0 || self.workspace_is_empty(&self.active_workspace) {
            return None;
        }

        let id = self.next_numeric_workspace_id();
        let profile = self
            .workspaces
            .get(&self.active_workspace)
            .map(|workspace| workspace.profile_id.clone())
            .unwrap_or_else(|| ProfileId("dock-default".to_string()));
        self.ensure_workspace(id.clone(), format!("Workspace {}", id.0), profile);
        Some(id)
    }

    pub fn window(&self, id: WindowId) -> Option<&WindowInfo> {
        self.windows.get(&id)
    }

    pub fn set_window_geometry(
        &mut self,
        id: WindowId,
        geometry: crate::Rect,
    ) -> Result<(), LayoutError> {
        let window = self
            .windows
            .get_mut(&id)
            .ok_or(LayoutError::UnknownWindow(id))?;
        window.geometry = geometry;
        Ok(())
    }

    pub fn set_window_state(
        &mut self,
        id: WindowId,
        state: WindowState,
    ) -> Result<(), LayoutError> {
        let window = self
            .windows
            .get_mut(&id)
            .ok_or(LayoutError::UnknownWindow(id))?;
        window.state = state;
        Ok(())
    }

    pub fn set_bounds(&mut self, bounds: crate::Rect) {
        self.bounds = bounds;
    }

    pub fn register_window(&mut self, mut info: WindowInfo) -> Result<WindowId, LayoutError> {
        if !self.workspaces.contains_key(&info.workspace) {
            return Err(LayoutError::UnknownWorkspace(info.workspace));
        }

        if info.id.0 == 0 {
            info.id = self.allocate_window_id();
        } else {
            self.next_window_id = self.next_window_id.max(info.id.0 + 1);
        }

        let mode = self.mode_for_workspace(&info.workspace)?;
        info.state = state_for_mode(mode);
        let id = info.id;
        let workspace = self
            .workspaces
            .get_mut(&info.workspace)
            .ok_or_else(|| LayoutError::UnknownWorkspace(info.workspace.clone()))?;
        shell_mode(mode).on_window_opened(id, workspace, &mut ModeContext::default());
        self.windows.insert(id, info);
        self.normalize_dynamic_workspaces();
        Ok(id)
    }

    pub fn unregister_window(&mut self, id: WindowId) -> Option<WindowInfo> {
        let window = self.windows.remove(&id)?;
        if let Some(workspace) = self.workspaces.get_mut(&window.workspace) {
            let mode = mode_for_profile(&workspace.profile_id);
            let _ = shell_mode(mode).handle_action(
                ShellAction::MoveWindowToWorkspace {
                    window: id,
                    workspace: window.workspace.clone(),
                },
                workspace,
                &mut ModeContext::default(),
            );
        }
        self.normalize_dynamic_workspaces();
        Some(window)
    }

    pub fn switch_workspace(&mut self, workspace: &WorkspaceId) -> Result<(), LayoutError> {
        if !self.workspaces.contains_key(workspace) {
            return Err(LayoutError::UnknownWorkspace(workspace.clone()));
        }

        self.active_workspace = workspace.clone();
        Ok(())
    }

    pub fn set_workspace_profile(
        &mut self,
        workspace: &WorkspaceId,
        profile: ProfileId,
    ) -> Result<(), LayoutError> {
        let mode = mode_for_profile(&profile);
        let workspace = self
            .workspaces
            .get_mut(workspace)
            .ok_or_else(|| LayoutError::UnknownWorkspace(workspace.clone()))?;
        workspace.profile_id = profile;
        for window in &workspace.floating_windows {
            if let Some(info) = self.windows.get_mut(window) {
                info.state = state_for_mode(mode);
            }
        }
        Ok(())
    }

    pub fn move_window_to_workspace(
        &mut self,
        window: WindowId,
        workspace_id: &WorkspaceId,
    ) -> Result<(), LayoutError> {
        if !self.workspaces.contains_key(workspace_id) {
            return Err(LayoutError::UnknownWorkspace(workspace_id.clone()));
        }

        let current_workspace = self
            .windows
            .get(&window)
            .ok_or(LayoutError::UnknownWindow(window))?
            .workspace
            .clone();

        if let Some(workspace) = self.workspaces.get_mut(&current_workspace) {
            let mode = mode_for_profile(&workspace.profile_id);
            let _ = shell_mode(mode).handle_action(
                ShellAction::MoveWindowToWorkspace {
                    window,
                    workspace: workspace_id.clone(),
                },
                workspace,
                &mut ModeContext::default(),
            );
        }

        let target = self
            .workspaces
            .get_mut(workspace_id)
            .ok_or_else(|| LayoutError::UnknownWorkspace(workspace_id.clone()))?;
        let mode = mode_for_profile(&target.profile_id);
        shell_mode(mode).on_window_opened(window, target, &mut ModeContext::default());

        if let Some(info) = self.windows.get_mut(&window) {
            info.workspace = workspace_id.clone();
            info.state = state_for_mode(mode);
        }

        self.normalize_dynamic_workspaces();
        Ok(())
    }

    pub fn arrange_active(&self) -> Result<crate::Arrangement, LayoutError> {
        let workspace = self
            .workspaces
            .get(&self.active_workspace)
            .ok_or_else(|| LayoutError::UnknownWorkspace(self.active_workspace.clone()))?;
        let geometries = window_geometry_map(&self.windows);
        let context = LayoutContext {
            bounds: self.bounds,
            windows: &self.windows,
            window_geometries: &geometries,
        };
        let mode = mode_for_profile(&workspace.profile_id);
        Ok(shell_mode(mode).arrange(workspace, &context))
    }

    fn allocate_window_id(&mut self) -> WindowId {
        let id = WindowId(self.next_window_id);
        self.next_window_id += 1;
        id
    }

    fn normalize_dynamic_workspaces(&mut self) {
        self.prune_extra_trailing_empty_workspaces();
        self.ensure_trailing_empty_workspace();
    }

    fn ensure_trailing_empty_workspace(&mut self) {
        if self.windows.is_empty() {
            return;
        }

        let Some(last) = self.ordered_workspace_ids().last().cloned() else {
            return;
        };
        if self.workspace_is_empty(&last) {
            return;
        }

        let id = self.next_numeric_workspace_id();
        let profile = self
            .workspaces
            .get(&last)
            .map(|workspace| workspace.profile_id.clone())
            .unwrap_or_else(|| ProfileId("dock-default".to_string()));
        self.ensure_workspace(id.clone(), format!("Workspace {}", id.0), profile);
    }

    fn prune_extra_trailing_empty_workspaces(&mut self) {
        loop {
            let ids = self.ordered_workspace_ids();
            if ids.len() <= 1 {
                return;
            }

            let Some(last) = ids.last() else {
                return;
            };
            let previous = &ids[ids.len() - 2];
            if last == &self.active_workspace
                && self.workspace_is_empty(last)
                && self.workspace_is_empty(previous)
            {
                self.active_workspace = previous.clone();
                self.workspaces.remove(last);
                continue;
            }

            if last == &self.active_workspace
                || !self.workspace_is_empty(last)
                || !self.workspace_is_empty(previous)
            {
                return;
            }
            self.workspaces.remove(last);
        }
    }

    fn workspace_is_empty(&self, workspace: &WorkspaceId) -> bool {
        self.windows
            .values()
            .all(|window| &window.workspace != workspace)
    }

    fn ordered_workspace_ids(&self) -> Vec<WorkspaceId> {
        let mut ids = self.workspaces.keys().cloned().collect::<Vec<_>>();
        ids.sort_by(compare_workspace_ids);
        ids
    }

    fn next_numeric_workspace_id(&self) -> WorkspaceId {
        let next = self
            .workspaces
            .keys()
            .filter_map(|workspace| workspace.0.parse::<u32>().ok())
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        WorkspaceId(next.to_string())
    }

    fn mode_for_workspace(&self, workspace: &WorkspaceId) -> Result<crate::ModeId, LayoutError> {
        let workspace = self
            .workspaces
            .get(workspace)
            .ok_or_else(|| LayoutError::UnknownWorkspace(workspace.clone()))?;
        Ok(mode_for_profile(&workspace.profile_id))
    }
}

fn compare_workspace_ids(left: &WorkspaceId, right: &WorkspaceId) -> std::cmp::Ordering {
    match (left.0.parse::<u32>(), right.0.parse::<u32>()) {
        (Ok(left), Ok(right)) => left.cmp(&right),
        (Ok(_), Err(_)) => std::cmp::Ordering::Less,
        (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
        (Err(_), Err(_)) => left.0.cmp(&right.0),
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LayoutError {
    #[error("unknown workspace {0:?}")]
    UnknownWorkspace(WorkspaceId),
    #[error("unknown window {0:?}")]
    UnknownWindow(WindowId),
}

#[cfg(test)]
mod tests;
