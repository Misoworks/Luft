use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

mod engine;
mod mode;

pub use engine::{LayoutEngine, LayoutError};
pub use mode::{
    ActionResult, ClassicMode, FocusMode, LayoutContext, ModeContext, ShellAction, ShellMode,
    ShellProfile, TilingMode, mode_for_profile, shell_mode, state_for_mode,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct WindowId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct WorkspaceId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ProfileId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AppId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub const fn zero() -> Self {
        Self::new(0, 0, 0, 0)
    }

    pub fn cascade(bounds: Self, index: usize, width: i32, height: i32) -> Self {
        let offset = (index as i32 * 32).min(192);
        let x = bounds.x + 80 + offset;
        let y = bounds.y + 72 + offset;
        let max_width = (bounds.width - 120).max(320);
        let max_height = (bounds.height - 120).max(240);

        Self::new(x, y, width.min(max_width), height.min(max_height))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WindowState {
    Normal,
    Floating,
    Tiled,
    Fullscreen,
    Maximized,
    Hidden,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub name: String,
    pub profile_id: ProfileId,
    pub root: LayoutNode,
    pub floating_windows: Vec<WindowId>,
    pub pinned_apps: Vec<AppId>,
    pub rules: Vec<WorkspaceRule>,
}

impl Workspace {
    pub fn empty(
        id: impl Into<String>,
        name: impl Into<String>,
        profile_id: impl Into<String>,
    ) -> Self {
        Self {
            id: WorkspaceId(id.into()),
            name: name.into(),
            profile_id: ProfileId(profile_id.into()),
            root: LayoutNode::Empty,
            floating_windows: Vec::new(),
            pinned_apps: Vec::new(),
            rules: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum LayoutNode {
    Empty,
    Window { window: WindowId },
    TabStack(TabStack),
    Split(SplitNode),
    Group(GroupNode),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TabStack {
    pub id: TabStackId,
    pub tabs: Vec<WindowId>,
    pub active: Option<WindowId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TabStackId(pub u64);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SplitNode {
    pub id: SplitNodeId,
    pub axis: SplitAxis,
    pub ratio: f32,
    pub first: Box<LayoutNode>,
    pub second: Box<LayoutNode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SplitNodeId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SplitAxis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupNode {
    pub id: GroupId,
    pub name: String,
    pub root: Box<LayoutNode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct GroupId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceRule {
    pub app_id: Option<String>,
    pub title_contains: Option<String>,
    pub placement: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChromeSpec {
    pub panel: bool,
    pub dock: bool,
    pub sidebar: bool,
    pub overview: bool,
    pub command_palette: bool,
}

impl ChromeSpec {
    pub const fn dock_default() -> Self {
        Self {
            panel: true,
            dock: true,
            sidebar: false,
            overview: true,
            command_palette: true,
        }
    }

    pub const fn browser_default() -> Self {
        Self {
            panel: false,
            dock: false,
            sidebar: true,
            overview: true,
            command_palette: true,
        }
    }
}

impl WindowInfo {
    pub fn new(id: WindowId, workspace: WorkspaceId, geometry: Rect) -> Self {
        Self {
            id,
            app_id: None,
            title: None,
            pid: None,
            is_xwayland: false,
            state: WindowState::Normal,
            geometry,
            workspace,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModeId {
    Classic,
    Dock,
    Panel,
    Tiling,
    Browser,
    Focus,
    Tablet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Arrangement {
    pub windows: BTreeMap<WindowId, Rect>,
}

impl Arrangement {
    pub fn empty() -> Self {
        Self {
            windows: BTreeMap::new(),
        }
    }
}
