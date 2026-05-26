use crate::{
    Arrangement, ChromeSpec, LayoutNode, ModeId, ProfileId, Rect, WindowId, WindowInfo,
    WindowState, Workspace,
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
    pub fn classic_default() -> Self {
        Self {
            id: crate::ProfileId("classic-default".to_string()),
            name: "Classic Default".to_string(),
            mode: ModeId::Classic,
            chrome: ChromeSpec {
                panel: true,
                dock: false,
                sidebar: false,
                overview: true,
                command_palette: true,
            },
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
pub struct ClassicMode;

#[derive(Debug, Default, Clone, Copy)]
pub struct TilingMode;

#[derive(Debug, Default, Clone, Copy)]
pub struct FocusMode;

const CHROME_TOP: i32 = 36;
const CHROME_BOTTOM: i32 = 0;
const CHROME_MARGIN: i32 = 0;
const WINDOW_CHROME_HEIGHT: i32 = 32;
const TILE_GAP: i32 = 8;

impl ShellMode for ClassicMode {
    fn id(&self) -> &'static str {
        "classic"
    }

    fn name(&self) -> &'static str {
        "Classic"
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

impl ShellMode for TilingMode {
    fn id(&self) -> &'static str {
        "tiling"
    }

    fn name(&self) -> &'static str {
        "Tiling"
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
        remember_window(workspace, window);
    }

    fn arrange(&self, workspace: &Workspace, ctx: &LayoutContext<'_>) -> Arrangement {
        let windows = visible_windows(workspace, ctx);
        let area = work_area(ctx.bounds);
        let mut arrangement = Arrangement::empty();
        if windows.is_empty() {
            return arrangement;
        }

        let columns = grid_columns(windows.len());
        let rows = div_ceil(windows.len() as i32, columns);
        let width = ((area.width - TILE_GAP * (columns - 1)) / columns).max(1);
        let height = ((area.height - TILE_GAP * (rows - 1)) / rows).max(1);

        for (index, window) in windows.iter().enumerate() {
            let column = index as i32 % columns;
            let row = index as i32 / columns;
            arrangement.windows.insert(
                **window,
                Rect::new(
                    area.x + column * (width + TILE_GAP),
                    area.y + row * (height + TILE_GAP),
                    width,
                    content_height(height),
                ),
            );
        }
        arrangement
    }

    fn handle_action(
        &self,
        action: ShellAction,
        workspace: &mut Workspace,
        _ctx: &mut ModeContext,
    ) -> ActionResult {
        handle_workspace_action(action, workspace)
    }
}

impl ShellMode for FocusMode {
    fn id(&self) -> &'static str {
        "focus"
    }

    fn name(&self) -> &'static str {
        "Focus"
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
        remember_window(workspace, window);
    }

    fn arrange(&self, workspace: &Workspace, ctx: &LayoutContext<'_>) -> Arrangement {
        let area = focus_area(ctx.bounds);
        let mut arrangement = Arrangement::empty();
        for window in visible_windows(workspace, ctx) {
            arrangement.windows.insert(*window, area);
        }
        arrangement
    }

    fn handle_action(
        &self,
        action: ShellAction,
        workspace: &mut Workspace,
        _ctx: &mut ModeContext,
    ) -> ActionResult {
        handle_workspace_action(action, workspace)
    }
}

pub fn mode_for_profile(profile: &ProfileId) -> ModeId {
    let profile = profile.0.as_str();
    if profile.contains("browser") {
        ModeId::Browser
    } else if profile.contains("tiling") {
        ModeId::Tiling
    } else if profile.contains("focus") {
        ModeId::Focus
    } else if profile.contains("panel") {
        ModeId::Panel
    } else {
        ModeId::Dock
    }
}

pub fn shell_mode(mode: ModeId) -> &'static dyn ShellMode {
    static CLASSIC: ClassicMode = ClassicMode;
    static TILING: TilingMode = TilingMode;
    static FOCUS: FocusMode = FocusMode;

    match mode {
        ModeId::Tiling => &TILING,
        ModeId::Browser | ModeId::Focus | ModeId::Tablet => &FOCUS,
        ModeId::Classic | ModeId::Dock | ModeId::Panel => &CLASSIC,
    }
}

pub fn state_for_mode(mode: ModeId) -> WindowState {
    match mode {
        ModeId::Tiling => WindowState::Tiled,
        ModeId::Browser | ModeId::Focus | ModeId::Tablet => WindowState::Maximized,
        ModeId::Classic | ModeId::Dock | ModeId::Panel => WindowState::Floating,
    }
}

pub fn window_geometry_map(windows: &BTreeMap<WindowId, WindowInfo>) -> BTreeMap<WindowId, Rect> {
    windows
        .iter()
        .map(|(id, info)| (*id, info.geometry))
        .collect()
}

fn remember_window(workspace: &mut Workspace, window: WindowId) {
    if !workspace.floating_windows.contains(&window) {
        workspace.floating_windows.push(window);
    }

    if matches!(workspace.root, LayoutNode::Empty) {
        workspace.root = LayoutNode::Window { window };
    }
}

fn handle_workspace_action(action: ShellAction, workspace: &mut Workspace) -> ActionResult {
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

fn work_area(bounds: Rect) -> Rect {
    let width = (bounds.width - CHROME_MARGIN * 2).max(1);
    let height = (bounds.height - CHROME_TOP - CHROME_BOTTOM).max(1);
    Rect::new(
        bounds.x + CHROME_MARGIN,
        bounds.y + CHROME_TOP,
        width,
        height,
    )
}

fn focus_area(bounds: Rect) -> Rect {
    let area = work_area(bounds);
    Rect::new(area.x, area.y, area.width, content_height(area.height))
}

fn content_height(height: i32) -> i32 {
    (height - WINDOW_CHROME_HEIGHT).max(1)
}

fn grid_columns(count: usize) -> i32 {
    let mut columns = 1;
    while (columns * columns) < count as i32 {
        columns += 1;
    }
    columns
}

fn div_ceil(value: i32, divisor: i32) -> i32 {
    (value + divisor - 1) / divisor
}

fn visible_windows<'a>(workspace: &'a Workspace, ctx: &'a LayoutContext<'_>) -> Vec<&'a WindowId> {
    workspace
        .floating_windows
        .iter()
        .filter(|window| {
            !matches!(
                ctx.windows.get(window).map(|info| &info.state),
                Some(WindowState::Hidden)
            )
        })
        .collect()
}
