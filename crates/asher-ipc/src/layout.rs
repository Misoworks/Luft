use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
mod engine;

pub use engine::{LayoutEngine, LayoutError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct WindowId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct WorkspaceId(pub String);

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
    pub floating_windows: Vec<WindowId>,
}

impl Workspace {
    pub fn empty(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: WorkspaceId(id.into()),
            name: name.into(),
            floating_windows: Vec::new(),
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
            state: WindowState::Floating,
            geometry,
            workspace,
        }
    }
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
