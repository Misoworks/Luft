use crate::{
    Arrangement, ChromeSpec, LayoutNode, ModeId, ProfileId, Rect, WindowId, WindowInfo, Workspace,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellProfile {
    pub id: crate::ProfileId,
    pub name: String,
    pub mode: ModeId,
    pub chrome: ChromeSpec,
}

impl ShellProfile {
    pub fn panel_default() -> Self {
        Self {
            id: crate::ProfileId("panel-default".to_string()),
            name: "Panel Default".to_string(),
            mode: ModeId::Panel,
            chrome: ChromeSpec::panel_default(),
        }
    }
}

#[derive(Debug)]
pub struct LayoutContext<'a> {
    pub bounds: Rect,
    pub windows: &'a BTreeMap<WindowId, WindowInfo>,
    pub window_geometries: &'a BTreeMap<WindowId, Rect>,
}

#[derive(Debug, Clone, Copy)]
pub struct ModeContext {
    pub bounds: Rect,
    pub default_window_size: (i32, i32),
}

impl Default for ModeContext {
    fn default() -> Self {
        Self {
            bounds: Rect::new(0, 0, 1280, 800),
            default_window_size: (900, 560),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellAction {
    SwitchWorkspace(crate::WorkspaceId),
    MoveWindowToWorkspace {
        window: WindowId,
        workspace: crate::WorkspaceId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionResult {
    Handled,
    Ignored,
}

pub trait ShellMode {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn chrome(&self, profile: &ShellProfile) -> ChromeSpec;
    fn on_window_opened(&self, window: WindowId, workspace: &mut Workspace, ctx: &mut ModeContext);
    fn arrange(&self, workspace: &Workspace, ctx: &LayoutContext<'_>) -> Arrangement;
    fn handle_action(
        &self,
        action: ShellAction,
        workspace: &mut Workspace,
        ctx: &mut ModeContext,
    ) -> ActionResult;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct PanelMode;

impl ShellMode for PanelMode {
    fn id(&self) -> &'static str {
        "panel"
    }

    fn name(&self) -> &'static str {
        "Panel"
    }

    fn chrome(&self, profile: &ShellProfile) -> ChromeSpec {
        profile.chrome.clone()
    }

    fn on_window_opened(
        &self,
        window: WindowId,
        workspace: &mut Workspace,
        _ctx: &mut ModeContext,
    ) {
        if !workspace.floating_windows.contains(&window) {
            workspace.floating_windows.push(window);
        }

        if matches!(workspace.root, LayoutNode::Empty) {
            workspace.root = LayoutNode::Window { window };
        }
    }

    fn arrange(&self, workspace: &Workspace, ctx: &LayoutContext<'_>) -> Arrangement {
        let mut arrangement = Arrangement::empty();

        for (index, window) in workspace.floating_windows.iter().enumerate() {
            if matches!(
                ctx.windows.get(window).map(|info| &info.state),
                Some(crate::WindowState::Hidden)
            ) {
                continue;
            }
            let geometry = ctx
                .window_geometries
                .get(window)
                .copied()
                .unwrap_or_else(|| Rect::cascade(ctx.bounds, index, 900, 560));
            arrangement.windows.insert(*window, geometry);
        }

        arrangement
    }

    fn handle_action(
        &self,
        action: ShellAction,
        workspace: &mut Workspace,
        _ctx: &mut ModeContext,
    ) -> ActionResult {
        match action {
            ShellAction::MoveWindowToWorkspace { window, .. } => {
                workspace.floating_windows.retain(|id| *id != window);
                if matches!(workspace.root, LayoutNode::Window { window: root } if root == window) {
                    workspace.root = LayoutNode::Empty;
                }
                ActionResult::Handled
            }
            ShellAction::SwitchWorkspace(_) => ActionResult::Ignored,
        }
    }
}

pub fn mode_for_profile(_profile: &ProfileId) -> ModeId {
    ModeId::Panel
}

pub fn shell_mode(_mode: ModeId) -> &'static dyn ShellMode {
    static PANEL: PanelMode = PanelMode;
    &PANEL
}

pub fn state_for_mode(_mode: ModeId) -> crate::WindowState {
    crate::WindowState::Floating
}

pub fn window_geometry_map(windows: &BTreeMap<WindowId, WindowInfo>) -> BTreeMap<WindowId, Rect> {
    windows
        .iter()
        .map(|(id, info)| (*id, info.geometry))
        .collect()
}
