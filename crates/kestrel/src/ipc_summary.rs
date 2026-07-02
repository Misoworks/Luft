use crate::state::KestrelState;
use asher_ipc::{OutputSummary, StatusPayload, WindowState, WindowSummary, WorkspaceSummary};
use smithay::wayland::{
    compositor,
    shell::xdg::{ToplevelSurface, XdgToplevelSurfaceData},
};

pub fn status_payload(state: &KestrelState) -> StatusPayload {
    let active_workspace = state.layout.active_workspace().clone();

    StatusPayload {
        compositor: "kestrel".to_string(),
        shell: state.shell_status,
        xwayland: state.xwayland_status,
        xwayland_display: state.xwayland_display.clone(),
        active_workspace,
        nested: state.shell_control_path.is_some(),
    }
}

pub fn window_summaries(state: &KestrelState) -> Vec<WindowSummary> {
    let active = state
        .windows
        .topmost_on_workspace(state.layout.active_workspace());
    state
        .windows
        .iter()
        .map(|managed| {
            let info = state.layout.window(managed.id);
            let metadata = window_metadata(&managed.surface);
            WindowSummary {
                id: managed.id,
                title: metadata
                    .title
                    .or_else(|| info.and_then(|window| window.title.clone())),
                app_id: metadata
                    .app_id
                    .or_else(|| info.and_then(|window| window.app_id.clone())),
                pid: info.and_then(|window| window.pid),
                workspace: managed.workspace.clone(),
                state: info
                    .map(|window| window.state.clone())
                    .unwrap_or(WindowState::Floating),
                geometry: info
                    .map(|window| window.geometry)
                    .unwrap_or_else(|| managed.geometry()),
                is_active: active == Some(managed.id),
                is_visible: &managed.workspace == state.layout.active_workspace()
                    && !managed.hidden,
            }
        })
        .collect()
}

pub fn workspace_summaries(state: &KestrelState) -> Vec<WorkspaceSummary> {
    state
        .layout
        .workspaces()
        .map(|workspace| WorkspaceSummary {
            id: workspace.id.clone(),
            name: workspace.name.clone(),
        })
        .collect()
}

pub fn output_summaries(state: &KestrelState) -> Vec<OutputSummary> {
    state.outputs.summaries()
}

fn window_metadata(surface: &ToplevelSurface) -> WindowMetadata {
    compositor::with_states(surface.wl_surface(), |states| {
        let Some(data) = states.data_map.get::<XdgToplevelSurfaceData>() else {
            return WindowMetadata::default();
        };
        let role = data.lock().unwrap();
        WindowMetadata {
            title: role.title.clone(),
            app_id: role.app_id.clone(),
        }
    })
}

#[derive(Debug, Default)]
struct WindowMetadata {
    title: Option<String>,
    app_id: Option<String>,
}
