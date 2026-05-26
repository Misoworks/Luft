use staccato_config::StaccatoConfig;
use staccato_layout::{LayoutEngine, Workspace, WorkspaceId};

pub fn layout_from_config(config: &StaccatoConfig) -> LayoutEngine {
    let workspaces = config
        .workspaces
        .entries
        .iter()
        .map(|(id, workspace)| {
            Workspace::empty(
                id.clone(),
                workspace.name.clone(),
                workspace.profile.clone(),
            )
        })
        .collect::<Vec<_>>();
    let active = workspaces
        .first()
        .map(|workspace| workspace.id.clone())
        .unwrap_or_else(|| WorkspaceId("1".to_string()));

    LayoutEngine::new(workspaces, active)
        .unwrap_or_else(|_| LayoutEngine::with_default_workspaces())
}
