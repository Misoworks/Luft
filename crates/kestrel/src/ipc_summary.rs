use crate::state::KestrelState;
use asher_ipc::{OutputSummary, ProfileSummary, StatusPayload, WindowSummary, WorkspaceSummary};
use asher_ipc::{ProfileId, WindowState, mode_for_profile};
use smithay::wayland::{
    compositor,
    shell::xdg::{ToplevelSurface, XdgToplevelSurfaceData},
};

pub fn status_payload(state: &KestrelState) -> StatusPayload {
    let active_workspace = state.layout.active_workspace().clone();
    let active_profile = active_profile(state)
        .unwrap_or_else(|| ProfileId(state.config.general.default_profile.clone()));

    StatusPayload {
        compositor: "kestrel".to_string(),
        shell: state.shell_status,
        xwayland: state.xwayland_status,
        xwayland_display: state.xwayland_display.clone(),
        active_workspace,
        active_mode: mode_for_profile(&active_profile),
        active_profile,
        nested: state.shell_control_path.is_some(),
        blur_enabled: state.config.general.enable_blur && state.config.effects.blur,
        debug_overlay: state.config.compositor.debug_overlay,
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
            profile: workspace.profile_id.clone(),
            mode: mode_for_profile(&workspace.profile_id),
        })
        .collect()
}

pub fn profile_summaries(state: &KestrelState) -> Vec<ProfileSummary> {
    known_profiles(state)
        .into_iter()
        .map(|profile| ProfileSummary {
            name: profile_name(&profile),
            mode: mode_for_profile(&profile),
            id: profile,
        })
        .collect()
}

pub fn output_summaries(state: &KestrelState) -> Vec<OutputSummary> {
    state.outputs.summaries()
}

pub fn known_profiles(state: &KestrelState) -> Vec<ProfileId> {
    let mut profiles = state
        .config
        .workspaces
        .entries
        .values()
        .map(|workspace| ProfileId(workspace.profile.clone()))
        .chain(
            state
                .layout
                .workspaces()
                .map(|workspace| workspace.profile_id.clone()),
        )
        .chain(std::iter::once(ProfileId(
            state.config.general.default_profile.clone(),
        )))
        .chain([ProfileId("panel-default".to_string())])
        .collect::<Vec<_>>();
    profiles.sort_by(|left, right| left.0.cmp(&right.0));
    profiles.dedup();
    profiles
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

fn active_profile(state: &KestrelState) -> Option<ProfileId> {
    state
        .layout
        .workspaces()
        .find(|workspace| &workspace.id == state.layout.active_workspace())
        .map(|workspace| workspace.profile_id.clone())
}

fn profile_name(profile: &ProfileId) -> String {
    profile
        .0
        .split('-')
        .map(title_case)
        .collect::<Vec<_>>()
        .join(" ")
}

fn title_case(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[derive(Debug, Default)]
struct WindowMetadata {
    title: Option<String>,
    app_id: Option<String>,
}
